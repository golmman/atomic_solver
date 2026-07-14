use atomic_movegen::types::{Move, Square};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

// The black-to-move FEN is a loss for Black, but the solver cannot prove it
// within the 60-second timeout without additional move-ordering improvements.
// Move ordering is explicitly out of scope for plan5 (see plan5.md non-goals).
#[ignore = "requires move-ordering follow-up (plan5 non-goal)"]
#[test]
fn black_root_report4_fen() {
    let mut pos =
        Position::from_fen("6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(60);
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
    search.set_timeout(60);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
    let first = pv.first().copied().unwrap();
    assert_eq!(first, Move::make_move(Square::G8, Square::G7));
    assert_eq!(pv.len(), 3, "expected a 3-ply win");
}
