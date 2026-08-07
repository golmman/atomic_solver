mod common;

use atomic_solver::position::Outcome;
use common::{assert_pv_valid, assert_solves_to, assert_solves_with_first_move, solve_with_pv};

#[test]
fn black_root_report4_fen() {
    // After f7e6, White has a forced win, so Black is lost.
    assert_solves_with_first_move(
        "6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27",
        Outcome::Loss,
        "f7e6",
    );
}

#[test]
fn white_child_f7e6_short_win() {
    let fen = "6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28";
    let (outcome, pv, _nodes) = solve_with_pv(fen);
    assert_eq!(outcome, Outcome::Win, "expected white to win after f7e6");
    assert!(!pv.is_empty(), "expected a non-empty PV");
    assert!(
        pv[0] == "g8g7" || pv[0] == "g8f8",
        "expected first move g8g7 or g8f8, got {}",
        pv[0]
    );
    assert!(
        pv.len() <= 3,
        "expected a short win, got {} plies: {pv:?}",
        pv.len()
    );
    assert_pv_valid(fen, Outcome::Win, &pv);
}

#[test]
fn two_rook_mate_refinement_stays_short() {
    assert_solves_to("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1", Outcome::Win, Some(3));
}
