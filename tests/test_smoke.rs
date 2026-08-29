//! Always-on smoke suite for the fast tier (`make test`).
//!
//! Solves every entry in `tests/fixtures/smoke_positions.txt` with a small
//! per-position timeout (2 s) and asserts the property used elsewhere for
//! timing-sensitive tests: the solver never returns a *wrong* result.
//! `Draw` is accepted only when the search was cut short (`time_exceeded()`)
//! or when `Draw` is the expected outcome (terminal stalemate draws return
//! instantly). See `common::assert_smoke` for the exact contract.
//!
//! Entries whose note carries a child-evals budget are routed to the
//! deterministic helpers (`assert_unproven_within_evals` /
//! `assert_solves_within_evals`) instead; the `m22` deep tripwire uses this.
//!
//! This guards against misclassification and hangs in the default gate
//! without burning a fixed wall-clock budget per position.

mod common;

use common::{
    EvalBudget, assert_smoke, assert_solves_within_evals, assert_unproven_within_evals,
    load_smoke_suite, parse_eval_budget_note,
};

#[test]
fn smoke_suite_no_misclassification() {
    for case in load_smoke_suite() {
        match parse_eval_budget_note(case.note.as_deref().unwrap_or("")) {
            Some(EvalBudget::Unproven(budget)) => {
                assert_unproven_within_evals(&case.fen, budget);
            }
            Some(EvalBudget::Solvable(budget)) => {
                let expected = case
                    .expected
                    .expect("solvable_evals smoke entries should have an expected outcome");
                assert_solves_within_evals(&case.fen, expected, budget);
            }
            None => {
                let expected = case
                    .expected
                    .expect("smoke fixture entries should have an expected outcome");
                assert_smoke(&case.fen, expected, 2);
            }
        }
    }
}

/// Sanity check that every smoke fixture FEN parses and has at least one legal
/// move unless it is terminal.
#[test]
fn smoke_fixture_fens_are_valid() {
    use atomic_solver::position::Position;

    for case in load_smoke_suite() {
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
