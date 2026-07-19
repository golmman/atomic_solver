use atomic_movegen::types::{Move, Square};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

#[test]
fn black_root_report4_fen() {
    let mut pos =
        Position::from_fen("6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27").unwrap();
    let mut search = Search::new(64);
    search.refine_shortest(true);
    search.set_timeout(5);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    // After f7e6, White has a forced win, so Black is lost.
    assert_eq!(
        outcome,
        Outcome::Loss,
        "expected black to lose, got {outcome:?}"
    );
    let first = pv.first().copied().unwrap();
    assert_eq!(first, Move::make_move(Square::F7, Square::E6));
}

#[test]
fn white_child_f7e6_short_win() {
    let mut pos = Position::from_fen("6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28").unwrap();
    let mut search = Search::new(64);
    search.refine_shortest(true);
    search.set_timeout(5);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
    let first = pv.first().copied().unwrap();
    let g8g7 = Move::make_move(Square::G8, Square::G7);
    let g8f8 = Move::make_move(Square::G8, Square::F8);
    assert!(
        first == g8g7 || first == g8f8,
        "expected first move g8g7 or g8f8, got {first:?}"
    );
    assert_eq!(pv.len(), 3, "expected a 3-ply win");
}

#[test]
fn two_rook_mate_refinement_stays_short() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.refine_shortest(true);
    search.set_timeout(5);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
    assert!(!pv.is_empty());
    assert!(
        pv.len() <= 3,
        "expected a short win, got {} plies: {:?}",
        pv.len(),
        pv
    );
}
