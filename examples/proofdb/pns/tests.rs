//! Breadth-PNS unit tests (plan4 D1): structural numbers, job-set
//! exclusions, and the ledger-child number rules. Included by `pns.rs`;
//! also runs via `tests/proofdb.rs`. The queue state-machine tests (layer
//! discipline, ladder, lineage gate) live in `pns/selector/tests.rs`.

use std::collections::HashSet;

use super::*;
use crate::proofdb::db::{DbContent, DbRow, load_db_rows};
use crate::proofdb::fixture::fixture_db;
use crate::proofdb::ledger::Ledger;
use crate::proofdb::policy::{Policy, jobs_for_policy};
use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::{Outcome, Position};

pub(crate) fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("proofdb_pns_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(crate) fn load_fixture(dir: &std::path::Path) -> (DbContent, std::path::PathBuf) {
    let (db_path, digest) = fixture_db(dir);
    let content = load_db_rows(&db_path, &digest).unwrap();
    (content, db_path)
}

/// A `DbContent` from hand-written rows (path, ply, outcome). Physically
/// implausible outcomes are fine: the numbers logic is structural; proof
/// verification is the merger's job.
pub(crate) fn crafted(rows: &[(&str, usize, Option<Outcome>)]) -> DbContent {
    let rows = rows
        .iter()
        .map(|&(p, ply, o)| DbRow {
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

pub(crate) fn build(db: &DbContent, dir: &std::path::Path) -> Pns {
    Pns::build(
        db,
        Ledger::default(),
        dir.join("ledger.json"),
        BUDGET_PNS_BASE_EVALS,
        crate::proofdb::pns::PnsConfig::default(),
        0,
    )
    .unwrap()
}

#[test]
fn fixture_numbers_and_queue_order() {
    let dir = tmp_dir("numbers");
    let (db, _) = load_fixture(&dir);
    let mut pns = build(&db, &dir);
    let c = &pns.census;
    assert_eq!(c.rows_total, 9);
    assert_eq!(c.open_rows, 5);
    assert_eq!(c.jobs(), 5, "all five open rows are jobs");
    assert_eq!(c.jobs_rows, 5);
    assert_eq!(c.jobs_ledger, 0);
    for e in [
        Exclusion::ProvenAncestor,
        Exclusion::ImpliedWin,
        Exclusion::ImpliedLoss,
    ] {
        assert_eq!(c.excluded_of(e), (0, 0), "{e:?} exclusions");
    }
    // Root (OR): pn = 1, dn = 20 (two row children + 18 unvisited).
    assert_eq!(pns.numbers(0), (1, 20));
    // f2f3 (AND, Black to move): pn = 20, dn = 1; selection number 1.
    let f2f3 = pns.by_key["f2f3"];
    assert_eq!(pns.numbers(f2f3), (20, 1));
    assert_eq!(pns.effective(f2f3), 1);
    // Pop order: (number, ply, path) — root, then the ply-1 rows, then the
    // ply-2 rows.
    let seq: Vec<String> = (0..5).map(|_| pns.pop().unwrap().key).collect();
    assert_eq!(seq, vec!["", "f2f3", "g2g4", "f2f3 e7e6", "g2g4 e7e6"]);
    assert!(pns.pop().is_none(), "queue exhausted");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn exclusions_proven_ancestor_and_implied() {
    let dir = tmp_dir("excl");
    // Open row behind a proven ancestor (sibling-skip, decision 9): the
    // outcome polarity is irrelevant to the exclusion.
    let db = crafted(&[
        ("", 0, None),
        ("f2f3", 1, Some(Outcome::Win)),
        ("f2f3 e7e6", 2, None),
    ]);
    let pns = build(&db, &dir);
    assert_eq!(
        pns.census.excluded_of(Exclusion::ProvenAncestor),
        (1, 0),
        "e7e6 behind the proven f2f3"
    );
    assert_eq!(pns.census.jobs(), 1, "only the root remains");
    assert_eq!(pns.census.open_rows, 2);

    // Implied win (decision 10): an AND node all of whose replies are
    // proven root-player wins has pn = 0 — here f2f3 under twenty
    // proven-win reply rows.
    let mut rows: Vec<(&str, usize, Option<Outcome>)> = vec![("", 0, None), ("f2f3", 1, None)];
    // Black to move after 1.f3: every reply stored as a proven root-player
    // win (even ply + win).
    let pos_after_f3 = {
        let mut p = Position::from_fen(Position::STARTPOS_FEN).unwrap();
        let mv = uci_to_move("f2f3", &p).unwrap();
        p.do_move(mv);
        p
    };
    for mv in pos_after_f3.legal_moves_vec() {
        let uci = move_to_uci(mv);
        rows.push((
            Box::leak(format!("f2f3 {uci}").into_boxed_str()),
            2,
            Some(Outcome::Win),
        ));
    }
    let db = crafted(&rows);
    let mut pns = build(&db, &dir);
    // The implication cascades at session start: f2f3 (AND, all replies
    // proven root-wins) has pn 0, which makes the root's OR-min 0 too.
    assert_eq!(pns.census.excluded_of(Exclusion::ImpliedWin), (2, 0));
    assert_eq!(pns.census.jobs(), 0, "root implied won through f2f3");
    assert_eq!(pns.numbers(pns.by_key[""]), (0, INF));

    // Implied loss: an OR node all of whose children are proven root-player
    // losses has pn = ∞ (every reply refuted).
    let mut rows: Vec<(&str, usize, Option<Outcome>)> = vec![("", 0, None)];
    for mv in Position::from_fen(Position::STARTPOS_FEN)
        .unwrap()
        .legal_moves_vec()
    {
        let uci = move_to_uci(mv);
        rows.push((Box::leak(uci.into_boxed_str()), 1, Some(Outcome::Win)));
    }
    let db = crafted(&rows);
    let pns = build(&db, &dir);
    assert_eq!(pns.census.excluded_of(Exclusion::ImpliedLoss), (1, 0));
    assert_eq!(pns.census.jobs(), 0, "root implied lost");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn ledger_children_flat_and_proven_numbers() {
    let dir = tmp_dir("flat");
    // A ledger record contributes 1 + passes (flat) to its parent's
    // formula; a proven root-loss child makes an OR parent's dn ∞.
    let (db, _) = load_fixture(&dir);
    let ledger_path = dir.join("ledger.json");
    std::fs::write(
        &ledger_path,
        r#"{"records":[{"path":["f2f3","e7e6","a2a3"],"work_done":0,"passes_failed":3}]}"#,
    )
    .unwrap();
    let ledger = Ledger::load(&ledger_path).unwrap();
    let mut pns = Pns::build(
        &db,
        ledger,
        ledger_path,
        BUDGET_PNS_BASE_EVALS,
        crate::proofdb::pns::PnsConfig::default(),
        0,
    )
    .unwrap();
    assert_eq!(pns.census.ledger_records, 1);
    assert_eq!(pns.census.jobs_ledger, 1, "the ledger node is a job");
    let rec = pns.by_key["f2f3 e7e6 a2a3"];
    assert_eq!(pns.nodes[rec].passes, 3);
    assert_eq!(pns.effective(rec), 4);
    // The record's parent (f2f3 e7e6, OR) keeps pn 1 (min over children);
    // f2f3's pn still sums to 20 (the record replaced one unvisited child of
    // its child, not of itself).
    let f2f3 = pns.by_key["f2f3"];
    let (pn, dn) = pns.numbers(f2f3);
    assert_eq!((pn, dn), (20, 1));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn policy_breadth_pns_names_and_empty_sequence() {
    assert_eq!(Policy::BreadthPns.as_str(), "breadth-pns");
    assert_eq!(Policy::parse("breadth-pns").unwrap(), Policy::BreadthPns);
    let dir = tmp_dir("policy");
    let (db_path, digest) = fixture_db(&dir);
    let f = crate::proofdb::frontier::extract_frontier(&db_path, &digest, &HashSet::new()).unwrap();
    assert!(jobs_for_policy(Policy::BreadthPns, &f).is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rebuild_is_deterministic() {
    let dir = tmp_dir("det");
    let (db, _) = load_fixture(&dir);
    let mut seqs = Vec::new();
    for k in 0..2 {
        let mut pns = build(&db, &dir);
        let _ = k;
        let seq: Vec<String> = pns.pop().into_iter().map(|s| s.key).collect();
        seqs.push(format!("{seq:?}"));
    }
    assert_eq!(seqs[0], seqs[1], "same DB + same ledger → same sequence");
    std::fs::remove_dir_all(&dir).ok();
}
