//! The harvest loop's job mechanics (plan2 D2, plan3 D1): job identity,
//! classification, tag derivation, and path replay.
//!
//! A harvest **job** is one frontier position of the current DB, by path
//! identity. Frontier classes (plan3 §2, reused by the walk's census as
//! territory tags, plan16 D1): `C1` open DB nodes, `C3` unexpanded
//! children of open nodes, `L` ledger-known-open nodes. The plan18 policy
//! consolidation removed the C2 sharp-sibling class and the
//! precomputed-sequence machinery with the legacy policies; the only
//! remaining policy is the `frontier-walk` ledger walk ([`super::walk`]).
//! A decisive solve at a job produces a shard that grafts exactly there
//! (plan1's ancestor-upgrade machinery handles the overlay).
//!
//! Search-side context lives in the CLI (`proofdb_harvest`): path replay +
//! `search_depth_with_prefix`, deterministic child-eval budgets, TT
//! retention across jobs. [`classify`] separates a **proven** draw
//! (`Outcome::Draw` with `ExitReason::Complete`) from a censored draw
//! (budget exhaustion or any other non-complete exit): the former is a
//! path-keyed scheduling fact (a `drawn` row; no shard, no tree node), the
//! latter stays a fact of absence (a `censor` row). Only decisive outcomes
//! produce shards.
//!
//! Tests run in `tests/proofdb.rs` (which includes this module verbatim;
//! cargo does not run unit tests inside example targets) and in
//! `harvest/tests.rs`.

use atomic_solver::notation::uci_to_move;
use atomic_solver::position::Outcome;
use atomic_solver::position::Position;
use atomic_solver::search::dfpn::ExitReason;

use super::sidecar::SidecarEntry;

/// Frontier class of a job (plan3 §2; plan18 removed the C2/D classes
/// with the legacy and `descend` policies).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobClass {
    /// Open DB node (`outcome IS NULL` row).
    Open,
    /// Unexpanded child of an open node.
    OpenChild,
    /// Ledger-known-open frontier node (no DB row; a sidecar record from a
    /// previous censoring or exposure, plan4 D1).
    Ledger,
}

impl JobClass {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "C1",
            Self::OpenChild => "C3",
            Self::Ledger => "L",
        }
    }
}

/// The space-joined key of a child path (empty parent = root). The path
/// identity convention shared by the ledger, the DB rows, and the census.
#[must_use]
pub fn child_key(parent_key: &str, uci: &str) -> String {
    if parent_key.is_empty() {
        uci.to_string()
    } else {
        format!("{parent_key} {uci}")
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
    /// Search budget (child-evals) granted to the job.
    pub budget: u64,
}

/// The three-way job verdict: a decisive proof (shard), a proven draw
/// (path-keyed scheduling mark; the tree model still has no draw outcome),
/// or a censor (fact of absence).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobVerdict {
    /// `Win`/`Loss`: export a shard as before.
    Decisive(Outcome),
    /// `Draw` with `ExitReason::Complete`: the solver proved the draw. No
    /// shard or tree node exists for it (schema v2 has no draw outcome),
    /// but it is recorded as a `drawn` scheduling row so the walk prunes the
    /// subtree instead of re-searching it.
    ProvenDraw,
    /// `Draw` from budget exhaustion or any other non-complete exit.
    Censored,
}

/// Classify a search outcome + exit reason. Only a `Draw` that completed
/// proves a draw; every other draw is censored (pre-registered decision 1's
/// soundness rule, now without discarding a completed proof).
#[must_use]
pub fn classify(outcome: Outcome, exit_reason: ExitReason) -> JobVerdict {
    match outcome {
        Outcome::Draw if exit_reason == ExitReason::Complete => JobVerdict::ProvenDraw,
        Outcome::Draw => JobVerdict::Censored,
        decisive => JobVerdict::Decisive(decisive),
    }
}

/// Shard tag for a job path: `"h_"` + the first 16 hex chars of the SHA-256
/// over the space-separated UCI path (deterministic; the root's empty path
/// hashes to the well-known empty-string digest). Collisions are checked
/// against the standing shard rows by the caller.
#[must_use]
pub fn tag_for_path(path: &[String]) -> String {
    let joined = path.join(" ");
    format!("h_{}", &super::digest_hex(joined.as_bytes())[..16])
}

/// Build the sidecar for a validated shard at `job` (plan22 D1). The
/// `sha256` field is filled in by the caller after the bin is serialized.
#[must_use]
pub fn make_sidecar(job: &Job, fen: &str, outcome: Outcome) -> SidecarEntry {
    let tag = tag_for_path(&job.path);
    SidecarEntry {
        file: format!("{tag}.bin"),
        tag,
        fen: fen.to_string(),
        moves: job.path.clone(),
        outcome,
        sha256: String::new(),
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
