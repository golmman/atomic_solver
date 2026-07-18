use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

fn solve(fen: &str) -> (Outcome, Vec<String>, u64) {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, pv, nodes) = search.solve(&mut pos);
    (outcome, pv.iter().map(|&m| move_to_uci(m)).collect(), nodes)
}

/// A 50-move checkmate must be reported as a loss for the side to move,
/// not as a draw.  The no-legal-moves checkmate/stalemate check has priority
/// over the 50-move draw rule.
#[test]
fn fifty_move_checkmate_is_loss() {
    let (outcome, _pv, _nodes) = solve("7K/8/8/8/8/8/1Q6/k7 b - - 100 1");
    assert_eq!(outcome, Outcome::Loss);
}

/// A 50-move stalemate is a draw: the side to move has no legal moves and is
/// not in check, which is terminal before the 50-move draw rule.
#[test]
fn fifty_move_stalemate_is_draw() {
    let (outcome, _pv, _nodes) = solve("7k/8/8/8/8/8/2q5/K7 w - - 100 1");
    assert_eq!(outcome, Outcome::Draw);
}

/// In standard atomic chess touching commoners (kings) are allowed and do not
/// count as an attack, so this two-king position is a draw by insufficient
/// material, not a checkmate.
#[test]
fn touching_commoners_with_two_pieces_is_draw() {
    let (outcome, _pv, _nodes) = solve("8/8/8/8/8/8/1K6/k7 b - - 0 1");
    assert_eq!(outcome, Outcome::Draw);
}
