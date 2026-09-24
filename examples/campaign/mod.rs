//! Shared protocol for the plan5 two-job campaign prototype
//! (`campaign_master` / `campaign_worker`).
//!
//! This is campaign-side code only (initiative non-goal: campaign code lives
//! outside the product surface). The message schemas and the soundness
//! contract they implement are normative in
//! `docs/plans/solve/campaign_architecture.md` §2 and §8; the subset actually
//! implemented by this v0 prototype is recorded in that document's §10.
//!
//! File-based store v0 layout under a session directory:
//!
//! ```text
//! session.json    master-written session config (root FEN, budgets)
//! jobs/           master ⇒ worker job files (claimed by atomic rename)
//! results/        worker ⇒ master result files (durable before acting)
//! stop            master ⇒ workers: drain and exit
//! ```

#![allow(dead_code)]

use atomic_solver::notation::uci_to_move;
use atomic_solver::position::Position;
use std::io::Write;
use std::path::Path;

pub const SESSION_FILE: &str = "session.json";
pub const JOBS_DIR: &str = "jobs";
pub const RESULTS_DIR: &str = "results";
pub const STOP_FILE: &str = "stop";

pub mod state;
pub mod verify;

/// Master-written session configuration (also the workers' source of truth
/// for the campaign root FEN).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionConfig {
    pub root_fen: String,
    pub tt_mb: usize,
    pub slice_budget: u64,
    pub max_slice: u64,
    pub feedback: bool,
}

/// One job: `(job_id, subposition, direction, budget, context)` per the
/// architecture doc §1. `path_uci` is the full move path from the campaign
/// root; the worker replays it to derive the subposition (clock included)
/// and the repetition-prefix context.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub job_id: String,
    pub worker: usize,
    pub path_uci: Vec<String>,
    pub direction: String,
    pub budget_evals: u64,
}

/// One proof-tree fact inside a worker result: path relative to the job
/// root, proven outcome, proven depth. Terminal and structural facts are
/// re-derived by the master's replay; these carry the worker's *decision*.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventRec {
    pub path: Vec<String>,
    pub outcome: String,
    pub depth: u32,
}

/// Worker ⇒ master result. `events` is present only for decisive outcomes
/// (worker-decisive facts cross as proof-event streams, never bare outcomes;
/// advisory numbers `root_pn`/`root_dn` are never trusted for composition).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobResult {
    pub job_id: String,
    /// Echo of the job's path (diagnostic + master cross-check).
    #[serde(default)]
    pub path_uci: Vec<String>,
    pub outcome: String,
    pub exit_reason: String,
    pub nodes: u64,
    pub child_evals: u64,
    pub wall_s: f64,
    pub root_pn: u64,
    pub root_dn: u64,
    #[serde(default)]
    pub events: Vec<EventRec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Read a JSON file, failing with context.
pub fn read_json<T: serde::de::DeserializeOwned, P: AsRef<Path>>(path: P) -> Result<T, String> {
    let data = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    serde_json::from_slice(&data).map_err(|e| format!("parse: {e}"))
}

/// Atomic JSON write (tmp + rename) so readers never observe a torn file.
pub fn write_json<T: serde::Serialize, P: AsRef<Path>>(path: P, value: &T) -> Result<(), String> {
    let path = path.as_ref();
    let tmp = path.with_extension("tmp");
    let text = serde_json::to_vec(value).map_err(|e| format!("serialize: {e}"))?;
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| format!("create: {e}"))?;
        f.write_all(&text).map_err(|e| format!("write: {e}"))?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("rename: {e}"))
}

/// Replay `path_uci` from the campaign root FEN.
///
/// Returns the position at the subposition and the repetition-prefix keys
/// the worker must seed the search with: the board keys of the campaign root
/// and of every position *before* the subposition (the subposition's own key
/// is pushed by the search itself) — the same discipline the reconstruct
/// walker uses, so every repetition judgment inside the job is made under
/// the global path's ancestor set.
pub fn replay_path(root_fen: &str, path_uci: &[String]) -> Result<(Position, Vec<u64>), String> {
    let mut pos = Position::from_fen(root_fen).map_err(|e| format!("root FEN: {e}"))?;
    let mut prefix = vec![pos.repetition_key()];
    for (i, uci) in path_uci.iter().enumerate() {
        let mv = uci_to_move(uci, &pos)
            .ok_or_else(|| format!("illegal move {uci} at ply {i} during replay"))?;
        pos.do_move(mv);
        if i + 1 < path_uci.len() {
            prefix.push(pos.repetition_key());
        }
    }
    Ok((pos, prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_path_builds_prefix_excluding_job_root() {
        let fen = Position::STARTPOS_FEN;
        let (pos, prefix) = replay_path(fen, &["e2e4".into(), "e7e5".into()]).unwrap();
        assert_eq!(prefix.len(), 2, "root key + first move key");
        assert_eq!(
            pos.fen(),
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2"
        );
    }

    #[test]
    fn replay_rejects_illegal_move() {
        assert!(replay_path(Position::STARTPOS_FEN, &["e2e5".into()]).is_err());
    }

    #[test]
    fn job_result_json_round_trips() {
        let r = JobResult {
            job_id: "w0_1".into(),
            path_uci: vec!["a2a3".into()],
            outcome: "win".into(),
            exit_reason: "Complete".into(),
            nodes: 10,
            child_evals: 20,
            wall_s: 0.5,
            root_pn: 0,
            root_dn: 7,
            events: vec![EventRec {
                path: vec!["a2a3".into()],
                outcome: "win".into(),
                depth: 1,
            }],
            error: None,
        };
        let text = serde_json::to_string(&r).unwrap();
        let back: JobResult = serde_json::from_str(&text).unwrap();
        assert_eq!(back.events.len(), 1);
    }
}
