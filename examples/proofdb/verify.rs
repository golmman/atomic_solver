//! The whole-DB replay validator (plan22 D7): `proofdb_verify` re-validates
//! the standing DB's tree read back from the DB itself — validation is path
//! replay, it needs no shards. Streaming DFS from the root with a lib
//! `Position` (do/undo), one indexed children query per node, O(depth) RAM.
//!
//! Per node:
//! - **replay legality** — the stored move parses via `uci_to_move` in the
//!   replayed parent position, `ply = parent ply + 1`, and
//!   `move_code == move_to_bits(move)`;
//! - **proven AND** (`outcome='loss'`, non-terminal) — the child set equals
//!   the full legal-reply set (movegen) and every child is `win`, with
//!   `depth_bound = max(child bound) + 1` (the finalize fixpoint rule,
//!   checked locally on the retreat);
//! - **proven OR** (`outcome='win'`, non-terminal) — at least one
//!   proven-loss child (alternative winning moves are annotation children,
//!   which is what the merged reality stores — the spec's looser
//!   "exactly one proving child" wording is fixed by item 19's spec v2)
//!   and `depth_bound = min(loss-child bound) + 1`;
//! - **proven terminal** — `depth_bound = 0`, no children, and statically
//!   terminal in the replayed position (lib terminal classification);
//! - **open nodes** — no structural requirement (annotation/graft
//!   territory), but a stored bound/status is a defect;
//! - **every row** — non-empty `provenance`, valid outcome/status domains,
//!   and `NULL` bound/status iff `NULL` outcome;
//! - plus `meta.node_count == COUNT(nodes)` and full reachability from the
//!   root.
//!
//! Recorded limitation (accepted, D7): non-terminal `exact` minimality is
//! not re-derivable from the DB alone — trusted as a search fact (the same
//! class of trust the shard validator already extends to search-computed
//! bounds).

use std::collections::HashMap;

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::MoveList;

use atomic_solver::notation::{move_to_bits, move_to_uci, uci_to_move};
use atomic_solver::position::{Outcome, Position};
use rusqlite::Connection;

type Res<T> = Result<T, String>;

/// The meta checks (schema version, root FEN, `node_count == COUNT(nodes)
/// census). The CLI drives this before [`verify_db`]; tests exercise it
/// alongside the tree checks.
///
/// # Errors
/// A missing/mistyped meta row, a non-v2 schema, a non-startpos root, or a
/// declared node count that disagrees with the table.
pub fn check_meta(conn: &Connection) -> Res<()> {
    let version: String = conn
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("meta row schema_version: {e}"))?;
    if version != "2" {
        return Err(format!("DB schema version {version:?} is not v2"));
    }
    let root_fen: String = conn
        .query_row("SELECT value FROM meta WHERE key='root_fen'", [], |r| {
            r.get(0)
        })
        .map_err(|e| format!("meta row root_fen: {e}"))?;
    if root_fen != Position::STARTPOS_FEN {
        return Err(format!("DB root_fen is not the startpos: {root_fen}"));
    }
    let declared: String = conn
        .query_row("SELECT value FROM meta WHERE key='node_count'", [], |r| {
            r.get(0)
        })
        .map_err(|e| format!("meta row node_count: {e}"))?;
    let declared: i64 = declared
        .parse()
        .map_err(|_| format!("meta node_count {declared:?} is not a number"))?;
    let actual: i64 = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if declared != actual {
        return Err(format!(
            "meta node_count {declared} != COUNT(nodes) {actual}"
        ));
    }
    Ok(())
}

/// A defect line (node-qualified, one per defective row).
pub type Defects = Vec<String>;

/// The verify verdict.
#[derive(Debug)]
pub enum Verdict {
    /// The DB tree is fully valid.
    Ok {
        nodes: usize,
        proven: usize,
        open: usize,
    },
    /// `total` defects found (all counted); up to `reported` lines.
    Failed {
        total: usize,
        reported: Defects,
        nodes: usize,
    },
}

/// One `nodes` row as verify reads it.
struct Row {
    id: i64,
    ply: i64,
    move_uci: Option<String>,
    move_code: Option<i64>,
    outcome: Option<Outcome>,
    bound: Option<i64>,
    status: Option<String>,
    provenance: String,
}

/// Read the children of `parent` (`None` = the root) in id order.
fn read_children(conn: &Connection, parent: Option<i64>) -> Res<Vec<Row>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, ply, move_uci, move_code, outcome, depth_bound, depth_status,
             provenance FROM nodes WHERE parent_id IS ?1 ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([parent], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, String>(7)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, ply, move_uci, move_code, outcome_s, bound, status, provenance) in rows {
        let outcome = match outcome_s.as_deref() {
            None => None,
            Some("win") => Some(Outcome::Win),
            Some("loss") => Some(Outcome::Loss),
            Some(o) => return Err(format!("node {id}: unexpected outcome {o:?}")),
        };
        out.push(Row {
            id,
            ply,
            move_uci,
            move_code,
            outcome,
            bound,
            status,
            provenance,
        });
    }
    Ok(out)
}

