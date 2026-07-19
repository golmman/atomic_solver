mod common;

use atomic_solver::position::Outcome;
use common::solve;

#[test]
fn solve_rook_mate_win() {
    assert!(matches!(
        solve("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1"),
        Outcome::Win
    ));
}

#[test]
fn solve_rook_mate_black_to_move_draw() {
    assert!(matches!(
        solve("4k3/8/8/8/8/8/8/4R1K1 b - - 0 1"),
        Outcome::Draw
    ));
}

#[test]
fn solve_king_only_draw_white() {
    assert!(matches!(
        solve("4k3/8/8/8/8/8/8/4K3 w - - 0 1"),
        Outcome::Draw
    ));
}

#[test]
fn solve_king_only_draw_black() {
    assert!(matches!(
        solve("4k3/8/8/8/8/8/8/4K3 b - - 0 1"),
        Outcome::Draw
    ));
}

#[test]
fn solve_opposed_kings_draw() {
    assert!(matches!(
        solve("8/8/8/8/4k3/8/4K3/8 w - - 0 1"),
        Outcome::Draw
    ));
}

#[test]
fn solve_no_white_pieces_loss() {
    assert!(matches!(
        solve("4k3/8/8/8/8/8/8/8 w - - 0 1"),
        Outcome::Loss
    ));
}

#[test]
fn solve_no_white_pieces_black_win() {
    assert!(matches!(solve("4k3/8/8/8/8/8/8/8 b - - 0 1"), Outcome::Win));
}
