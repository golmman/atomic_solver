use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::{Search, outcome_from_pn_dn};
use atomic_solver::zobrist::INF;

fn solve(fen: &str) -> Outcome {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    outcome
}

#[test]
fn lone_commoner_checkmate_is_loss_for_side_to_move() {
    // Black commoner on a1 is attacked by the white queen on b2 with no escape.
    assert_eq!(solve("7K/8/8/8/8/8/1Q6/k7 b - - 0 1"), Outcome::Loss);
}

#[test]
fn stalemate_with_no_commoner_under_attack_is_draw() {
    // White commoner on a1 has no legal moves and is not under attack.
    // Black also has a commoner so the game is not already decided.
    assert_eq!(solve("7k/8/8/8/8/8/2q5/K7 w - - 0 1"), Outcome::Draw);
}

#[test]
fn outcome_prefers_own_extinction_over_rule50_and_two_piece_draw() {
    // White has no commoners (only a pawn) and rule50 >= 100; should still be Loss.
    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4P3 w - - 100 1").unwrap();
    assert_eq!(pos.outcome(), Some(Outcome::Loss));
}

#[test]
fn outcome_prefers_opponent_extinction_over_rule50() {
    // Black to move, white has no commoners, rule50 >= 100; should be Win for Black.
    let pos = Position::from_fen("4k3/8/8/8/8/8/8/8 b - - 100 1").unwrap();
    assert_eq!(pos.outcome(), Some(Outcome::Win));
}

#[test]
fn outcome_from_pn_dn_only_recognizes_win() {
    assert_eq!(outcome_from_pn_dn(0, INF), Some(Outcome::Win));
    assert_eq!(outcome_from_pn_dn(INF, 0), None);
    assert_eq!(outcome_from_pn_dn(1, 1), None);
}
