//! Regression/benchmark suite for the move-order positions (m20 to m29).
//!
//! This test is intentionally slow: it solves every position in the suite with a
//! 5-second timeout and asserts that the solver never returns a wrong decisive
//! outcome. `Draw` results are allowed because the hardest positions currently
//! time out; the test still guards against misclassification and hangs.

mod common;

use atomic_solver::position::Outcome;
use common::{assert_solves_or_times_out, assert_solves_to_timeout, load_move_order_suite};

#[test]
#[cfg_attr(debug_assertions, ignore = "slow move-order benchmark suite")]
fn move_order_suite_no_misclassification() {
    for case in load_move_order_suite() {
        let expected = case
            .expected
            .expect("fixture should have an expected outcome");
        assert_solves_or_times_out(&case.fen, expected, 5);
    }
}

/// `m22_white` is the target of the plan-aware ordering work. It is decisive
/// within a 10-second refined search.
#[test]
#[cfg_attr(debug_assertions, ignore = "slow move-order benchmark suite")]
fn m22_white_solves_in_10s() {
    let m22 = load_move_order_suite()
        .into_iter()
        .find(|c| c.name == "m22_white")
        .expect("m22_white fixture missing");
    assert_solves_to_timeout(&m22.fen, Outcome::Win, None, 10);
}

/// Sanity check that every fixture FEN parses and has at least one legal move
/// unless it is terminal.
#[test]
fn move_order_fixture_fens_are_valid() {
    use atomic_solver::position::Position;

    for case in load_move_order_suite() {
        let pos = Position::from_fen(&case.fen)
            .unwrap_or_else(|e| panic!("fixture FEN for {} is invalid: {e}", case.name));
        let legal = pos.legal_moves_vec();
        if legal.is_empty() {
            assert!(
                pos.outcome().is_some(),
                "{} has no legal moves but is not terminal",
                case.name
            );
        }
    }
}
