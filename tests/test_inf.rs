mod common;

use atomic_solver::position::Outcome;
use common::assert_solves_to;

#[test]
fn solve_rook_mate_win() {
    assert_solves_to("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1", Outcome::Win, Some(1));
}

#[test]
fn solve_rook_mate_black_to_move_draw() {
    assert_solves_to("4k3/8/8/8/8/8/8/4R1K1 b - - 0 1", Outcome::Draw, None);
}

#[test]
fn solve_king_only_draw_white() {
    assert_solves_to("4k3/8/8/8/8/8/8/4K3 w - - 0 1", Outcome::Draw, None);
}

#[test]
fn solve_king_only_draw_black() {
    assert_solves_to("4k3/8/8/8/8/8/8/4K3 b - - 0 1", Outcome::Draw, None);
}

#[test]
fn solve_opposed_kings_draw() {
    assert_solves_to("8/8/8/8/4k3/8/4K3/8 w - - 0 1", Outcome::Draw, None);
}

#[test]
fn solve_no_white_pieces_loss() {
    assert_solves_to("4k3/8/8/8/8/8/8/8 w - - 0 1", Outcome::Loss, None);
}

#[test]
fn solve_no_white_pieces_black_win() {
    assert_solves_to("4k3/8/8/8/8/8/8/8 b - - 0 1", Outcome::Win, None);
}
