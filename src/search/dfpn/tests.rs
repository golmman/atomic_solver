//! Cross-module DF-PN unit tests.

use crate::position::{Outcome, Position};
use crate::search::dfpn::Search;
use atomic_movegen::types::{Move, Square};

#[test]
fn local_repetition_in_prefix_returns_draw() {
    // The same cyclic rook-safe-area position, reached after a reversible
    // rook/king shuffle. Its own repetition key is supplied as a prefix, so
    // the solver should short-circuit to a draw.
    let mut pos = Position::from_fen("8/8/8/8/2k5/8/8/4KR2 w - - 0 1").unwrap();
    pos.do_move(Move::make_move(Square::F1, Square::G1));
    pos.do_move(Move::make_move(Square::C4, Square::B4));
    pos.do_move(Move::make_move(Square::G1, Square::F1));
    pos.do_move(Move::make_move(Square::B4, Square::C4));

    let rep_key = pos.repetition_key();
    let mut search = Search::new(64);
    search.set_timeout(5);

    let (outcome, depth, _nodes) = search.search_depth_with_prefix(&mut pos, u32::MAX, &[rep_key]);
    assert_eq!(outcome, Outcome::Draw);
    assert_eq!(depth, 0);
}

#[test]
fn try_use_tt_rejects_win_when_best_move_repeats() {
    // Store a win for a position whose winning move leads to a board already
    // on the search path. The one-ply repetition guard in try_use_tt should
    // reject the cached result.
    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let key = pos.hash();
    let win_move = Move::make_move(Square::E1, Square::E8);

    let mut child = pos.clone();
    child.do_move(win_move);
    let child_rep_key = child.repetition_key();

    let mut search = Search::new(64);
    search.tt.store(
        key,
        win_move,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        crate::zobrist::INF,
        1,
        u32::MAX,
    );

    // With the child on the path, the cached win is invalid.
    search.path_stack.push(child_rep_key);
    assert!(
        search.try_use_tt(&pos, key, u32::MAX).is_none(),
        "try_use_tt should reject a win whose best move repeats a board on the path"
    );

    // Without the child on the path, the cached win is valid.
    search.path_stack.clear();
    let resolved = search
        .try_use_tt(&pos, key, u32::MAX)
        .expect("cached win should be accepted when the child is not on the path");
    assert_eq!(resolved.outcome, Outcome::Win);
    assert_eq!(resolved.depth, 1);
}

#[test]
fn set_timeout_zero_causes_immediate_exit() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(0);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Draw, "timeout 0 should return Draw");
    assert!(
        search.time_exceeded(),
        "time should be exceeded after timeout 0"
    );
    assert!(
        matches!(
            search.exit_reason(),
            crate::search::dfpn::ExitReason::Timeout
        ),
        "exit reason should be Timeout"
    );
}

#[test]
fn first_outcome_only_skips_refinement() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_first_outcome_only(true);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
    assert!(
        !pv.is_empty(),
        "first-outcome mode should still return a winning PV"
    );
}

#[test]
fn solve_with_progress_calls_closure() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);

    let mut calls = 0;
    let (outcome, _pv, _nodes) = search.solve_with_progress(&mut pos, |_o, _pv| {
        calls += 1;
    });
    assert_eq!(outcome, Outcome::Win);
    assert!(
        calls > 0,
        "progress closure should be invoked at least once"
    );
}

#[test]
fn exit_reason_reports_complete() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let _ = search.solve(&mut pos);
    assert!(
        matches!(
            search.exit_reason(),
            crate::search::dfpn::ExitReason::Complete
        ),
        "a solved position should report Complete"
    );
}

#[test]
fn child_eval_budget_zero_causes_immediate_exit() {
    let mut pos = Position::from_fen("4k3/PP6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_child_eval_budget(0);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Draw, "budget 0 should return Draw");
    assert!(
        pv.is_empty(),
        "expected an empty PV after immediate budget cutoff, got {pv:?}"
    );
    assert!(
        search.child_eval_budget_exceeded(),
        "budget should be exceeded"
    );
    assert!(
        !search.time_exceeded(),
        "budget exhaustion must not be reported as a timeout"
    );
    assert_eq!(
        search.exit_reason(),
        crate::search::dfpn::ExitReason::BudgetExhausted,
        "exit reason should be BudgetExhausted"
    );
    let (_buckets, live_entries, solved_entries, _unsolved, _generation) = search.tt_stats();
    assert_eq!(live_entries, 0, "a budget-0 search must not store anything");
    assert_eq!(
        solved_entries, 0,
        "a budget-0 search must not cache a proven entry"
    );
}

#[test]
fn child_eval_budget_generous_still_solves() {
    // The full refined solve of the promotion-transposition position measures
    // W = 426,882 child evaluations (deterministic); 5,000,000 is ~10x W.
    let mut pos = Position::from_fen("4k3/PP6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_child_eval_budget(5_000_000);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win, "generous budget should still solve");
    assert!(!pv.is_empty(), "solved search should return a PV");
    assert!(
        !search.child_eval_budget_exceeded(),
        "the budget must not be exhausted by a solve inside it"
    );
    assert_eq!(
        search.exit_reason(),
        crate::search::dfpn::ExitReason::Complete
    );
}

#[test]
fn child_eval_budget_fraction_does_not_solve() {
    // The first decisive line needs W1 = 7,449 child evaluations; a budget
    // below that must return Draw, report the budget (not a timeout), and
    // cache only unsolved entries.
    let mut pos = Position::from_fen("4k3/PP6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_child_eval_budget(5_000);

    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(
        outcome,
        Outcome::Draw,
        "a budget below the solve effort must return Draw"
    );
    assert!(search.child_eval_budget_exceeded());
    assert!(
        !search.time_exceeded(),
        "budget exhaustion must not be reported as a timeout"
    );
    assert_eq!(
        search.exit_reason(),
        crate::search::dfpn::ExitReason::BudgetExhausted
    );

    // A budget-cut result must not poison the transposition table: re-solving
    // the same position on the same Search (and therefore the same TT) with
    // an unbounded budget must still find the win.
    search.set_child_eval_budget(u64::MAX);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(
        outcome,
        Outcome::Win,
        "a budget-cut search must not cache a proven result that hides the win"
    );
}

#[test]
fn budget_exhausted_pv_is_empty_or_valid() {
    // Tiny budget on a deeper position: the informational PV must never be a
    // wrong decisive line, so it must be empty or a fully legal line.
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_child_eval_budget(5);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Draw);
    assert!(search.child_eval_budget_exceeded());
    let mut current = pos.clone();
    for mv in &pv {
        assert!(
            current.try_do_move(*mv),
            "PV move must be legal in a budget-exhausted search"
        );
    }
}
