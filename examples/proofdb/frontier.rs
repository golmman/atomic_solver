//! Frontier-class extraction (plan3 D1): C1/C2/C3 over the full DB frontier,
//! with the AND-completeness assert and the decision-3 disjointness checks.
//!
//! The DB is a partial AND/OR tree keyed by path: a proven `loss` node
//! (AND) stores a refutation for every legal reply (asserted here — a
//! violation is a merger/model defect and aborts), while a proven `win`
//! node (OR) stores exactly one proving child; every other legal move under
//! it is *unexpanded*. The classes (plan3 §2):
//!
//! - **C1** — open nodes (`outcome IS NULL` rows);
//! - **C2** — unexpanded children of proven `win` nodes (the ~150k-position
//!   class a user hits by replaying a non-stored move);
//! - **C3** — unexpanded children of open nodes.
//!
//! Children of proven `loss` nodes are excluded by the AND-completeness
//! assert (every legal reply is already a row). Extraction is one read-only
//! pass ([`super::db::load_db_rows`] + a DFS replay from the startpos), so
//! C2/C3 child paths are legal by construction and checked disjoint from
//! all DB rows and manifest paths (decision 3). Cost is O(rows + frontier):
//! one movegen pass per win/open/loss row — the 150k-class C2 enumeration
//! builds in well under a second. The class *rankings* and the coverage
//! policies live in [`super::policy`].
//!
//! Tests run in `tests/proofdb.rs` (which includes this module verbatim;
//! cargo does not run unit tests inside example targets).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::Outcome;
use atomic_solver::position::Position;

use super::db::load_db_rows;
use super::harvest::{Job, JobClass};

type Res<T> = Result<T, String>;

/// The full frontier, ranked per class (extraction output).
#[derive(Debug)]
pub struct Frontier {
    pub root_fen: String,
    /// Total DB node count (census).
    pub n_nodes: usize,
    /// Open rows (C1), deepest-first (plan2's rule).
    pub c1: Vec<Job>,
    /// Unexpanded children of proven win nodes (C2), sharpness order.
    pub c2: Vec<Job>,
    /// Unexpanded children of open nodes (C3), ply-ascending order.
    pub c3: Vec<Job>,
    /// Proven loss nodes the AND-completeness assert walked (census).
    pub and_checks: usize,
}

/// A job path must be neither a DB row (open or proven) nor a manifest path
/// (plan3 decision 3, all classes).
fn reject_known(
    path: &[String],
    by_path: &HashMap<String, usize>,
    manifest_paths: &HashSet<String>,
) -> Res<()> {
    let p = path.join(" ");
    if by_path.contains_key(&p) {
        return Err(format!(
            "frontier path {p:?} is also a DB row (extraction defect)"
        ));
    }
    if manifest_paths.contains(&p) {
        return Err(format!(
            "frontier path {p:?} already has a shard in the manifest"
        ));
    }
    Ok(())
}

