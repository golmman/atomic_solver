//! Integration tests for the opt-in win-only front-end (`--win-front-end`).
//!
//! Two contracts are pinned here:
//!
//! - **Fallback identity** — on a root the front-end does not prove within its
//!   cap, the general search is bit-identical to a flag-off run: same outcome,
//!   PV, general-search nodes/evals, and TT snapshot.
//! - **Proven win** — on a root the front-end proves, the product result is a
//!   replay-validated `Win` with `PvStatus::WinFrontEnd`.

mod common;

use atomic_movegen::types::MoveList;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::{ExitReason, PvStatus, Search};
use atomic_solver::tt_snapshot::write_tt_snapshot;

/// Black to move and losing in three plies; the front-end only claims Wins, so
/// this root always falls back.
const FALLBACK_FEN: &str = "rnbqkbnr/ppppp1pp/5p2/7Q/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 2";
/// White to move wins in one move (`Re8` captures the black commoner).
const MATE_IN_ONE: &str = "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1";

fn solve(fen: &str, front_end: bool) -> (Outcome, Vec<atomic_movegen::types::Move>, Search) {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(16);
    search.set_timeout(60);
    search.set_first_outcome_only(true);
    search.set_win_front_end(front_end);
    search.set_win_front_end_budget(200);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    (outcome, pv, search)
}

fn snapshot(search: &Search, fen: &str) -> Vec<u8> {
    let mut out = Vec::new();
    write_tt_snapshot(search.tt(), fen, 16, &mut out).expect("snapshot");
    out
}

#[test]
fn fallback_is_bit_identical_to_flag_off() {
    let (on_outcome, on_pv, on) = solve(FALLBACK_FEN, true);
    let (off_outcome, off_pv, off) = solve(FALLBACK_FEN, false);

    assert_eq!(on_outcome, off_outcome);
    assert_eq!(on_pv, off_pv);
    assert_eq!(
        on.nodes() - on.win_front_end_nodes(),
        off.nodes(),
        "general-search nodes must match the flag-off run"
    );
    assert_eq!(
        on.child_evaluations() - on.win_front_end_evaluations(),
        off.child_evaluations(),
        "general-search evals must match the flag-off run"
    );
    assert!(on.win_front_end_nodes() > 0, "the front-end must have run");
    assert!(on.win_front_end_evaluations() > 0);
    assert_ne!(on.pv_status(), PvStatus::WinFrontEnd);
    assert_eq!(snapshot(&on, FALLBACK_FEN), snapshot(&off, FALLBACK_FEN));
}

#[test]
fn front_end_work_counts_against_the_global_budget() {
    let mut pos = Position::from_fen(Position::STARTPOS_FEN).unwrap();
    let mut search = Search::new(16);
    search.set_timeout(60);
    search.set_first_outcome_only(true);
    search.set_win_front_end(true);
    search.set_win_front_end_budget(100);
    search.set_child_eval_budget(500);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    assert_eq!(outcome, Outcome::Draw);
    assert_eq!(search.exit_reason(), ExitReason::BudgetExhausted);
    assert!(search.win_front_end_evaluations() >= 100);
    assert!(search.child_evaluations() >= 500);
}

#[test]
fn proven_win_returns_replay_validated_pv() {
    let (outcome, pv, search) = solve(MATE_IN_ONE, true);
    assert_eq!(outcome, Outcome::Win);
    assert_eq!(search.pv_status(), PvStatus::WinFrontEnd);
    assert_eq!(pv.len(), 1);

    let mut replay = Position::from_fen(MATE_IN_ONE).unwrap();
    for mv in &pv {
        let mut moves = MoveList::new();
        replay.legal_moves(&mut moves);
        assert!(moves.as_slice().contains(mv), "PV must be legal");
        replay.do_move(*mv);
    }
    assert_eq!(replay.outcome(), Some(Outcome::Loss));
    assert!(search.win_front_end_evaluations() > 0);
    assert_eq!(
        search.child_evaluations(),
        search.win_front_end_evaluations(),
        "the general search must not run after a front-end claim"
    );
}
