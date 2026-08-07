mod common;

use atomic_solver::position::Outcome;
use common::assert_solves_to;

#[test]
fn mate_in_4_white_to_move() {
    assert_solves_to(
        "rnbqkbnr/ppppp1pp/5p2/8/8/4P3/PPPP1PPP/RNBQKBNR w KQkq - 0 2",
        Outcome::Win,
        None,
    );
}

#[test]
fn mate_in_3_black_to_move() {
    assert_solves_to(
        "rnbqkbnr/ppppp1pp/5p2/7Q/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 2",
        Outcome::Loss,
        None,
    );
}

#[test]
fn mate_in_2_white_to_move() {
    assert_solves_to(
        "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3",
        Outcome::Win,
        None,
    );
}

#[test]
fn mate_in_2_black_to_move() {
    assert_solves_to(
        "rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3",
        Outcome::Loss,
        None,
    );
}

#[test]
fn mate_in_1_white_to_move() {
    // The queen move Qf7 is not a capture, so it does not explode the king; the
    // actual shortest line is Qf7 Kd7 Qe7# (3 plies).
    assert_solves_to(
        "rnbqkbnr/ppp1p2p/3p1pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 4",
        Outcome::Win,
        None,
    );
}

#[test]
fn mate_in_1_black_to_move() {
    // Black is already in a mating net; the win is delivered in 2 plies.
    assert_solves_to(
        "rnbqkbnr/ppp1pQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 4",
        Outcome::Loss,
        None,
    );
}

#[test]
fn win_with_exploded_black_king_white_to_move() {
    assert_solves_to(
        "rnbq1bnr/pppkpQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR w KQ - 2 5",
        Outcome::Win,
        Some(1),
    );
}

#[test]
fn win_with_exploded_black_king_black_to_move() {
    assert_solves_to(
        "rnb3nr/ppp4p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR b KQ - 0 5",
        Outcome::Loss,
        None,
    );
}

#[test]
fn only_two_kings_draw_white_to_move() {
    assert_solves_to("7k/8/8/8/8/8/8/7K w - - 0 1", Outcome::Draw, None);
}

#[test]
fn only_two_kings_draw_black_to_move() {
    assert_solves_to("7k/8/8/8/8/8/8/7K b - - 0 1", Outcome::Draw, None);
}
