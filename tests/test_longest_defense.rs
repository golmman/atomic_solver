use atomic_movegen::types::{Move, Square};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

#[test]
fn longest_defense_pv() {
    let mut search = Search::new(64);
    let mut pos =
        Position::from_fen("rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3")
            .unwrap();
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Loss);
    assert_eq!(
        pv,
        vec![
            Move::make_move(Square::D7, Square::D6),
            Move::make_move(Square::D5, Square::F7),
            Move::make_move(Square::E8, Square::D7),
            Move::make_move(Square::F7, Square::E7),
        ]
    );
}
