use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

/// A transposition-heavy position: white has two rooks that can be developed in
/// either order (f1-f3/g1-g3 transposes with g1-g3/f1-f3).  The solver should
/// still find the forced win quickly without a node blow-up.
#[test]
fn two_rooks_mate_with_transpositions() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, pv, nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
    assert!(!pv.is_empty());
    assert!(
        nodes < 10_000,
        "node blow-up in transposition position: {}",
        nodes
    );
}