/// Extract the full frontier (C1/C2/C3) from the DB at `path`, read-only,
/// with the AND-completeness assert over all proven loss nodes and the
/// decision-3 disjointness checks. C2/C3 child paths are replayed from the
/// startpos during the DFS, so they are legal by construction (the run-time
/// `replay_job_path` re-checks anyway).
///
/// # Errors
/// Anything [`load_db_rows`] rejects, a broken DB tree, an AND-completeness
/// violation (merger/model defect — abort, never a workaround), a proven
/// win row without a `depth_bound`, or a frontier path that is a DB row or
/// a manifest path.
pub fn extract_frontier(
    db_path: &Path,
    manifest_sha256: &str,
    manifest_paths: &HashSet<String>,
) -> Res<Frontier> {
    let db = load_db_rows(db_path, manifest_sha256)?;
    let by_path: HashMap<String, usize> = db
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| (r.path.join(" "), i))
        .collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); db.rows.len()];
    for (i, r) in db.rows.iter().enumerate() {
        if r.ply == 0 {
            continue;
        }
        let parent_path = r.path[..r.path.len() - 1].join(" ");
        let p = *by_path
            .get(&parent_path)
            .ok_or("broken DB tree (child without a parent row)")?;
        children[p].push(i);
    }
    let root_idx = *by_path.get("").ok_or("DB has no root row")?;
    let startpos =
        Position::from_fen(Position::STARTPOS_FEN).map_err(|e| format!("startpos FEN: {e}"))?;
    let mut c1 = Vec::new();
    let mut c2 = Vec::new();
    let mut c3 = Vec::new();
    let mut and_checks = 0usize;
    let mut stack: Vec<(usize, Position)> = vec![(root_idx, startpos)];
    while let Some((idx, pos)) = stack.pop() {
        let row = &db.rows[idx];
        let path_str = row.path.join(" ");
        // This node's stored children: a child path P+m is a DB row iff it
        // is a child row of the row at P (the DB is a path tree).
        let child_ucis: HashSet<&str> = children[idx]
            .iter()
            .map(|&c| {
                db.rows[c]
                    .path
                    .last()
                    .expect("non-root row has a move")
                    .as_str()
            })
            .collect();
        match row.outcome {
            None => {
                if manifest_paths.contains(&path_str) {
                    return Err(format!("open node {path_str:?} already has a shard"));
                }
                c1.push(Job {
                    ply: row.ply,
                    path: row.path.clone(),
                    class: JobClass::Open,
                    parent_bound: None,
                    budget: 0,
                });
                for mv in pos.legal_moves_vec() {
                    let uci = move_to_uci(mv);
                    if child_ucis.contains(uci.as_str()) {
                        continue;
                    }
                    let mut path = row.path.clone();
                    path.push(uci);
                    reject_known(&path, &by_path, manifest_paths)?;
                    c3.push(Job {
                        ply: row.ply + 1,
                        path,
                        class: JobClass::OpenChild,
                        parent_bound: None,
                        budget: 0,
                    });
                }
            }
            Some(Outcome::Win) => {
                let bound = row
                    .depth_bound
                    .ok_or_else(|| format!("proven win at {path_str:?} has no depth_bound"))?;
                for mv in pos.legal_moves_vec() {
                    let uci = move_to_uci(mv);
                    if child_ucis.contains(uci.as_str()) {
                        continue;
                    }
                    let mut path = row.path.clone();
                    path.push(uci);
                    reject_known(&path, &by_path, manifest_paths)?;
                    c2.push(Job {
                        ply: row.ply + 1,
                        path,
                        class: JobClass::SharpSibling,
                        parent_bound: Some(bound),
                        budget: 0,
                    });
                }
            }
            Some(Outcome::Loss) => {
                and_checks += 1;
                for mv in pos.legal_moves_vec() {
                    let uci = move_to_uci(mv);
                    if !child_ucis.contains(uci.as_str()) {
                        return Err(format!(
                            "AND-completeness violated: proven loss at {path_str:?} has no DB \
                             row for legal reply {uci} (merger or model defect — abort)"
                        ));
                    }
                }
            }
            Some(Outcome::Draw) => {
                return Err(format!("draw outcome at {path_str:?} is not a DB fact"));
            }
        }
        for &c in &children[idx] {
            let last = db.rows[c]
                .path
                .last()
                .expect("non-root row has a move")
                .clone();
            let mv = uci_to_move(&last, &pos).ok_or_else(|| {
                format!("DB move {last:?} below {path_str:?} is illegal on replay")
            })?;
            let mut child_pos = pos.clone();
            child_pos.do_move(mv);
            stack.push((c, child_pos));
        }
    }
    // C1: deepest-first, ties lexicographic path ascending (plan2's rule).
    c1.sort_by(|a, b| {
        b.ply
            .cmp(&a.ply)
            .then_with(|| a.path.join(" ").cmp(&b.path.join(" ")))
    });
    // C2: parent depth_bound ascending (sharpness gradient), ties ply
    // ascending, ties path lexicographic ascending.
    c2.sort_by(|a, b| {
        a.parent_bound
            .cmp(&b.parent_bound)
            .then_with(|| a.ply.cmp(&b.ply))
            .then_with(|| a.path.join(" ").cmp(&b.path.join(" ")))
    });
    // C3: ply ascending, ties path lexicographic ascending.
    c3.sort_by(|a, b| {
        a.ply
            .cmp(&b.ply)
            .then_with(|| a.path.join(" ").cmp(&b.path.join(" ")))
    });
    Ok(Frontier {
        root_fen: db.root_fen,
        n_nodes: db.rows.len(),
        c1,
        c2,
        c3,
        and_checks,
    })
}

#[cfg(test)]
mod tests;
