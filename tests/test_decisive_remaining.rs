mod common;

use common::{
    EvalBudget, assert_solves_to_timeout, assert_solves_within_evals, assert_unproven_in_60s,
    assert_unproven_within_evals, load_decisive_remaining_suite, parse_eval_budget_note,
};

/// Deterministic solvable regression targets (slow tier): entries noted
/// `solvable_evals:<B>` must produce their expected outcome within B child
/// evaluations. Budgets carry 3× headroom over the measured first-outcome
/// effort, so a real solver slowdown fails the test deterministically instead
/// of flaking on a wall-clock budget.
#[test]
#[ignore = "slow: deterministic eval budgets total ~10 min; run with -- --include-ignored"]
fn decisive_remaining_solvable_within_evals() {
    for case in load_decisive_remaining_suite() {
        let Some(EvalBudget::Solvable(budget)) =
            parse_eval_budget_note(case.note.as_deref().unwrap_or(""))
        else {
            continue;
        };
        let expected = case
            .expected
            .expect("solvable_evals fixture entries should have an expected outcome");
        assert_solves_within_evals(&case.fen, expected, budget);
    }
}

/// Deterministic unproven tripwire (fast tier): entries noted
/// `unproven_evals:<B>` must stay unproven within B child evaluations. The
/// budget sits far below any known solve effort, so a move-ordering
/// improvement that solves one of these fails the test deterministically and
/// the entry should be re-categorized.
#[test]
fn decisive_remaining_unproven_within_evals() {
    for case in load_decisive_remaining_suite() {
        let Some(EvalBudget::Unproven(budget)) =
            parse_eval_budget_note(case.note.as_deref().unwrap_or(""))
        else {
            continue;
        };
        assert_unproven_within_evals(&case.fen, budget);
    }
}

/// Legacy wall-clock runner for entries noted `solvable_60s`. No entry
/// currently uses that note; it is kept so re-categorized entries (e.g. after
/// a move-ordering improvement) have a runner. See the fixture header.
#[test]
#[ignore = "slow: up to 60 s per solvable position; run with -- --include-ignored"]
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

/// Legacy wall-clock stress runner for entries noted `unproven_60s`, guarding
/// against hangs and time-related bugs that node budgets cannot cover. No
/// entry currently uses that note; it is kept so re-categorized entries have a
/// runner. See the fixture header.
#[test]
#[ignore = "slow: burns a full 60 s per unproven position; run with -- --include-ignored"]
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
