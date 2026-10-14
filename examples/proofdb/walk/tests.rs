//! Unit tests for the `frontier-walk` selection machinery (plan16 task 1):
//! the pick order + tie-break, the D2 drop rules, the D3 handled rule and
//! its underworked-ledger-child case, the census line + root states, and
//! the live frontier mutation (admit / complete / decided). Driver-level
//! tests (censor growth, the complete rule end-to-end, the fresh-layer
//! boot) live in `tests/proofdb.rs`, which includes this module tree
//! verbatim.

use super::super::db::DbContent;
use super::*;
use crate::proofdb::ledger::LedgerEntry;
use atomic_solver::position::Position;

/// A crafted DB content: `(space-joined path, ply, outcome)` rows that all
/// replay legally by construction (paths are legal move sequences only if
/// the test says so — the walk reads them as data).
fn crafted(rows: &[(&str, usize, Option<Outcome>)]) -> DbContent {
    let rows = rows
        .iter()
        .map(|&(p, ply, o)| super::super::db::DbRow {
            path: if p.is_empty() {
                Vec::new()
            } else {
                p.split(' ').map(str::to_string).collect()
            },
            ply,
            outcome: o,
            depth_bound: o.map(|_| 1),
        })
        .collect();
    DbContent {
        root_fen: Position::STARTPOS_FEN.to_string(),
        rows,
    }
}

fn ledger_of(records: &[(&str, u64, u32)]) -> Ledger {
    let mut m = std::collections::BTreeMap::new();
    for (key, work, passes) in records {
        let inserted = m.insert(
            (*key).to_string(),
            LedgerEntry {
                work_done: *work,
                passes_failed: *passes,
            },
        );
        assert!(inserted.is_none(), "duplicate test key {key:?}");
    }
    Ledger::from_entries(m)
}

#[test]
fn walk_selection_order_and_tie_break() {
    // Root record (ply 0), a ply-1 lex tie-break, a ply-2 record.
    let ledger = ledger_of(&[
        ("a2a3 b1b2", 100, 1),
        ("b1c3", 100, 1),
        ("a2a3", 100, 1),
        ("", 100, 1),
    ]);
    let db = crafted(&[("", 0, None)]);
    let (mut f, census) = WalkFrontier::new(&ledger, &db, "");
    assert_eq!(census.records, 4);
    assert_eq!((census.lineage, census.ancestor), (0, 0));
    // Shallowest first, path-lex tie-break (D2).
    assert_eq!(f.pick(&ledger).unwrap().0, "");
    f.mark_decided("");
    assert_eq!(f.pick(&ledger).unwrap().0, "a2a3");
    f.mark_decided("a2a3");
    assert_eq!(f.pick(&ledger).unwrap().0, "b1c3");
    f.mark_decided("b1c3");
    assert_eq!(f.pick(&ledger).unwrap().0, "a2a3 b1b2");
    f.mark_decided("a2a3 b1b2");
    assert!(f.pick(&ledger).is_none());
}

#[test]
fn walk_lineage_and_ancestor_drops() {
    // Proven rows: "a2a3" (the record's own path → lineage) and the root's
    // open row stays unproven. The record behind the proven row drops by
    // ancestor; unrelated records survive.
    let db = crafted(&[
        ("", 0, None),
        ("a2a3", 1, Some(Outcome::Win)),
        ("a2a4", 1, None),
    ]);
    let ledger = ledger_of(&[
        ("", 0, 1),
        ("a2a3", 100, 1),
        ("a2a3 b1b2", 100, 1),
        ("a2a4", 100, 1),
        ("b1c3", 100, 1),
    ]);
    let (f, census) = WalkFrontier::new(&ledger, &db, "");
    assert_eq!((census.records, census.lineage, census.ancestor), (5, 1, 1));
    // Survivors: the root record, the open-row sibling, the unrelated one.
    let mut f = f;
    let mut picked = Vec::new();
    for _ in 0..3 {
        let (k, _) = f.pick(&ledger).unwrap();
        f.mark_decided(&k);
        picked.push(k);
    }
    assert_eq!(picked, vec!["", "a2a4", "b1c3"]);
}

#[test]
fn walk_proven_root_row_drops_everything() {
    // A proven root row is an ancestor of every record (the empty prefix
    // counts): the whole ledger drops and the walk is exhausted (D2/D6).
    let db = crafted(&[("", 0, Some(Outcome::Win))]);
    let ledger = ledger_of(&[("", 0, 1), ("a2a3", 100, 1), ("a2a3 b1b2", 100, 1)]);
    let (f, census) = WalkFrontier::new(&ledger, &db, "");
    assert_eq!((census.lineage, census.ancestor), (1, 2));
    assert!(f.pick(&ledger).is_none());
}

