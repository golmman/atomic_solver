use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

fn solve(fen: &str) -> Outcome {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    outcome
}

#[test]
fn mate_in_4_white_to_move() {
    assert!(matches!(
        solve("rnbqkbnr/ppppp1pp/5p2/8/8/4P3/PPPP1PPP/RNBQKBNR w KQkq - 0 2"),
        Outcome::Win
    ));
}

#[test]
fn mate_in_3_black_to_move() {
    assert!(matches!(
        solve("rnbqkbnr/ppppp1pp/5p2/7Q/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 2"),
        Outcome::Loss
    ));
}

#[test]
fn mate_in_2_white_to_move() {
    assert!(matches!(
        solve("rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"),
        Outcome::Win
    ));
}

#[test]
fn mate_in_2_black_to_move() {
    assert!(matches!(
        solve("rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3"),
        Outcome::Loss
    ));
}

#[test]
fn mate_in_1_white_to_move() {
    assert!(matches!(
        solve("rnbqkbnr/ppp1p2p/3p1pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 4"),
        Outcome::Win
    ));
}

#[test]
fn mate_in_1_black_to_move() {
    assert!(matches!(
        solve("rnbqkbnr/ppp1pQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 4"),
        Outcome::Loss
    ));
}

#[test]
fn win_with_exploded_black_king_white_to_move() {
    assert!(matches!(
        solve("rnbq1bnr/pppkpQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR w KQ - 2 5"),
        Outcome::Win
    ));
}

#[test]
fn win_with_exploded_black_king_black_to_move() {
    assert!(matches!(
        solve("rnb3nr/ppp4p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR b KQ - 0 5"),
        Outcome::Loss
    ));
}

#[test]
fn only_two_kings_draw_white_to_move() {
    assert!(matches!(
        solve("7k/8/8/8/8/8/8/7K w - - 0 1"),
        Outcome::Draw
    ));
}

#[test]
fn only_two_kings_draw_black_to_move() {
    assert!(matches!(
        solve("7k/8/8/8/8/8/8/7K b - - 0 1"),
        Outcome::Draw
    ));
}
