//! Decision-function tests (plan6 D1): the degenerate-config equivalence to
//! the plan4 selector and the eligibility edges (incl. the
//! last-virgin-child transition). The pacing/rationing tests (reserve,
//! rung cap, rotation, fallback, layer caps, growth) live in the sibling
//! `tests_pacing` module. Included by `decision.rs`; also runs via
//! `tests/proofdb.rs`. Fixture helpers from the parent tree's test module.
//!
//! Test-tree convention: the root row + legal replies as ledger records
//! (passes ≥ 1 = censored = non-virgin; passes 0 = exposed, never visited =
//! virgin). Censoring a node exposes its unrecorded children one ply
//! deeper (the standing cascade the real sessions run on).

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};

use super::super::super::tests::{crafted, tmp_dir};
use super::super::super::{BUDGET_PNS_BASE_EVALS, Pns, PnsConfig};
use crate::proofdb::ledger::Ledger;
use crate::proofdb::pns::config::{self, Rotation};
use crate::proofdb::pns::selector::VisitKind;

/// Build with a config + session cap over a hand-written ledger
/// (`(path, work_done, passes_failed)` records).
pub(super) fn build_with(
    rows: &[(&str, usize, Option<Outcome>)],
    records: &[(&str, u64, u32)],
    cfg: PnsConfig,
    session_cap: u64,
    dir: &std::path::Path,
) -> Pns {
    let db = crafted(rows);
    let mut json = String::from("{\"records\":[");
    for (i, &(p, w, k)) in records.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        let path_json = if p.is_empty() {
            "[]".to_string()
        } else {
            serde_json::to_string(&p.split(' ').collect::<Vec<&str>>()).expect("paths serialize")
        };
        json.push_str(&format!(
            "{{\"path\":{path_json},\"work_done\":{w},\"passes_failed\":{k}}}"
        ));
    }
    json.push_str("]}\n");
    let path = dir.join("ledger_in.json");
    std::fs::write(&path, json).unwrap();
    let ledger = Ledger::load(&path).unwrap();
    Pns::build(
        &db,
        ledger,
        dir.join("ledger.json"),
        BUDGET_PNS_BASE_EVALS,
        cfg,
        session_cap,
    )
    .unwrap()
}

