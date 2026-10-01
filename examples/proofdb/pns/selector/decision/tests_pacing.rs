//! Pacing/rationing tests (plan6 D1): reserve, rung cap, rotation,
//! fallback drain, layer caps, `eligibility = "always"`, growth modes.
//! Included by `decision.rs`; also runs via `tests/proofdb.rs`. Shared
//! fixtures/helpers in the sibling `tests` module.

use super::super::super::tests::tmp_dir;
use super::super::super::{BUDGET_PNS_BASE_EVALS, PnsConfig};
use super::super::VisitKind;
use super::tests::{build_with, legal_ucis, root_with_all_children_records, startpos_rows};
use crate::proofdb::pns::config::{self, Eligibility, RungGrowth};

#[test]
fn reserve_exhaustion_routes_to_expansion() {
    let dir = tmp_dir("reserve");
    let recs = root_with_all_children_records(1);
    // Cap 64M → reserve 16M: the rungs at 8M + 16M exhaust it; further
    // rung slots route to expansion.
    let mut pns = build_with(
        &startpos_rows(),
        &recs,
        config::mechanism_defaults(),
        64_000_000,
        &dir,
    );
    let v1 = pns.next().unwrap().unwrap();
    assert_eq!(v1.kind, VisitKind::Rung);
    assert_eq!(v1.key, "", "the root is the only eligible node");
    assert_eq!(v1.budget, 2 * BUDGET_PNS_BASE_EVALS, "rung 2 = 2^1 × base");
    pns.on_censored(&v1.key, 8_000_000).unwrap();
    pns.charge_reserve(8_000_000);
    assert_eq!(pns.pacing.reserve_spent, 8_000_000);
    // V=1..3: expansions (censored replies at number 2, then the fresh
    // ply-2/3 records their censors exposed).
    for i in 1..4 {
        let v = pns.next().unwrap().unwrap();
        assert_eq!(v.kind, VisitKind::Expand, "V={i} expands");
        pns.on_censored(&v.key, 1_000).unwrap();
    }
    // V=4: a rung slot — the root's second rung (pass 3) at 16M → the
    // reserve hits 24M ≥ 16M → afterwards expansion only.
    let v5 = pns.next().unwrap().unwrap();
    assert_eq!(v5.kind, VisitKind::Rung);
    assert_eq!(v5.budget, 4 * BUDGET_PNS_BASE_EVALS, "rung 3 = 2^2 × base");
    pns.on_censored(&v5.key, 16_000_000).unwrap();
    pns.charge_reserve(16_000_000);
    assert_eq!(pns.pacing.reserve_spent, 24_000_000);
    let v6 = pns.next().unwrap().unwrap();
    assert_eq!(v6.kind, VisitKind::Expand, "reserve exhausted → expansion");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn max_rung_passes_caps_the_ladder() {
    let dir = tmp_dir("rungcap");
    let recs = root_with_all_children_records(1);
    // max_rung_passes = 1: only one rung per node per session.
    let mut pns = build_with(
        &startpos_rows(),
        &recs,
        PnsConfig {
            max_rung_passes: 1,
            ..config::mechanism_defaults()
        },
        300_000_000,
        &dir,
    );
    let v1 = pns.next().unwrap().unwrap();
    assert_eq!(v1.kind, VisitKind::Rung);
    assert_eq!(v1.key, "");
    pns.on_censored(&v1.key, 1_000).unwrap();
    pns.charge_reserve(1_000);
    // The root is rung-capped now; the replies (censored at passes 1) all
    // have unexpanded children → virgin → not eligible. Expansions only,
    // including at the V=4 rung slot.
    for _i in 0..4 {
        let v = pns.next().unwrap().unwrap();
        assert_eq!(v.kind, VisitKind::Expand, "no second rung on the root");
        pns.on_censored(&v.key, 1_000).unwrap();
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn fallback_drain_when_no_expansion_possible() {
    let dir = tmp_dir("fallback");
    let recs = root_with_all_children_records(1);
    let mut pns = build_with(
        &startpos_rows(),
        &recs,
        config::mechanism_defaults(),
        300_000_000,
        &dir,
    );
    // V=0: the rung slot fires first (root eligible).
    let v1 = pns.next().unwrap().unwrap();
    assert_eq!(v1.kind, VisitKind::Rung);
    pns.on_censored(&v1.key, 1_000).unwrap();
    // Decide every queued reply by key (simulating decisive outcomes);
    // the queue's only live entry is the root (re-inserted at number 3).
    for uci in legal_ucis() {
        pns.on_decided(&uci);
    }
    // Ration ply 0 to one visit and spend its slot (white-box: layer 0
    // never gets an expansion visit while number-1 nodes exist).
    pns.pacing.cfg.layer_visit_cap = 1;
    pns.pacing.layer_visits.insert(0, 1);
    // V=1: not an interleave slot; no expansion possible (the only live
    // node's layer is capped); reserve remains → the fallback drain.
    let v2 = pns.next().unwrap().unwrap();
    assert_eq!(v2.kind, VisitKind::Rung, "fallback drain");
    assert_eq!(v2.key, "");
    assert_eq!(v2.pass, 3);
    assert_eq!(v2.budget, 4 * BUDGET_PNS_BASE_EVALS);
    pns.on_censored(&v2.key, 1_000).unwrap();
    // Third rung (pass 4, 32M) via the fallback again.
    let v3 = pns.next().unwrap().unwrap();
    assert_eq!(v3.kind, VisitKind::Rung);
    assert_eq!(v3.pass, 4);
    pns.on_censored(&v3.key, 1_000).unwrap();
    // The root is rung-capped (3 rungs) → exhausted.
    assert!(pns.next().unwrap().is_none(), "exhausted");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn layer_visit_caps_ration_expansion() {
    let dir = tmp_dir("layercap");
    // Root + 20 open-row replies, no records: all number 1. Rungs off
    // (ladder_deleted); layer_visit_cap = 5: the ply-1 layer (2 rows + 18
    // fresh after the root's censor) stops at 5 visits, deeper layers open.
    let rows = startpos_rows();
    let mut pns = build_with(&rows, &[], config::ladder_deleted(), 300_000_000, &dir);
    pns.pacing.cfg.layer_visit_cap = 5;
    let mut plies = Vec::new();
    for _ in 0..8 {
        let v = pns.next().unwrap().unwrap();
        assert_eq!(v.kind, VisitKind::Expand, "reserve 0 → no rungs ever");
        plies.push(v.ply);
        pns.on_censored(&v.key, 1_000).unwrap();
    }
    assert_eq!(&plies[..5], &[1, 1, 1, 1, 1], "5 ply-1 visits, then capped");
    assert!(plies[5] >= 2 && plies[6] >= 2, "deeper layers open");
    assert_eq!(pns.pacing.layer_visits[&1], 5, "the cap bound before drain");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn eligibility_always_ignores_virgin_children() {
    let dir = tmp_dir("always");
    // Root censored, one reply virgin: "always" makes the root eligible
    // anyway → the first visit is the rung.
    let mut recs = root_with_all_children_records(1);
    recs[1].2 = 0;
    let mut pns = build_with(
        &startpos_rows(),
        &recs,
        PnsConfig {
            eligibility: Eligibility::Always,
            ..config::mechanism_defaults()
        },
        300_000_000,
        &dir,
    );
    let v1 = pns.next().unwrap().unwrap();
    assert_eq!(v1.kind, VisitKind::Rung);
    assert_eq!(v1.key, "");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn linear_and_constant_rung_growth() {
    let dir = tmp_dir("growth");
    let recs = root_with_all_children_records(1);
    for (growth, mults) in [
        (RungGrowth::Linear, [2u64, 3, 4]),
        (RungGrowth::Constant, [1, 1, 1]),
    ] {
        let mut pns = build_with(
            &startpos_rows(),
            &recs,
            PnsConfig {
                rung_growth: growth,
                ..config::mechanism_defaults()
            },
            300_000_000,
            &dir,
        );
        // The first visit is the V=0 rung slot; after each rung the queue's
        // replies are decided and ply 0's cap spent, so every subsequent
        // visit is a fallback-drain rung (the ladder climbs 2 → 3 → 4).
        pns.pacing.cfg.layer_visit_cap = 1;
        pns.pacing.layer_visits.insert(0, 1);
        for (i, mult) in mults.into_iter().enumerate() {
            let v = pns.next().unwrap().unwrap();
            assert_eq!(
                v.kind,
                VisitKind::Rung,
                "growth {:?} rung {}",
                growth,
                i + 2
            );
            assert_eq!(v.key, "");
            assert_eq!(v.budget, mult * BUDGET_PNS_BASE_EVALS);
            pns.on_censored(&v.key, 1_000).unwrap();
            pns.charge_reserve(1_000);
            if i + 1 < mults.len() {
                for uci in legal_ucis() {
                    pns.on_decided(&uci);
                }
            }
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}
