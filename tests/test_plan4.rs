mod common;

use atomic_solver::position::Outcome;
use common::solve_refined;

#[test]
fn shortest_pv_for_reported_fen() {
    let (outcome, pv, _nodes) = solve_refined("6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28");
    assert!(matches!(outcome, Outcome::Win));
    assert!(!pv.is_empty());
    assert!(
        pv[0] == "g8g7" || pv[0] == "g8f8",
        "expected the PV to start with a 7th/8th-rank rook move, got {}",
        pv[0]
    );
    assert!(
        pv.len() <= 3,
        "expected a short win, got {}: {:?}",
        pv.len(),
        pv
    );
}
