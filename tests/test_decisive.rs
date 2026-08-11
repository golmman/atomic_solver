mod common;

use common::{assert_solves_or_times_out, load_decisive_suite};

/// Solve every decisive benchmark position with a 5-second timeout and assert
/// that the solver never returns a wrong decisive outcome.  Draw results are
/// allowed because positions may occasionally time out due to machine variance,
/// but a Draw without a timeout or a wrong decisive result is a failure.
#[test]
#[cfg_attr(debug_assertions, ignore = "slow decisive benchmark suite")]
fn decisive_suite_no_misclassification() {
    for case in load_decisive_suite() {
        let expected = case
            .expected
            .expect("fixture should have an expected outcome");
        assert_solves_or_times_out(&case.fen, expected, 5);
    }
}

/// Sanity check that every fixture FEN parses and has at least one legal move
/// unless it is terminal.
#[test]
fn decisive_fixture_fens_are_valid() {
    use atomic_solver::position::Position;

    for case in load_decisive_suite() {
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
