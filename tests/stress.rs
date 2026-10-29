//! Stress tests for positions that are too deep to prove within the default
//! budget. They are marked `#[ignore]` (slow tier) and run via
//! `cargo test --release -- --include-ignored`; they serve as a guard against
//! hangs or false decisive results.
//!
//! The m19 FEN below is a literal because this test cannot include
//! `examples/common.rs`; its canonical source is `common::M19_FEN` there
//! (keep the two in sync).

mod common;

use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use common::load_move_order_suite;

fn assert_unproven_in_60s(fen: &str) {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(60);

    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    // The position should still be unproven within a 60-second budget. If the
    // solver ever returns a decisive result here, the test should fail so that
    // the position can be moved to the regression suite and the outcome documented.
    assert!(
        search.time_exceeded(),
        "expected search to time out for unproven stress position {fen}"
    );
    assert_eq!(
        outcome,
        Outcome::Draw,
        "unproven stress position should return Draw on timeout, got {outcome:?} for {fen}"
    );
}

#[test]
#[ignore = "slow: 60 s budget per position; run with -- --include-ignored"]
fn m19_white_unproven_in_60s() {
    assert_unproven_in_60s("4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19");
}

/// The hardest move-order benchmark positions (m20–m21) should still time out in
/// 60 seconds. `m22` is occasionally solved in release within 60 seconds, so it
/// is not asserted as unproven here. When a move-order improvement makes m20 or
/// m21 decisive, this test will fail; the position should then be moved to the
/// regression suite.
///
/// `m21_white` crossed that line: at a 64 MB TT it is now decisive in ~52 s
/// (596,102,838 child evals, `Win` 200 plies, `pv_status: cap-cut`; measured
/// during plan12 Phase 0 against HEAD). It is therefore excluded from the
/// unproven assertion; the remaining positions still time out. This exclusion
/// is a deliberate decision, not a pending TODO: do not promote `m21_white` to
/// a positive regression unless the slow tier's budget is deliberately raised
/// (it would add ~3.5% to the already ~25-minute full tier, while the
/// restart-sensitive class it guards is covered by `dec31`, `m23_white`, and
/// `m22_white_wins`).
#[test]
#[ignore = "slow: 60 s budget per position; run with -- --include-ignored"]
fn move_order_hard_positions_unproven_in_60s() {
    for case in load_move_order_suite() {
        let hardest = case.name.starts_with("m20_") || case.name.starts_with("m21_");
        if hardest && case.name != "m21_white" {
            assert_unproven_in_60s(&case.fen);
        }
    }
}
