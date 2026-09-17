//! Integration tests for the detector-gated bounded pre-phase (plan13).
//!
//! The ladder-root tests close the full `KQvK` component (~420k positions,
//! ~5.9M child evals) and are therefore `#[ignore]`-marked slow tests per
//! the tier conventions; the fast-tier unit tests live in
//! `src/search/preflight/tests.rs`.

mod common;

use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::{PvStatus, Search};

/// The plan12 `KQvK` ladder root: win in 15, unproven by the DF-PN solver at
/// 756M+ nodes (plan12 T0). The pre-phase must decide it as a certified Win
/// with the exact DTM as rank and PV length.
const LADDER_ROOT_FEN: &str = "8/2K5/k7/8/8/8/8/4Q3 w - - 0 1";
/// Ladder step 3: win in 9.
const LADDER_STEP3_FEN: &str = "8/5K2/8/3k4/8/8/8/4Q3 w - - 6 4";

#[test]
#[ignore = "slow: full KQvK closure (~5.9M child evals); run with -- --include-ignored"]
fn preflight_certifies_ladder_root_win() {
    let mut pos = Position::from_fen(LADDER_ROOT_FEN).unwrap();
    let mut search = Search::new(128);
    search.set_timeout(60);
    let (outcome, pv, _nodes) = search.solve(&mut pos);

    assert_eq!(outcome, Outcome::Win, "the ladder root is a certified win");
    assert_eq!(search.pv_status(), PvStatus::PreflightProof);
    assert_eq!(pv.len(), 15, "principal line length == exact DTM");

    let report = search.preflight_report().expect("hook ran");
    assert!(report.decided);
    assert_eq!(report.outcome, Some(Outcome::Win));
    assert_eq!(report.rank, 15);
    assert_eq!(report.region, 420_532, "the measured KQvK ladder region");
    assert!(
        report.evals <= 20_000_000,
        "G-A bar: {} child evals",
        report.evals
    );
    assert_eq!(search.child_evaluations(), report.evals);

    // The PV replays legally to the claimed terminal at the claimed rank.
    assert!(search.validate_pv(&pv, &pos, Outcome::Win, Some(15)));

    // No TT interaction of any kind.
    let (_buckets, live, solved, _unsolved, _gen) = search.tt_stats();
    assert_eq!(live, 0);
    assert_eq!(solved, 0);
}

#[test]
#[ignore = "slow: full KQvK closure; run with -- --include-ignored"]
fn preflight_certifies_ladder_step3_win() {
    let mut pos = Position::from_fen(LADDER_STEP3_FEN).unwrap();
    let mut search = Search::new(128);
    search.set_timeout(60);
    let (outcome, pv, _nodes) = search.solve(&mut pos);

    assert_eq!(outcome, Outcome::Win);
    assert_eq!(search.pv_status(), PvStatus::PreflightProof);
    assert_eq!(pv.len(), 9);
    let report = search.preflight_report().unwrap();
    assert!(report.decided);
    assert_eq!(report.rank, 9);
    assert!(search.validate_pv(&pv, &pos, Outcome::Win, Some(9)));
}

#[test]
#[ignore = "slow: full KQvK closure; run with -- --include-ignored"]
fn rule50_rank_guard_defers_ladder_root_at_high_clocks() {
    // D3 end-to-end at the boundary: rank 15, so `clock + 15 <= 99` admits
    // clock 84 exactly (the certificate's deepest clock is 99, still below
    // the rule50 boundary) and defers clock 85 and 99. The clock-0 claim is
    // covered by `preflight_certifies_ladder_root_win`.
    //
    // Clock 84: boundary claim.
    {
        let mut pos = Position::from_fen("8/2K5/k7/8/8/8/8/4Q3 w - - 84 1").unwrap();
        let mut search = Search::new(128);
        search.set_timeout(60);
        let (outcome, pv, _nodes) = search.solve(&mut pos);
        let report = search.preflight_report().unwrap();
        assert!(report.decided, "clock 84 + rank 15 == 99 must claim");
        assert_eq!(outcome, Outcome::Win);
        assert_eq!(report.rank, 15);
        assert!(search.validate_pv(&pv, &pos, Outcome::Win, Some(15)));
    }
    // Clocks above the boundary defer, never claim.
    for clock in [85u16, 99] {
        let fen = format!("8/2K5/k7/8/8/8/8/4Q3 w - - {clock} 1");
        let mut pos = Position::from_fen(&fen).unwrap();
        let mut search = Search::new(128);
        search.set_timeout(10);
        let (outcome, _pv, _nodes) = search.solve(&mut pos);

        let report = search.preflight_report().unwrap();
        assert!(
            !report.decided,
            "clock {clock} + rank 15 exceeds the rule50 guard"
        );
        assert_eq!(report.reason, "rank-guard");
        // The ordinary search then runs and reports whatever it finds on its
        // own terms (at these clocks the rule50 deadline actually prunes the
        // defender's shuffling space, so the solver may even prove the Win
        // itself — that is a search outcome, not a pre-phase claim).
        let _ = outcome;
    }
}

#[test]
#[ignore = "slow: full KQvK closure; run with -- --include-ignored"]
fn search_depth_bound_gates_the_certificate() {
    // A3: under `search_depth` the certificate bound is `max_depth`.
    let mut pos = Position::from_fen(LADDER_STEP3_FEN).unwrap();
    let mut search = Search::new(128);
    search.set_timeout(60);
    let (_outcome, _pv, _nodes) = search.search_depth(&mut pos, 8);

    let report = search.preflight_report().unwrap();
    assert!(!report.decided, "rank 9 > bound 8 must defer");
    assert_eq!(report.reason, "bound");

    // With the bound at the exact rank the claim returns.
    let mut pos = Position::from_fen(LADDER_STEP3_FEN).unwrap();
    let mut search = Search::new(128);
    search.set_timeout(60);
    let (outcome, pv, _nodes) = search.search_depth(&mut pos, 9);
    assert_eq!(outcome, Outcome::Win);
    assert_eq!(pv.len(), 9);
    assert_eq!(search.pv_status(), PvStatus::PreflightProof);
    assert!(search.preflight_report().unwrap().decided);
}

#[test]
#[ignore = "slow: full KRvK closure (~5.2M child evals); run with -- --include-ignored"]
fn prephase_keeps_cyclic_rook_position_draw() {
    // The report7 cyclic-rook safe-area position is 3 men, so the detector
    // DOES fire (the plan's R3 "4 men" note was wrong about this position).
    // The pre-phase proves it a Draw by full closure — a claim, but a sound
    // one: the test_repetition gates (never a Win) keep holding.
    let mut pos = Position::from_fen("8/8/8/8/2k5/8/8/4KR2 w - - 0 1").unwrap();
    let mut search = Search::new(128);
    search.set_timeout(60);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    assert_eq!(outcome, Outcome::Draw, "cyclic rook stays Draw (report7)");
    let report = search.preflight_report().unwrap();
    assert!(report.decided);
    assert_eq!(report.outcome, Some(Outcome::Draw));
    assert_eq!(search.pv_status(), PvStatus::None);
}
