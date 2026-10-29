//! Unit tests for the move-class classifier: one fixture per class, the
//! evasion cap, and the range invariant.

use super::*;
use atomic_movegen::types::Square;

/// Play `from`-`to` on `fen` and return the **child** position plus the move.
fn child(fen: &str, from: Square, to: Square) -> (Position, Move) {
    let mut pos = Position::from_fen(fen).unwrap();
    let mv = Move::make_move(from, to);
    assert!(
        pos.try_do_move(mv),
        "fixture move {from:?}-{to:?} must be legal"
    );
    (pos, mv)
}

fn legal_count(pos: &Position) -> usize {
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    moves.len()
}

/// Rh1-h8 checks the black commoner on e8; the child has exactly three legal
/// replies (Kd7, Ke7, Kf7).
#[test]
fn check_starts_at_the_legal_reply_count() {
    let (pos, mv) = child("4k3/8/8/8/8/8/8/4K2R w - - 0 1", Square::H1, Square::H8);
    assert_eq!(legal_count(&pos), 3);
    let (class, pn) = classify_and_start(&pos, mv);
    assert_eq!(class, MoveClass::Check);
    assert_eq!(pn, 3);
}

/// Ra1-e1 checks along the e-file in a busy position with nine legal replies:
/// the count is capped at 8.
#[test]
fn check_reply_count_is_capped_at_eight() {
    let (pos, mv) = child(
        "r1b1kb1r/8/2n3n1/8/8/8/8/R5K1 w - - 0 1",
        Square::A1,
        Square::E1,
    );
    assert!(legal_count(&pos) > 8, "fixture must exceed the cap");
    let (class, pn) = classify_and_start(&pos, mv);
    assert_eq!(class, MoveClass::Check);
    assert_eq!(pn, 8);
}

/// Ng5-e6 attacks both rook targets beside the black commoner on e8.
#[test]
fn double_threat_starts_at_one() {
    let (pos, mv) = child("3rkr2/8/8/6N1/8/8/8/K7 w - - 0 1", Square::G5, Square::E6);
    let (class, pn) = classify_and_start(&pos, mv);
    assert_eq!(class, MoveClass::DoubleThreat);
    assert_eq!(pn, 1);
}

/// Ng5-e6 attacks two rook targets beside the black commoner on e8, but the
/// bishop on d4 attacks b2 in the ring around our own commoner on a1, so the
/// conservative "immediately lost" over-approximation downgrades the double
/// threat to the single-threat class.
#[test]
fn double_threat_downgrades_when_immediately_lost() {
    let (pos, mv) = child(
        "3rkr2/8/8/6N1/3b4/8/1P6/K7 w - - 0 1",
        Square::G5,
        Square::E6,
    );
    let (class, pn) = classify_and_start(&pos, mv);
    assert_eq!(class, MoveClass::Threat);
    assert_eq!(pn, 4);
}

/// Ng5-e6 attacks one rook target beside the black commoner on e8.
#[test]
fn single_threat_starts_at_four() {
    let (pos, mv) = child("3rk3/8/8/6N1/8/8/8/K7 w - - 0 1", Square::G5, Square::E6);
    let (class, pn) = classify_and_start(&pos, mv);
    assert_eq!(class, MoveClass::Threat);
    assert_eq!(pn, 4);
}

/// Same single target, but the rook on e7 can blast the moved knight on e6,
/// which no white piece defends.
#[test]
fn weak_threat_starts_at_eight() {
    let (pos, mv) = child("3rk3/4r3/8/6N1/8/8/8/K7 w - - 0 1", Square::G5, Square::E6);
    let (class, pn) = classify_and_start(&pos, mv);
    assert_eq!(class, MoveClass::WeakThreat);
    assert_eq!(pn, 8);
}

/// Ng1-e2 creates no target and lands on a square the e7 rook attacks.
#[test]
fn hanging_quiet_starts_at_fifteen() {
    let (pos, mv) = child("4k3/4r3/8/8/8/8/8/K5N1 w - - 0 1", Square::G1, Square::E2);
    let (class, pn) = classify_and_start(&pos, mv);
    assert_eq!(class, MoveClass::HangingQuiet);
    assert_eq!(pn, 15);
}

/// Ng1-f3 creates no target and is not attacked.
#[test]
fn plain_quiet_starts_at_nine() {
    let (pos, mv) = child("4k3/8/8/8/8/8/8/K5N1 w - - 0 1", Square::G1, Square::F3);
    let (class, pn) = classify_and_start(&pos, mv);
    assert_eq!(class, MoveClass::Quiet);
    assert_eq!(pn, 9);
}

/// Every class's starting number is in `[1, 15]`, and the evasion argument
/// cannot push the check class out of range.
#[test]
fn starting_numbers_stay_in_range() {
    for class in [
        MoveClass::Check,
        MoveClass::DoubleThreat,
        MoveClass::Threat,
        MoveClass::WeakThreat,
        MoveClass::HangingQuiet,
        MoveClass::Quiet,
    ] {
        for evasions in [0, 1, 7, 8, 9, u32::MAX] {
            let pn = class.starting_proof_number(evasions);
            assert!((1..=15).contains(&pn), "{class:?}/{evasions} -> {pn}");
        }
    }
}

/// The fixtures above are all non-terminal, non-repeating positions; the
/// classifier must never report the check class for a quiet child.
#[test]
fn quiet_child_is_not_a_check() {
    let (pos, mv) = child("4k3/8/8/8/8/8/8/K5N1 w - - 0 1", Square::G1, Square::F3);
    assert_eq!(pos.board().checkers(), Bitboard::EMPTY);
    assert_ne!(classify(pos.board(), mv), MoveClass::Check);
}
