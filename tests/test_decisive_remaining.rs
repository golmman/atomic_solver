mod common;

use common::{assert_solves_to_timeout, assert_unproven_in_60s, load_decisive_remaining_suite};

/// Positions from the provided list that are too hard for the default 5-second
/// budget but still produce a decisive outcome within 60 seconds.  These are
/// treated as regression targets: when future move-ordering improvements make
/// them faster, this test should continue to pass.
#[test]
#[cfg_attr(debug_assertions, ignore = "slow decisive regression suite")]
fn decisive_remaining_solvable_in_60s() {
    for case in load_decisive_remaining_suite() {
        if case.note.as_deref() != Some("solvable_60s") {
            continue;
        }
        let expected = case
            .expected
            .expect("solvable_60s fixture entries should have an expected outcome");
        assert_solves_to_timeout(&case.fen, expected, None, 60);
    }
}

/// Positions from the provided list that remain unproven within 60 seconds.
/// When a move-ordering improvement makes one of these decisive, this test will
/// fail; the position should then be re-categorized as `solvable_60s` and an
/// expected outcome should be recorded.
#[test]
#[cfg_attr(debug_assertions, ignore = "slow decisive stress suite")]
fn decisive_remaining_unproven_in_60s() {
    for case in load_decisive_remaining_suite() {
        if case.note.as_deref() != Some("unproven_60s") {
            continue;
        }
        assert_unproven_in_60s(&case.fen);
    }
}

/// Sanity check that every fixture FEN parses and has at least one legal move
/// unless it is terminal.
#[test]
fn decisive_remaining_fixture_fens_are_valid() {
    use atomic_solver::position::Position;

    for case in load_decisive_remaining_suite() {
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