/// Verify the DB tree (the caller has checked the meta rows). Defects are
/// accumulated (each counted; up to `max_report` reported); traversal
/// continues where possible and abandons a subtree where a defect breaks
/// the replay.
///
/// # Errors
/// A tool-level failure (missing tables, a duplicate root, an unresolvable
/// outcome value) — distinct from tree defects.
pub fn verify_db(conn: &Connection, max_report: usize) -> Res<Verdict> {
    let node_count: usize = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get::<_, i64>(0))
        .map_err(|e| e.to_string())? as usize;
    let roots = read_children(conn, None)?;
    if roots.len() != 1 {
        return Err(format!(
            "nodes has {} parentless rows (expected exactly one root)",
            roots.len()
        ));
    }
    let mut defects: Defects = Vec::new();
    let mut proven = 0usize;
    let mut open = 0usize;
    let mut visited = 0usize;
    let mut pos = Position::from_fen(Position::STARTPOS_FEN).map_err(|e| e.to_string())?;
    let mut state = StateInfo::new();
    let mut move_list = MoveList::new();
    let root = roots.into_iter().next().expect("one root");
    visit(
        conn,
        root,
        &mut pos,
        &mut state,
        &mut move_list,
        &mut defects,
        &mut proven,
        &mut open,
        &mut visited,
        &mut HashMap::new(),
    );
    if defects.is_empty() {
        if visited != node_count {
            return Err(format!(
                "{}/{} node rows are unreachable from the root",
                node_count - visited,
                node_count
            ));
        }
        Ok(Verdict::Ok {
            nodes: node_count,
            proven,
            open,
        })
    } else {
        let total = defects.len();
        defects.truncate(max_report);
        Ok(Verdict::Failed {
            total,
            reported: defects,
            nodes: node_count,
        })
    }
}

