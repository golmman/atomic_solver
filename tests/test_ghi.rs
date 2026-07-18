use atomic_movegen::types::{Move, Square};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

/// Two rooks can be developed in either order, leading to the same position.
/// Solving the transposition from both sides (white to move in the start, black to
/// move in the transposed board) should give consistent decisive results and the
/// solver should not blow up when the same TT is reused across the two roots.
#[test]
fn promotion_transposition_outcome_is_consistent() {
    let mut search = Search::new(64);
    search.set_timeout(5);

    // White to move; pawns on a7 and b7 can promote in either order.
    let mut pos1 = Position::from_fen("4k3/PP6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let (outcome1, pv1, _nodes1) = search.solve(&mut pos1);
    assert_eq!(
        outcome1,
        Outcome::Win,
        "white should win from the pawn start"
    );
    assert!(!pv1.is_empty(), "expected a PV for the win");

    // The same transposed position, but with Black to move, is a loss for Black.
    // The TT populated by the first solve should contain a decisive result for
    // this board that the second solve can reuse.
    let mut pos2 = Position::from_fen("QQ2k3/8/8/8/8/8/8/4K3 b - - 0 1").unwrap();
    let (outcome2, pv2, _nodes2) = search.solve(&mut pos2);
    assert_eq!(
        outcome2,
        Outcome::Loss,
        "black to move in the transposed position should be losing"
    );
    assert!(!pv2.is_empty(), "expected a PV for the loss");
}

/// A rook alone cannot force a win against a lone king that has a 2x2 safe area.
/// The search tree contains reversible checking cycles, so the solver must not
/// incorrectly claim a win from this GHI-sensitive position.
#[test]
#[ignore = "slow cyclic GHI regression; run with --ignored"]
fn cyclic_rook_position_does_not_claim_win() {
    let mut pos = Position::from_fen("8/8/8/8/2k5/8/8/4KR2 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_ne!(
        outcome,
        Outcome::Win,
        "rook alone should not win in a 2x2 safe area"
    );
}

/// Perform a reversible rook/king shuffle that returns to the same board with a
/// higher rule50 counter. The repeated board is drawn (the rook cannot win), and
/// the solver must not report a win.
#[test]
#[ignore = "slow cyclic GHI regression; run with --ignored"]
fn reversible_cycle_does_not_claim_win() {
    let mut pos = Position::from_fen("8/8/8/8/2k5/8/8/4KR2 w - - 0 1").unwrap();

    // Rf1-g1, Kc4-b4, Rg1-f1, Kb4-c4 returns to the same board.
    let moves = [
        Move::make_move(Square::F1, Square::G1),
        Move::make_move(Square::C4, Square::B4),
        Move::make_move(Square::G1, Square::F1),
        Move::make_move(Square::B4, Square::C4),
    ];
    for mv in moves {
        pos.do_move(mv);
    }

    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_ne!(
        outcome,
        Outcome::Win,
        "repeated board should not be declared a win"
    );
}

/// A cross-path twin whose winning move depends on a repetition that is only
/// legal in the twin's original path is difficult to construct for atomic chess.
/// When such a position is found, this test should be enabled with a concrete FEN
/// and the solver should not incorrectly reuse a win proven along a different
/// path.
#[test]
#[ignore = "TODO: construct a concrete atomic-chess cross-path repetition-dependent win"]
fn cross_path_repetition_dependent_win_is_not_reused() {}
