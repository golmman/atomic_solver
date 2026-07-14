use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

fn solve(fen: &str) -> (Outcome, Vec<String>) {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.refine_shortest(true);
    search.set_timeout(10);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    (outcome, pv.iter().map(|&m| move_to_uci(m)).collect())
}

#[test]
fn shortest_pv_for_reported_fen() {
    let (outcome, pv) = solve("6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28");
    assert!(matches!(outcome, Outcome::Win));
    assert!(!pv.is_empty());
    assert_eq!(&pv[0], "g8g7");
    assert!(
        pv.len() <= 3,
        "expected a short win, got {}: {:?}",
        pv.len(),
        pv
    );
}
