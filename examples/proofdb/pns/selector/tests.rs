//! Selector state-machine tests (plan4 D1): the live queue's layer
//! discipline, the geometric ladder, and the ledger lineage gate end to
//! end. Included by `selector.rs`; also runs via `tests/proofdb.rs`. The
//! number/job-set classification tests live in the parent module's tests.

use super::super::BUDGET_PNS_BASE_EVALS;
use super::super::tests::{build as pns_build, load_fixture, tmp_dir};
use crate::proofdb::ledger::Ledger;

#[test]
fn ladder_and_exposure_layer_discipline() {
    let dir = tmp_dir("ladder");
    let (db, _) = load_fixture(&dir);
    let mut pns = pns_build(&db, &dir);
    let base = BUDGET_PNS_BASE_EVALS;

    // Pop the root and censor it: its 18 unvisited replies become fresh
    // ledger records entering the queue at number 1, the root re-enters at
    // 2 — the freshly exposed layer is explored before the doubled revisit.
    let root = pns.pop().unwrap();
    assert_eq!(root.key, "");
    assert_eq!((root.pass, root.number, root.work_before), (1, 1, 0));
    assert_eq!(root.budget, base);
    pns.on_censored("", 1_000).unwrap();

    let root_entry = pns.ledger.get("").copied().unwrap();
    assert_eq!(
        root_entry,
        crate::proofdb::ledger::LedgerEntry {
            work_done: 1_000,
            passes_failed: 1,
        }
    );
    assert_eq!(
        pns.census.jobs_ledger, 18,
        "root's unvisited replies exposed"
    );
    // The ledger file was saved with the bump + exposure in one write.
    let saved = std::fs::read_to_string(dir.join("ledger.json")).unwrap();
    assert_eq!(saved.matches("\"passes_failed\":0").count(), 18);
    assert!(saved.contains("\"passes_failed\":1"));
    assert!(saved.starts_with("{\"records\":["), "deterministic bytes");
    assert!(saved.ends_with("]}\n"));

    // Every number-1 node pops before the root's number-2 revisit: the 18
    // fresh children plus the two open rows f2f3/g2g4 (all number 1, ply 1,
    // interleaved by path order). (The fresh children are popped without
    // censoring them — censoring would expose their own replies and keep
    // the queue at number 1 forever, which is the intended breadth-first
    // dynamic the session cap bounds.)
    let mut fresh = 0;
    let mut rows = 0;
    loop {
        let s = pns.pop().unwrap();
        assert_eq!(s.number, 1, "root revisits only after its exposed layer");
        assert_eq!((s.pass, s.budget), (1, base));
        assert_ne!(s.key, "");
        if s.from_ledger {
            fresh += 1;
        } else {
            rows += 1;
        }
        if fresh == 18 && rows == 2 {
            break;
        }
    }
    assert_eq!(rows, 2, "f2f3 and g2g4 are the open-row jobs");
    // The initial ply-2 jobs are number-1 nodes as well; only after them
    // does the root's number-2 revisit come up.
    loop {
        let s = pns.pop().unwrap();
        if s.key.is_empty() {
            assert_eq!((s.pass, s.number, s.work_before), (2, 2, 1_000));
            assert_eq!(s.budget, base * 2);
            break;
        }
        assert_eq!(s.number, 1, "number-1 nodes first: got {:?}", s.key);
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn lineage_gate_drops_decided_and_aborts_illegal() {
    let dir = tmp_dir("gate");
    let (db, _) = load_fixture(&dir);

    // Record on a proven row → dropped; on an open row → kept with its
    // censor history (raises the effective number); illegal path → abort.
    let ledger_path = dir.join("ledger.json");
    std::fs::write(
        &ledger_path,
        r#"{"records":[
            {"path":["f2f3","e7e6","g2g4"],"work_done":4000000,"passes_failed":1},
            {"path":["f2f3"],"work_done":8000000,"passes_failed":2}
        ]}"#,
    )
    .unwrap();
    let ledger = Ledger::load(&ledger_path).unwrap();
    let mut pns = crate::proofdb::pns::Pns::build(
        &db,
        ledger,
        ledger_path.clone(),
        BUDGET_PNS_BASE_EVALS,
        crate::proofdb::pns::PnsConfig::default(),
        0,
    )
    .unwrap();
    assert_eq!(pns.census.ledger_dropped, 1, "decided path dropped");
    assert!(!pns.ledger.contains("f2f3 e7e6 g2g4"));
    assert!(pns.ledger.contains("f2f3"));
    let f2f3 = pns.by_key["f2f3"];
    assert_eq!(pns.nodes[f2f3].passes, 2);
    assert_eq!(pns.effective(f2f3), 3, "eff = 1 + passes_failed");
    // Pop order now: root (1), the untouched number-1 nodes, then f2f3 (3).
    let first = pns.pop().unwrap();
    assert_eq!(first.key, "");
    loop {
        let s = pns.pop().unwrap();
        if s.key == "f2f3" {
            assert_eq!((s.pass, s.number, s.work_before), (3, 3, 8_000_000));
            assert_eq!(s.budget, BUDGET_PNS_BASE_EVALS * 4, "third rung");
            break;
        }
    }

    let ledger_path2 = dir.join("ledger2.json");
    std::fs::write(
        &ledger_path2,
        r#"{"records":[{"path":["e7e5","a1a1"],"work_done":0,"passes_failed":0}]}"#,
    )
    .unwrap();
    let ledger = Ledger::load(&ledger_path2).unwrap();
    let err = match crate::proofdb::pns::Pns::build(
        &db,
        ledger,
        ledger_path2,
        BUDGET_PNS_BASE_EVALS,
        crate::proofdb::pns::PnsConfig::default(),
        0,
    ) {
        Err(e) => e,
        Ok(_) => panic!("illegal ledger path must abort"),
    };
    assert!(
        err.contains("does not replay legally"),
        "illegal ledger path aborts: {err}"
    );
    std::fs::remove_dir_all(&dir).ok();
}
