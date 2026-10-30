//! Unit tests for the parent-side plainness predicate.

use super::*;
use atomic_movegen::types::Square;

fn pos(fen: &str) -> Position {
    Position::from_fen(fen).unwrap()
}

fn normal(fen: &str, from: Square, to: Square) -> bool {
    is_plain(&pos(fen), Move::make_move(from, to))
}

#[test]
fn plain_quiet_knight_move_is_grouped() {
    assert!(normal(
        "4k3/8/8/8/8/8/8/K5N1 w - - 0 1",
        Square::G1,
        Square::F3
    ));
}

#[test]
fn check_is_not_plain() {
    assert!(!normal(
        "4k3/8/8/8/8/8/8/4K2R w - - 0 1",
        Square::H1,
        Square::H8
    ));
}

#[test]
fn capture_is_not_plain() {
    assert!(!normal(
        "4k3/6p1/8/8/8/8/8/4K1R1 w - - 0 1",
        Square::G1,
        Square::G7
    ));
}

#[test]
fn promotion_is_not_plain() {
    let p = pos("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
    let mv = Move::make_promotion(Square::A7, Square::A8, PieceType::Queen);
    assert!(!is_plain(&p, mv));
}

#[test]
fn castling_is_not_plain() {
    let p = pos("4k3/8/8/8/8/8/8/4K2R w K - 0 1");
    let mv = Move::make_castling(Square::E1, Square::H1);
    assert!(!is_plain(&p, mv));
}

#[test]
fn commoner_move_is_not_plain() {
    assert!(!normal(
        "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
        Square::E1,
        Square::D1
    ));
}

#[test]
fn single_threat_is_not_plain() {
    assert!(!normal(
        "3rk3/8/8/6N1/8/8/8/K7 w - - 0 1",
        Square::G5,
        Square::E6
    ));
}

#[test]
fn double_threat_is_not_plain() {
    assert!(!normal(
        "3rkr2/8/8/6N1/8/8/8/K7 w - - 0 1",
        Square::G5,
        Square::E6
    ));
}

/// The moved piece attacks the enemy commoner from its origin but not from
/// its destination. The origin-square test must reject it (the cheap
/// "abandoned attack" case); uncovered checks through another piece's line
/// remain the documented approximation.
#[test]
fn move_abandoning_an_attack_on_the_commoner_is_not_plain() {
    assert!(!normal(
        "4k3/4R3/8/8/8/8/8/4K3 w - - 0 1",
        Square::E7,
        Square::D7
    ));
}

#[test]
fn quiet_move_that_creates_no_attack_is_grouped_even_with_a_target_present() {
    // The knight on g5 already attacks the rook target on e4? No: the rook is
    // on d8 (a target), and Ng5-f3 can reach neither d8 nor the commoner.
    assert!(normal(
        "3rk3/8/8/6N1/8/8/8/K7 w - - 0 1",
        Square::G5,
        Square::F3
    ));
}

#[test]
fn group_pn_is_the_quiet_number() {
    assert_eq!(GROUP_PN, 9);
}