#[test]
fn walk_handled_rule_categories() {
    let db = crafted(&[
        ("", 0, None),
        ("a2a3", 1, None),               // stored open row → handled
        ("b1c3", 1, Some(Outcome::Win)), // stored proven row → handled
    ]);
    let rows = WalkFrontier::rows_map(&db);
    let ledger = ledger_of(&[
        ("c2c3", 500, 1),   // underworked record → NOT handled (D3)
        ("d2d4", 1_000, 3), // record at the budget → handled
    ]);
    let shard = |k: &str| k == "f2f4"; // the manifest ∪ in-session shard part
    let h = |k: &str| is_handled(k, &rows, &ledger, 1_000, shard);
    assert!(h("a2a3"), "stored open row is handled (any outcome)");
    assert!(h("b1c3"), "stored proven row is handled");
    assert!(!h("c2c3"), "an underworked ledger child is a target (D3)");
    assert!(h("d2d4"), "a ledger record at work ≥ B is handled");
    assert!(!h("e2e4"), "no row, no record, no shard → unhandled");
    assert!(h("f2f4"), "a manifest/in-session shard path is handled");
}

#[test]
fn walk_frontier_admit_and_live_work() {
    let db = crafted(&[("", 0, None), ("a2a3", 1, Some(Outcome::Win))]);
    let ledger = ledger_of(&[("", 0, 1)]);
    let (mut f, census) = WalkFrontier::new(&ledger, &db, "");
    assert_eq!(census.records, 1);
    // The root record is the pick (ply 0); retire it, then admit a fresh
    // record: it becomes a candidate (ply 1 here).
    assert_eq!(f.pick(&ledger).unwrap().0, "");
    f.mark_decided("");
    f.admit("b1c3");
    assert_eq!(f.pick(&ledger).unwrap().0, "b1c3");
    // Admitting a lineage-dropped key is a no-op.
    f.admit("a2a3");
    // Live work: bump the record and pick again — the new work shows.
    let ledger2 = ledger_of(&[("b1c3", 5_000, 2)]);
    assert_eq!(f.pick(&ledger2).unwrap(), ("b1c3".to_string(), 5_000));
    // Complete-marking retires the candidate and counts.
    f.mark_complete("b1c3");
    assert!(f.pick(&ledger2).is_none());
    assert_eq!(f.completes, 1);
    // Double-marking neither duplicates nor recounts.
    f.mark_complete("b1c3");
    assert_eq!(f.completes, 1);
}

