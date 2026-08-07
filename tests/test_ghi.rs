use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

/// Two rooks can be developed in either order, leading to the same position.
/// Solving the transposition from both sides (white to move in the start, black
/// to move in the transposed board) should give consistent decisive results and
/// the solver should not blow up when the same TT is reused across the two roots.
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
    assert!(
        Search::validate_pv(&pv1, &pos1, Outcome::Win, None),
        "winning PV must validate for the pawn start"
    );

    // The same transposed position, but with Black to move, is a loss for Black.
    // The TT populated by the first solve should contain a decisive base result
    // for this board that the second solve can reuse.
    let mut pos2 = Position::from_fen("QQ2k3/8/8/8/8/8/8/4K3 b - - 0 1").unwrap();
    let (outcome2, pv2, _nodes2) = search.solve(&mut pos2);
    assert_eq!(
        outcome2,
        Outcome::Loss,
        "black to move in the transposed position should be losing"
    );
    assert!(!pv2.is_empty(), "expected a PV for the loss");
    assert!(
        Search::validate_pv(&pv2, &pos2, Outcome::Loss, None),
        "losing PV must validate for the transposed position"
    );
}
