mod common;

use atomic_solver::position::Outcome;
use common::{assert_solves_to, solve_with_pv};

#[test]
fn black_root_report4_fen() {
    // After f7e6, White has a forced win, so Black is lost.
    assert_solves_to(
        "6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27",
        Outcome::Loss,
        None,
    );
}

#[test]
fn white_child_f7e6_short_win() {
    let fen = "6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28";
    let (outcome, _pv, _nodes) = solve_with_pv(fen);
    assert_eq!(outcome, Outcome::Win, "expected white to win after f7e6");
}

#[test]
fn two_rook_mate_refinement_stays_short() {
    assert_solves_to("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1", Outcome::Win, Some(3));
}