#[test]
fn walk_census_line_and_root_states() {
    use super::super::session::Session;
    let dir = std::env::temp_dir().join(format!("proofdb_walk_census_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Open root (the fresh-layer bootstrap shape).
    let db = crafted(&[("", 0, None), ("a2a3", 1, Some(Outcome::Win))]);
    let session = Session::new(
        4,
        dir.join("s"),
        dir.join("s/manifest.json"),
        Vec::new(),
        "frontier-walk",
    );
    let (f, census) = WalkFrontier::new(&Ledger::default(), &db, "");
    let mut census = census;
    census.root = root_state(&db, &session, "");
    assert_eq!(census.root, RootState::Open);
    assert_eq!(
        census.describe(4_000_000),
        "frontier-walk: records 0 (lineage 0, ancestor 0, out_of_scope 0, complete 0) \
         — root open; base budget 4000000; scope global"
    );
    assert_eq!(f.completes, 0);
    // Proven root row → proven; a manifest shard path at the root is
    // proven-stored even over an open row.
    let db_proven = crafted(&[("", 0, Some(Outcome::Loss))]);
    assert_eq!(root_state(&db_proven, &session, ""), RootState::Proven);
    let session_shard = Session::new(
        4,
        dir.join("s2"),
        dir.join("s2/manifest.json"),
        vec![crate::proofdb::ShardEntry {
            tag: "root".to_string(),
            file: "root.bin".to_string(),
            fen: Position::STARTPOS_FEN.to_string(),
            moves: Vec::new(),
            outcome: Outcome::Win,
            validate: "ok".to_string(),
        }],
        "frontier-walk",
    );
    assert_eq!(root_state(&db, &session_shard, ""), RootState::Proven);
    // Fresh root: no root row at all (pathological, but pinned).
    let db_empty = crafted(&[]);
    assert_eq!(root_state(&db_empty, &session, ""), RootState::Fresh);
    std::fs::remove_dir_all(&dir).ok();
}

// --- plan19: the scoped walk (`--moves` re-anchor) ---

#[test]
fn walk_scope_candidacy_filtering_and_ordering() {
    // Scope "a2a3": in-scope records are "a2a3" (the scope root itself,
    // relative ply 0) and "a2a3 b1b2"; everything else is out of scope and
    // never pickable. The scope root record's live work is respected.
    let ledger = ledger_of(&[
        ("", 100, 1),
        ("b1c3", 100, 1),
        ("a2a3", 5_000, 1),
        ("a2a3 b1b2", 100, 1),
    ]);
    let db = crafted(&[("", 0, None)]);
    let (mut f, census) = WalkFrontier::new(&ledger, &db, "a2a3");
    assert_eq!(census.records, 4);
    assert_eq!(census.out_of_scope, 2);
    assert_eq!((census.lineage, census.ancestor), (0, 0));
    assert_eq!(census.scope, "a2a3");
    assert_eq!(f.pick(&ledger).unwrap(), ("a2a3".to_string(), 5_000));
    f.mark_decided("a2a3");
    assert_eq!(f.pick(&ledger).unwrap().0, "a2a3 b1b2");
    f.mark_decided("a2a3 b1b2");
    assert!(f.pick(&ledger).is_none());
}

#[test]
fn walk_scope_drops_are_relative_and_ignore_above_scope() {
    // Proven rows: the game root "" (above the scope — ignored, the
    // operator chose the scope deliberately) and "a2a3 b1b2 c2c3" (in
    // scope). Scope "a2a3": the record at the deep proven row drops by
    // lineage (relative), the record behind it by ancestor, the records
    // between survive; out-of-scope records are counted, not visited.
    let db = crafted(&[
        ("", 0, Some(Outcome::Win)),
        ("a2a3", 1, None),
        ("a2a3 b1b2 c2c3", 3, Some(Outcome::Loss)),
    ]);
    let ledger = ledger_of(&[
        ("", 100, 1),
        ("b1c3", 100, 1),
        ("a2a3", 100, 1),
        ("a2a3 b1b2", 100, 1),
        ("a2a3 b1b2 c2c3", 100, 1),
        ("a2a3 b1b2 c2c3 d2d3", 100, 1),
    ]);
    let (mut f, census) = WalkFrontier::new(&ledger, &db, "a2a3");
    assert_eq!(census.records, 6);
    assert_eq!(census.out_of_scope, 2);
    assert_eq!((census.lineage, census.ancestor), (1, 1));
    let mut picked = Vec::new();
    for _ in 0..2 {
        let (k, _) = f.pick(&ledger).unwrap();
        f.mark_decided(&k);
        picked.push(k);
    }
    assert_eq!(picked, vec!["a2a3", "a2a3 b1b2"]);
    assert!(f.pick(&ledger).is_none());
}

#[test]
fn walk_scope_proven_scope_root_drops_everything_in_scope() {
    // A proven row at the scope key is the relative root: everything in
    // scope drops (the empty relative prefix counts), out-of-scope records
    // survive untouched for later global sessions.
    let db = crafted(&[("", 0, None), ("a2a3", 1, Some(Outcome::Win))]);
    let ledger = ledger_of(&[
        ("", 0, 1),
        ("a2a3", 100, 1),
        ("a2a3 b1b2", 100, 1),
        ("b1c3", 100, 1),
    ]);
    let (f, census) = WalkFrontier::new(&ledger, &db, "a2a3");
    assert_eq!(census.records, 4);
    assert_eq!(census.out_of_scope, 2);
    assert_eq!((census.lineage, census.ancestor), (1, 1));
    assert!(f.pick(&ledger).is_none());
}

#[test]
fn walk_scope_root_states_and_census_line() {
    use super::super::session::Session;
    let dir = std::env::temp_dir().join(format!("proofdb_walk_scope_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let session = Session::new(
        4,
        dir.join("s"),
        dir.join("s/manifest.json"),
        Vec::new(),
        "frontier-walk",
    );
    // The scope root row decides the state (open / proven); no row at the
    // scope key is a fresh scope (normal — the game-root case is the
    // pathological one).
    let db_open = crafted(&[("", 0, None), ("a2a3 b1b2", 2, None)]);
    assert_eq!(root_state(&db_open, &session, "a2a3 b1b2"), RootState::Open);
    assert_eq!(root_state(&db_open, &session, "a2a3"), RootState::Fresh);
    let db_proven = crafted(&[("", 0, None), ("a2a3 b1b2", 2, Some(Outcome::Loss))]);
    assert_eq!(
        root_state(&db_proven, &session, "a2a3 b1b2"),
        RootState::Proven
    );
    // A manifest/in-session shard path at the scope key is proven-stored.
    let session_shard = Session::new(
        4,
        dir.join("s2"),
        dir.join("s2/manifest.json"),
        vec![crate::proofdb::ShardEntry {
            tag: "scope".to_string(),
            file: "scope.bin".to_string(),
            fen: Position::STARTPOS_FEN.to_string(),
            moves: vec!["a2a3".to_string(), "b1b2".to_string()],
            outcome: Outcome::Win,
            validate: "ok".to_string(),
        }],
        "frontier-walk",
    );
    assert_eq!(
        root_state(&db_open, &session_shard, "a2a3 b1b2"),
        RootState::Proven
    );
    // The scoped census line shape (S3).
    let ledger = ledger_of(&[("a2a3 b1b2", 100, 1), ("b1c3", 100, 1)]);
    let (_, mut census) = WalkFrontier::new(&ledger, &db_open, "a2a3 b1b2");
    census.root = root_state(&db_open, &session, "a2a3 b1b2");
    assert_eq!(
        census.describe(1_000),
        "frontier-walk: records 2 (lineage 0, ancestor 0, out_of_scope 1, \
         complete 0) — root open; base budget 1000; scope a2a3 b1b2"
    );
    std::fs::remove_dir_all(&dir).ok();
}