/// The verify DFS (do/undo on one `Position`). Invariant: on entry `pos` is
/// the row's *parent* position (the root: the startpos, no move to apply);
/// the row's move is applied inside and undone before returning, so the
/// caller's loop position is unchanged across children.
#[allow(clippy::too_many_arguments)]
fn visit(
    conn: &Connection,
    row: Row,
    pos: &mut Position,
    state: &mut StateInfo,
    move_list: &mut MoveList,
    defects: &mut Defects,
    proven: &mut usize,
    open: &mut usize,
    visited: &mut usize,
    seen: &mut HashMap<i64, ()>,
) {
    let label = |uci: &Option<String>| uci.clone().unwrap_or_else(|| "<root>".to_string());
    if seen.insert(row.id, ()).is_some() {
        defects.push(format!("node {}: visited twice (cycle)", row.id));
        return;
    }
    // Replay legality: the stored move must be legal in the replayed parent
    // position, and the stored move-code must agree.
    let mv = if row.ply == 0 {
        if row.move_uci.is_some() || row.move_code.is_some() {
            defects.push(format!(
                "node {}: the root row must have no move/move_code",
                row.id
            ));
        }
        None
    } else {
        let Some(uci) = &row.move_uci else {
            defects.push(format!("node {}: non-root row without a move", row.id));
            return;
        };
        let Some(mv) = uci_to_move(uci, pos) else {
            defects.push(format!(
                "node {}: stored move {uci:?} is not legal in the replayed position",
                row.id
            ));
            return;
        };
        if row.move_code != Some(i64::from(move_to_bits(mv))) {
            defects.push(format!(
                "node {}: move_code {:?} disagrees with move {uci:?}",
                row.id, row.move_code
            ));
        }
        Some(mv)
    };
    // Field domains. Provenance is non-empty for every fact row; the root
    // row of a shard-less (bootstrap) layer legitimately has none —
    // provenance means "shard tags that contributed facts here", and a
    // fresh root has no contributing shards yet.
    if row.provenance.is_empty() && row.ply != 0 {
        defects.push(format!("node {}: empty provenance", row.id));
    }
    if row.outcome.is_none() && (row.bound.is_some() || row.status.is_some()) {
        defects.push(format!(
            "node {}: open row with a stored bound/status",
            row.id
        ));
    }
    if row.outcome.is_some() && (row.bound.is_none() || row.status.is_none()) {
        defects.push(format!(
            "node {}: proven row without a bound/status",
            row.id
        ));
    }
    if let Some(s) = row.status.as_deref()
        && s != "bound"
        && s != "exact"
    {
        defects.push(format!("node {}: unknown depth_status {s:?}", row.id));
    }
    match row.outcome {
        None => *open += 1,
        Some(_) => *proven += 1,
    }
    *visited += 1;
    let children = read_children(conn, Some(row.id)).unwrap_or_else(|e| {
        defects.push(format!("node {}: children query failed: {e}", row.id));
        Vec::new()
    });
    for c in &children {
        if c.ply != row.ply + 1 {
            defects.push(format!(
                "node {}: ply {} but its parent is at ply {}",
                c.id, c.ply, row.ply
            ));
        }
    }
    if let Some(m) = mv {
        pos.do_move(m);
    }
    // The node's own legal-reply set (one movegen per node).
    move_list.clear();
    pos.legal_moves_with_state(move_list, state);
    let legal_set: std::collections::BTreeSet<String> = move_list
        .as_slice()
        .iter()
        .map(|&m| move_to_uci(m))
        .collect();
    let child_ucis: std::collections::BTreeSet<&str> = children
        .iter()
        .filter_map(|c| c.move_uci.as_deref())
        .collect();
    let bound = row.bound.unwrap_or(0);
    match row.outcome {
        Some(o) if bound == 0 => {
            // Proven terminal: no children, statically terminal.
            if !children.is_empty() {
                defects.push(format!(
                    "node {} ({}): terminal with {} children",
                    row.id,
                    label(&row.move_uci),
                    children.len()
                ));
            }
            let static_outcome = pos.outcome_from_state(state, move_list);
            if static_outcome != Some(o) {
                defects.push(format!(
                    "node {} ({}): terminal outcome {} but the replayed position is {}",
                    row.id,
                    label(&row.move_uci),
                    o.as_str(),
                    static_outcome.map_or("non-terminal", |x| x.as_str()),
                ));
            }
        }
        Some(Outcome::Win) => {
            // Proven OR: >= 1 proven-loss child; bound = min(loss child)+1.
            let mut min_loss = u32::MAX;
            let mut any_loss = false;
            for c in &children {
                if c.outcome == Some(Outcome::Loss) {
                    any_loss = true;
                    if let Some(b) = c.bound {
                        min_loss = min_loss.min(b.max(0) as u32);
                    }
                }
            }
            if !any_loss {
                defects.push(format!(
                    "node {} ({}): proven win without a proven-loss child",
                    row.id,
                    label(&row.move_uci)
                ));
            } else {
                let expect = min_loss + 1;
                if i64::from(expect) != bound {
                    defects.push(format!(
                        "node {} ({}): bound {bound} but min proven-loss child + 1 = {expect}",
                        row.id,
                        label(&row.move_uci)
                    ));
                }
            }
        }
        Some(Outcome::Loss) => {
            // Proven AND: child set == the legal-reply set, every child win,
            // bound = max(child bound) + 1.
            for uci in legal_set.iter() {
                if !child_ucis.contains(uci.as_str()) {
                    defects.push(format!(
                        "node {} ({}): legal reply {uci} has no child row",
                        row.id,
                        label(&row.move_uci)
                    ));
                }
            }
            for uci in &child_ucis {
                if !legal_set.contains(*uci) {
                    defects.push(format!(
                        "node {} ({}): child {uci} is not a legal reply",
                        row.id,
                        label(&row.move_uci)
                    ));
                }
            }
            let mut max_child = 0i64;
            for c in &children {
                if c.outcome != Some(Outcome::Win) {
                    defects.push(format!(
                        "node {} ({}): AND child {} has outcome {} (expected win)",
                        row.id,
                        label(&row.move_uci),
                        label(&c.move_uci),
                        c.outcome.map_or("open", |x| x.as_str()),
                    ));
                }
                max_child = max_child.max(c.bound.unwrap_or(0));
            }
            if !children.is_empty() && max_child + 1 != bound {
                defects.push(format!(
                    "node {} ({}): bound {bound} but max reply child + 1 = {}",
                    row.id,
                    label(&row.move_uci),
                    max_child + 1
                ));
            }
        }
        Some(Outcome::Draw) => {
            // Unreachable: the reader rejects a draw outcome at parse time.
            defects.push(format!("node {}: draw outcome", row.id));
        }
        None => {}
    }
    // Children of a proven terminal are not descended into (the proof
    // validator's rule — a terminal has no proof structure below it); the
    // child rows above already counted as unreachable unless a defect
    // exists anyway.
    let is_terminal = row.outcome.is_some() && bound == 0;
    if !is_terminal {
        for c in children {
            visit(
                conn, c, pos, state, move_list, defects, proven, open, visited, seen,
            );
        }
    }
    if let Some(m) = mv {
        pos.undo_move(m);
    }
}
