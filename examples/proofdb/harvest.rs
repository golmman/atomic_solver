//! The harvest loop's job mechanics (plan2 D2, plan3 D1): job identity,
//! classification, tag derivation, and path replay.
//!
//! A harvest **job** is one frontier position of the current DB, by path
//! identity. Frontier classes (plan3 §2): `C1` open DB nodes, `C2`
//! unexpanded children of proven `win` nodes, `C3` unexpanded children of
//! open nodes — the DB-facing extraction and the coverage policies live in
//! [`super::policy`]. A decisive solve at a job produces a shard that grafts
//! exactly there (plan1's ancestor-upgrade machinery handles the overlay).
//! Proven nodes are never jobs; job paths are asserted disjoint from all DB
//! rows and the standing manifest's shard paths (plan3 decision 3).
//!
//! Search-side context lives in the CLI (`proofdb_harvest`): path replay +
//! `search_depth_with_prefix`, deterministic child-eval budgets, TT
//! retention across jobs. `Draw` from any cause maps to **censored**
//! ([`classify`]) — no fact, no DB write.
//!
//! Tests run in `tests/proofdb.rs` (which includes this module verbatim;
//! cargo does not run unit tests inside example targets).

use atomic_solver::notation::uci_to_move;
use atomic_solver::position::Outcome;
use atomic_solver::position::Position;

use super::ShardEntry;

/// Frontier class of a job (plan3 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobClass {
    /// Open DB node (`outcome IS NULL` row).
    Open,
    /// Unexpanded child of a proven `win` node; ranked by the parent's
    /// `depth_bound` ascending (the sharpness gradient).
    SharpSibling,
    /// Unexpanded child of an open node.
    OpenChild,
    /// Ledger-known-open frontier node (no DB row; a sidecar record from a
    /// previous censoring or exposure, plan4 D1).
    Ledger,
    /// Off-tree bootstrap candidate of the `descend` policy (plan15 D1; a
    /// startpos-rooted path of length 1..=K not covered by stored
    /// territory — see [`super::descend`]).
    Descend,
}

impl JobClass {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "C1",
            Self::SharpSibling => "C2",
            Self::OpenChild => "C3",
            Self::Ledger => "L",
            Self::Descend => "D",
        }
    }
}

/// One harvest job: a frontier position by path identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// UCI path from the startpos (empty for the root).
    pub path: Vec<String>,
    /// Plies from the startpos.
    pub ply: usize,
    /// Frontier class.
    pub class: JobClass,
    /// The parent's proven depth bound (C2 only; the sharpness ranking key).
    pub parent_bound: Option<u32>,
    /// Screen budget (child-evals) resolved by the coverage policy.
    pub budget: u64,
}

/// Classify a search outcome per pre-registered decision 1: `Draw` from any
/// cause (budget exhaustion included) is **censored** — no fact, no DB
/// write; only decisive outcomes produce a shard.
#[must_use]
pub fn classify(outcome: Outcome) -> Option<Outcome> {
    match outcome {
        Outcome::Draw => None,
        decisive => Some(decisive),
    }
}

/// Shard tag for a job path: `"h_"` + the first 16 hex chars of the SHA-256
/// over the space-separated UCI path (deterministic; the root's empty path
/// hashes to the well-known empty-string digest). Collisions are checked
/// against the standing manifest by the caller.
#[must_use]
pub fn tag_for_path(path: &[String]) -> String {
    let joined = path.join(" ");
    format!("h_{}", &super::digest_hex(joined.as_bytes())[..16])
}

/// Build the manifest entry for a validated shard at `job`. `validate` is
/// always `"ok"` — the caller writes an entry only after a validator pass
/// (pre-registered decision 2).
#[must_use]
pub fn make_entry(job: &Job, fen: &str, outcome: Outcome) -> ShardEntry {
    let tag = tag_for_path(&job.path);
    ShardEntry {
        file: format!("{tag}.bin"),
        tag,
        fen: fen.to_string(),
        moves: job.path.clone(),
        outcome,
        validate: "ok".to_string(),
    }
}

/// Replay `path` from the startpos and build the repetition-prefix keys the
/// search must be seeded with: the keys of every position *before* the node
/// (the node's own key is pushed by the search itself). For the root job
/// (empty path) the prefix is therefore **empty** — seeding it with the
/// startpos key would put the startpos twice on the search's own path and
/// yield an instant false repetition-draw (a real defect the plan2 batch
/// caught on its root job; the false `Draw` was harmlessly censored, but the
/// job's work measurement was wrong).
///
/// # Errors
/// An illegal move on the replayed path.
pub fn replay_job_path(path: &[String]) -> Result<(Position, Vec<u64>), String> {
    let mut pos =
        Position::from_fen(Position::STARTPOS_FEN).map_err(|e| format!("startpos FEN: {e}"))?;
    let mut prefix: Vec<u64> = Vec::new();
    if !path.is_empty() {
        prefix.push(pos.repetition_key());
    }
    for (i, uci) in path.iter().enumerate() {
        let Some(mv) = uci_to_move(uci, &pos) else {
            return Err(format!("illegal move {uci} at ply {i} during replay"));
        };
        pos.do_move(mv);
        if i + 1 < path.len() {
            prefix.push(pos.repetition_key());
        }
    }
    Ok((pos, prefix))
}

#[cfg(test)]
mod tests;
