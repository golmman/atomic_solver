//! Stress tests for positions that are too deep to prove within the default
//! budget. They run only in release builds (`cargo test --release -- --ignored`)
//! and serve as a guard against hangs or false decisive results.

use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

fn assert_does_not_hang_or_misclassify(fen: &str) {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(60);

    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    // These positions are currently unproven within a 60-second budget. If the
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
    assert_does_not_hang_or_misclassify("4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "60 second stress test; run with --ignored")]
fn m20_white_unproven_in_60s() {
    assert_does_not_hang_or_misclassify("4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "60 second stress test; run with --ignored")]
fn m20_black_unproven_in_60s() {
    assert_does_not_hang_or_misclassify("4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/1R4RK b - - 5 20");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "60 second stress test; run with --ignored")]
fn m21_white_unproven_in_60s() {
    assert_does_not_hang_or_misclassify("4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "60 second stress test; run with --ignored")]
fn m21_black_unproven_in_60s() {
    assert_does_not_hang_or_misclassify("4r2k/3p4/2pB2p1/p4p1p/6PP/2N1PP2/P1PP4/1R4RK b - - 0 21");
}
