//! The harvest loop's DB-facing engine (plan2 D2): frontier extraction,
//! job ordering, tag derivation, and outcome classification.
//!
//! A harvest **job** is one open node of the current DB (`outcome IS NULL`,
//! by path identity); a decisive solve there produces a shard that grafts
//! exactly there (plan1's ancestor-upgrade machinery handles the overlay).
//! Job order is deepest-first (ply descending, ties lexicographic path
//! ascending — recorded, replayable). Proven nodes are never jobs; job paths
//! are asserted disjoint from both the DB's proven paths and the standing
//! manifest's shard paths.
//!
//! Search-side context lives in the CLI (`proofdb_harvest`): path replay +
//! `search_depth_with_prefix`, deterministic child-eval budgets, TT
//! retention across jobs. `Draw` from any cause maps to **censored**
//! ([`classify`]) — no fact, no DB write.
//!
//! Tests run in `tests/proofdb.rs` (which includes this module verbatim;
//! cargo does not run unit tests inside example targets).

use std::path::Path;

use rusqlite::OpenFlags;

use atomic_solver::notation::uci_to_move;
use atomic_solver::position::Outcome;
use atomic_solver::position::Position;

use super::ShardEntry;

type Res<T> = Result<T, String>;

/// The DB content the harvest loop works against (extracted read-only).
#[derive(Debug)]
pub struct HarvestInput {
    /// The DB's `root_fen` meta row (must be the startpos).
    pub root_fen: String,
    /// Paths (space-separated UCI) of all *proven* DB nodes, for the
    /// disjointness assert.
    pub proven_paths: std::collections::HashSet<String>,
    /// Jobs = open nodes, deepest-first (ply desc, ties lexicographic path).
    pub jobs: Vec<Job>,
    /// Total node count (census).
    pub n_nodes: usize,
}

/// One harvest job: an open node by path identity.
#[derive(Debug)]
pub struct Job {
    /// UCI path from the startpos (empty for the root).
    pub path: Vec<String>,
    /// Plies from the startpos.
    pub ply: usize,
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

/// Extract the harvest frontier from the DB at `path`, read-only, and abort
/// unless `meta.built_from` equals `manifest_sha256` (no silent divergence
/// between DB and shards) and `root_fen` is the startpos.
///
/// # Errors
/// Any SQLite/I/O error, a digest mismatch (stale DB), a non-startpos root,
/// a broken parent chain, or a ply inconsistency.
pub fn extract_jobs(db_path: &Path, manifest_sha256: &str) -> Res<HarvestInput> {
    let conn = rusqlite::Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("cannot open DB {}: {e}", db_path.display()))?;
    let meta = |key: &str| -> Res<String> {
        conn.query_row("SELECT value FROM meta WHERE key=?1", [key], |r| r.get(0))
            .map_err(|e| format!("meta row {key:?}: {e}"))
    };
    let built_from: String = meta("built_from")?;
    if built_from != manifest_sha256 {
        return Err(format!(
            "DB built_from {} != manifest digest {} (stale DB or stale manifest — \
             re-merge the standing shard set first)",
            &built_from[..16],
            &manifest_sha256[..16]
        ));
    }
    let root_fen: String = meta("root_fen")?;
    if root_fen != Position::STARTPOS_FEN {
        return Err(format!("DB root_fen is not the startpos: {root_fen}"));
    }
    let mut stmt = conn
        .prepare("SELECT id, parent_id, ply, move_uci, outcome FROM nodes ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    // Row id → (parent row, ply, move) for the path walk; rows are dense
    // ids 0..n by construction, but resolve through the map regardless.
    let by_id: std::collections::HashMap<i64, (Option<i64>, i64, Option<String>)> = rows
        .iter()
        .map(|r| (r.0, (r.1, r.2, r.3.clone())))
        .collect();
    let mut proven_paths = std::collections::HashSet::new();
    let mut jobs: Vec<(usize, String, Vec<String>)> = Vec::new();
    for (_, parent, ply, mv_uci, outcome) in &rows {
        let mut chain: Vec<String> = Vec::new();
        let mut cur = Some((*parent, *ply, mv_uci.clone()));
        while let Some((p, pl, mv)) = cur {
            match mv {
                Some(m) => chain.push(m),
                None if pl == 0 => {}
                None => return Err("non-root node without a move".to_string()),
            }
            cur = p
                .map(|p| {
                    by_id
                        .get(&p)
                        .cloned()
                        .ok_or_else(|| "broken parent chain (unresolvable parent_id)".to_string())
                })
                .transpose()?;
        }
        chain.reverse();
        let path: Vec<String> = chain;
        // Parent ply must be exactly one below the child's (DB integrity).
        if *ply > 0 {
            let p_id = parent.expect("non-root has a parent");
            let p_ply = by_id.get(&p_id).expect("parent resolves").1;
            if p_ply != *ply - 1 {
                return Err(format!("node at ply {ply} has a parent at ply {p_ply}"));
            }
        }
        match outcome.as_deref() {
            None => jobs.push((*ply as usize, path.join(" "), path)),
            Some(o) if o == "win" || o == "loss" => {
                proven_paths.insert(path.join(" "));
            }
            Some(o) => return Err(format!("unexpected node outcome {o:?}")),
        }
    }
    // Deepest-first: ply descending, ties lexicographic path ascending.
    jobs.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let n_nodes = rows.len();
    Ok(HarvestInput {
        root_fen,
        proven_paths,
        jobs: jobs
            .into_iter()
            .map(|(_, _, path)| {
                let ply = path.len();
                Job { path, ply }
            })
            .collect(),
        n_nodes,
    })
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
