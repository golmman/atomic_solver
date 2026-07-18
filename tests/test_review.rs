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

/// Single-commoner checkmates: the losing side has exactly one commoner (the
/// king) and the winning side has a line piece and a king.  These cases were
/// fragile in earlier versions because stalemate and checkmate were conflated.
#[test]
fn queen_corner_mates_are_loss_for_black() {
    let fens = [
        "7K/8/8/8/8/8/1Q6/k7 b - - 0 1",
        "K7/8/8/8/8/8/6Q1/7k b - - 0 1",
        "k7/1Q6/8/8/8/8/8/7K b - - 0 1",
        "7k/6Q1/8/8/8/8/8/K7 b - - 0 1",
    ];
    for fen in fens {
        let (outcome, _pv, _nodes) = solve(fen);
        assert_eq!(outcome, Outcome::Loss, "expected mate for {fen}");
    }
}

/// Stalemate: the side to move has a commoner but no legal moves and is not
/// under attack, so the result is a draw, not a loss.
#[test]
fn stalemate_with_no_commoner_under_attack_is_draw() {
    let fens = [
        "7k/8/8/8/8/8/2q5/K7 w - - 0 1",
        "7K/8/8/8/8/8/2Q5/k7 b - - 0 1",
    ];
    for fen in fens {
        let (outcome, _pv, _nodes) = solve(fen);
        assert_eq!(outcome, Outcome::Draw, "expected stalemate draw for {fen}");
    }
}

/// A transposition-heavy win: the two white rooks can be developed in either
/// order, so the same board state is reached by different move paths.  The
/// solver must still find the forced win quickly.
#[test]
fn two_rook_transposition_still_wins() {
    let (outcome, pv, _nodes) = solve("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1");
    assert_eq!(outcome, Outcome::Win);
    assert!(!pv.is_empty());
    assert!(
        pv.len() <= 3,
        "expected a short win, got {} plies: {:?}",
        pv.len(),
        pv
    );
}

/// A promotion transposition: both white pawns can promote to queen, and the
/// order of the two promotions leads to the same board state.  This stresses
/// path-code handling for promotion moves.
#[test]
fn promotion_transposition_still_wins() {
    let (outcome, pv, nodes) = solve("4k3/PP6/8/8/8/8/8/4K3 w - - 0 1");
    assert_eq!(outcome, Outcome::Win);
    assert!(!pv.is_empty());
    assert!(
        nodes < 20_000,
        "node blow-up in promotion transposition position: {}",
        nodes
    );
}

/// A depth-bound cutoff must not be stored as a proven draw. `search_depth(0)`
/// should return `Draw` for a non-terminal winning position, but a follow-up
/// `search_depth(3)` on the same `Search` (and therefore the same transposition
/// table) must still discover the win.
#[test]
fn depth_zero_cutoff_is_not_reused_as_proven_draw() {
    let fen = "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1";
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);

    let (outcome, pv, _nodes) = search.search_depth(&mut pos, 0);
    assert_eq!(outcome, Outcome::Draw, "depth 0 should be a cutoff");
    assert!(pv.is_empty(), "depth 0 should have no PV");

    let (outcome, pv, _nodes) = search.search_depth(&mut pos, 3);
    assert_eq!(outcome, Outcome::Win, "depth 3 should find the win");
    assert!(!pv.is_empty(), "winning PV should not be empty");
}