/// Root row + every legal reply as a ledger record at `passes` (the root
/// censored at passes 1 with 4M work).
pub(super) fn root_with_all_children_records(passes: u32) -> Vec<(&'static str, u64, u32)> {
    let mut recs: Vec<(&'static str, u64, u32)> = vec![("", 4_000_000, 1)];
    let pos = Position::from_fen(Position::STARTPOS_FEN).unwrap();
    for mv in pos.legal_moves_vec() {
        let uci: &'static str = Box::leak(move_to_uci(mv).into_boxed_str());
        recs.push((uci, 4_000_000, passes));
    }
    recs
}

/// Root row + every legal reply as an open row (no records at all).
pub(super) fn startpos_rows() -> Vec<(&'static str, usize, Option<Outcome>)> {
    let mut rows: Vec<(&'static str, usize, Option<Outcome>)> = vec![("", 0, None)];
    let pos = Position::from_fen(Position::STARTPOS_FEN).unwrap();
    for mv in pos.legal_moves_vec() {
        let uci: &'static str = Box::leak(move_to_uci(mv).into_boxed_str());
        rows.push((uci, 1, None));
    }
    rows
}

pub(super) fn legal_ucis() -> Vec<String> {
    Position::from_fen(Position::STARTPOS_FEN)
        .unwrap()
        .legal_moves_vec()
        .into_iter()
        .map(move_to_uci)
        .collect()
}

/// The ledger records for every legal reply at `prefix`'s position (4M
/// work each, at `passes`).
pub(super) fn reply_records(prefix: &str, passes: u32) -> Vec<(String, u64, u32)> {
    let path: Vec<String> = if prefix.is_empty() {
        Vec::new()
    } else {
        prefix.split(' ').map(str::to_string).collect()
    };
    crate::proofdb::harvest::replay_job_path(&path)
        .expect("test prefix replays")
        .0
        .legal_moves_vec()
        .into_iter()
        .map(|mv| {
            (
                if prefix.is_empty() {
                    move_to_uci(mv)
                } else {
                    format!("{prefix} {}", move_to_uci(mv))
                },
                4_000_000,
                passes,
            )
        })
        .collect()
}

#[test]
fn degenerate_config_equals_legacy_selector() {
    let dir = tmp_dir("degenerate");
    let (db, _) = crate::proofdb::pns::tests::load_fixture(&dir);
    // Two builds over separate dirs: the plan4 selector (`pop`) vs the
    // mechanism's decision function under the degenerate config.
    let ldir = dir.join("legacy");
    let mdir = dir.join("mech");
    std::fs::create_dir_all(&ldir).unwrap();
    std::fs::create_dir_all(&mdir).unwrap();
    let mut legacy = crate::proofdb::pns::tests::build(&db, &ldir);
    let mut mech = crate::proofdb::pns::tests::build(&db, &mdir);
    mech.pacing.cfg = config::degenerate();
    // Drive both through a censor-heavy prefix: censor everything popped,
    // 60 visits (fresh exposures keep the queue alive well past that).
    for i in 0..60 {
        let Some(a) = legacy.pop() else {
            panic!("legacy queue drained early at {i}");
        };
        let Some(b) = mech.next().unwrap() else {
            panic!("mechanism queue drained early at {i}");
        };
        assert_eq!(
            (b.key.clone(), b.pass, b.number, b.work_before, b.budget),
            (a.key.clone(), a.pass, a.number, a.work_before, a.budget),
            "degenerate config must equal the plan4 selector at visit {i}"
        );
        assert_eq!(b.kind, VisitKind::Expand);
        legacy.on_censored(&a.key, 1_000).unwrap();
        mech.on_censored(&b.key, 1_000).unwrap();
    }
    // Identical censor streams → identical ledger bytes (deterministic
    // sorted serialization).
    assert_eq!(
        std::fs::read(ldir.join("ledger.json")).unwrap(),
        std::fs::read(mdir.join("ledger.json")).unwrap(),
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn eligibility_edges_and_last_virgin_transition() {
    let dir = tmp_dir("elig");
    // Root censored (passes 1); all 20 replies censored except one virgin
    // (a fresh, never-visited record at passes 0).
    let mut recs = root_with_all_children_records(1);
    recs[1].2 = 0;
    let mut pns = build_with(
        &startpos_rows(),
        &recs,
        config::mechanism_defaults(),
        300_000_000,
        &dir,
    );
    // V=0 is a rung slot, but the root is not eligible (virgin child) →
    // the first visit is an expansion (the number-1 virgin reply, ply 1).
    let v1 = pns.next().unwrap().unwrap();
    assert_eq!(v1.kind, VisitKind::Expand, "virgin child blocks the rung");
    assert_eq!(v1.key, "a2a3", "the virgin reply is the number-1 min");
    assert_eq!((v1.pass, v1.number, v1.work_before), (1, 1, 4_000_000));
    pns.on_censored(&v1.key, 1_000).unwrap();
    // The root flipped to eligible (its last virgin child was visited),
    // but V=1 is not a rung slot (interleave 4) and a2a3's censor exposed
    // fresh ply-2 records (number 1) → expansion continues.
    for i in 1..4 {
        let v = pns.next().unwrap().unwrap();
        assert_eq!(v.kind, VisitKind::Expand, "V={i} expands");
        assert_eq!(v.ply, 2, "the fresh ply-2 sublayer");
        assert_eq!((v.pass, v.number), (1, 1));
        pns.on_censored(&v.key, 1_000).unwrap();
    }
    // V=4: the rung slot fires on the now-eligible root (the only eligible
    // node: every reply has either a virgin child or a rung-blocking one).
    let v5 = pns.next().unwrap().unwrap();
    assert_eq!(v5.kind, VisitKind::Rung, "V=4 rung slot, root eligible");
    assert_eq!(v5.key, "", "fewest-passes: the root is the only eligible");
    assert_eq!(v5.pass, 2, "the rung is the root's second visit");
    assert_eq!(v5.budget, 2 * BUDGET_PNS_BASE_EVALS, "geometric rung 2");
    assert_eq!(
        v5.number, 20,
        "root eff = OR-min over reply-row pn (20) — the \
     ladder ignores numbers; the census field reports the selection-time eff"
    );
    pns.on_censored(&v5.key, 1_000).unwrap();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rotation_modes_disagree() {
    let dir = tmp_dir("rotation");
    // Two eligible ledger nodes with disagreeing sort keys: Y = "b1a3"
    // (AND, ply 1): passes 2, children at passes 1 → dn 2 → number 3;
    // X = "a2a3 b7b5" (OR, ply 2): passes 1, children at passes 4 → OR-min
    // 5 → number 5. fewest-passes picks X; number-ply-path picks Y.
    let recs: Vec<(String, u64, u32)> = reply_records("", 4)
        .into_iter()
        .map(|(p, w, _)| {
            let k = if p == "b1a3" {
                2
            } else if p == "a2a3" {
                1
            } else {
                4
            };
            (p, w, k)
        })
        .chain(reply_records("b1a3", 1))
        .chain(reply_records("a2a3 b7b5", 4))
        .chain(std::iter::once(("a2a3 b7b5".to_string(), 4_000_000, 1)))
        .collect();
    let recs: Vec<(&'static str, u64, u32)> = recs
        .into_iter()
        .map(|(p, w, k)| (Box::leak(p.into_boxed_str()) as &'static str, w, k))
        .collect();

    let cfg = |rotation| PnsConfig {
        rotation,
        ..config::mechanism_defaults()
    };
    let mut pns = build_with(
        &startpos_rows(),
        &recs,
        cfg(Rotation::NumberPlyPath),
        300_000_000,
        &dir,
    );
    let v1 = pns.next().unwrap().unwrap();
    assert_eq!(
        v1.kind,
        VisitKind::Rung,
        "the V=0 slot fires on eligibility"
    );
    assert_eq!(
        v1.key, "b1a3",
        "number-ply-path: Y (number 3) < X (number 5)"
    );
    assert_eq!(v1.number, 3);
    assert_eq!(v1.pass, 3, "Y's second rung");
    pns.on_censored(&v1.key, 1_000).unwrap();

    let mut pns = build_with(
        &startpos_rows(),
        &recs,
        cfg(config::mechanism_defaults().rotation),
        300_000_000,
        &dir,
    );
    let v1 = pns.next().unwrap().unwrap();
    assert_eq!(v1.kind, VisitKind::Rung);
    assert_eq!(
        v1.key, "a2a3 b7b5",
        "fewest-passes: X passes 1 < Y passes 2"
    );
    assert_eq!(v1.pass, 2);
    assert_eq!(v1.number, 5, "X's structural OR-min = 5");
    std::fs::remove_dir_all(&dir).ok();
}
