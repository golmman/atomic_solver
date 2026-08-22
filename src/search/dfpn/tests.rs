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
fn tt_work_for_returns_stored_work() {
    let mut search = Search::new(64);
    let key = 0x1234_5678_9abc_def0u64;
    search
        .tt
        .store(key, Move::NONE, u8::MAX, 42, None, 1, 1, 1, 1);
    assert_eq!(search.tt_work_for(key), Some(42));
    assert_eq!(search.tt_work_for(key ^ 0xffff_ffff), None);
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
