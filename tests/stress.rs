//! Stress tests for positions that are too deep to prove within the default
//! budget. They run only in release builds (`cargo test --release -- --ignored`)
//! and serve as a guard against hangs or false decisive results.

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
#[cfg_attr(debug_assertions, ignore = "60 second stress test; run with --ignored")]
fn m19_white_unproven_in_60s() {
    assert_unproven_in_60s("4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19");
}

/// The hardest move-order benchmark positions (m20–m21) should still time out in
/// 60 seconds. `m22` is occasionally solved in release within 60 seconds, so it
/// is not asserted as unproven here. When a move-order improvement makes m20 or
/// m21 decisive, this test will fail; the position should then be moved to the
/// regression suite.
#[test]
#[cfg_attr(debug_assertions, ignore = "60 second stress test; run with --ignored")]
fn move_order_hard_positions_unproven_in_60s() {
    for case in load_move_order_suite() {
        if case.name.starts_with("m20_") || case.name.starts_with("m21_") {
            assert_unproven_in_60s(&case.fen);
        }
    }
}
