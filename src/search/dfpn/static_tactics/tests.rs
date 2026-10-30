//! Unit tests for the static immediate-explosion closure: the target
//! geometry, the piece rules, and the "returned move is legal" contract.
//!
//! Two of the rules cannot be exercised through a *legal* move list: a capture
//! beside our own commoner destroys it (so `atomic_movegen`'s pseudo-royal
//! rule rejects the move) and a commoner capturer dies in its own blast. They
//! are therefore tested at the level of the helpers that implement them.

use super::*;
use crate::position::Position;
use atomic_movegen::types::Square;

/// Legal move list of `fen` plus the static claim, if any.
fn claim(fen: &str) -> (Option<Move>, MoveList) {
    let pos = Position::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    let found = immediate_exploding_capture(pos.board(), &moves);
    (found, moves)
}

/// Whether the legal move list contains `mv`.
fn contains(moves: &MoveList, mv: Move) -> bool {
    moves.as_slice().contains(&mv)
}

fn targets(fen: &str) -> Bitboard {
    let pos = Position::from_fen(fen).unwrap();
    explosion_targets(pos.board())
}

/// Rd1xd8 captures the black rook beside the black commoner on e8; d8 is not
/// adjacent to our commoner on e1, so the explosion is a one-move win.
#[test]
fn capture_beside_the_enemy_commoner_is_claimed() {
    let (found, moves) = claim("3rk3/8/8/8/8/8/8/3RK3 w - - 0 1");
    assert_eq!(
        found,
        Some(Move::make_move(Square::D1, Square::D8)),
        "the beside-commoner capture must be claimed"
    );
    assert!(
        found.is_some_and(|mv| contains(&moves, mv)),
        "the claim must be one of the supplied legal moves"
    );
}

/// Ra1xa8 captures a rook four files away from the black commoner on e8:
/// nothing explodes, so the position is not claimed.
#[test]
fn capture_far_from_both_commoners_is_not_claimed() {
    let (found, _) = claim("r3k3/8/8/8/8/8/8/R3K3 w - - 0 1");
    assert_eq!(found, None);
    assert_eq!(
        targets("r3k3/8/8/8/8/8/8/R3K3 w - - 0 1"),
        Bitboard::EMPTY,
        "a8 is not beside the commoner"
    );
}

/// The black rook on d2 is beside both commoners (black d3, white d1). The
/// blast would destroy our commoner, so d2 is not a target — and the move
/// generator does not even offer Rxd2.
#[test]
fn target_beside_our_own_commoner_is_rejected() {
    const FEN: &str = "8/8/8/8/8/3k4/R2r4/3K4 w - - 0 1";
    assert_eq!(
        explosion_targets(Position::from_fen(FEN).unwrap().board()),
        Bitboard::EMPTY
    );
    let (found, moves) = claim(FEN);
    assert!(
        !contains(&moves, Move::make_move(Square::A2, Square::D2)),
        "fixture: Rxd2 explodes our own commoner and is illegal"
    );
    assert_eq!(found, None);
}

/// A commoner as the capturing piece is rejected by the piece rule.
#[test]
fn commoner_capturer_is_rejected() {
    // White commoner e1, black commoner e4: e2 is beside the black commoner
    // and (for this synthetic target set) offered as a target.
    let pos = Position::from_fen("8/8/8/8/4k3/8/4r3/4K3 w - - 0 1").unwrap();
    let e2 = Bitboard::square_bb(Square::E2);
    assert!(
        !qualifies(pos.board(), e2, Move::make_move(Square::E1, Square::E2)),
        "a commoner capturer must never qualify"
    );
    // A rook capturer on the same square does.
    let pos = Position::from_fen("8/8/8/8/4k3/8/4r3/4R1K1 w - - 0 1").unwrap();
    assert!(qualifies(
        pos.board(),
        e2,
        Move::make_move(Square::E1, Square::E2)
    ));
}

/// Only captures qualify; quiet moves, empty target sets and en passant never
/// do.
#[test]
fn non_capture_empty_targets_and_en_passant_never_qualify() {
    let pos = Position::from_fen("8/8/8/8/4k3/8/4r3/4R1K1 w - - 0 1").unwrap();
    let e2 = Bitboard::square_bb(Square::E2);
    assert!(!qualifies(
        pos.board(),
        e2,
        Move::make_move(Square::E1, Square::E3)
    ));
    assert!(!qualifies(
        pos.board(),
        Bitboard::EMPTY,
        Move::make_move(Square::E1, Square::E2)
    ));

    // A real en-passant capture is never claimed, even if its destination is
    // handed in as a target (a pawn is destroyed by its own blast and its
    // `to` square is empty on the board, so it is outside the shape).
    let ep = Position::from_fen("7k/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap();
    let mut ep_moves = MoveList::new();
    ep.legal_moves(&mut ep_moves);
    let ep_move = ep_moves
        .as_slice()
        .iter()
        .copied()
        .find(|m| m.from_sq() == Square::E5 && m.to_sq() == Square::D6)
        .expect("fixture: e5d6 must be legal");
    assert!(ep_move.is_en_passant());
    assert!(!qualifies(
        ep.board(),
        Bitboard::square_bb(Square::D6),
        ep_move
    ));
}

/// With two enemy commoners only a square beside *both* is a target; the
/// union would claim a win that leaves a commoner alive.
#[test]
fn multi_commoner_positions_need_every_commoner_in_the_blast() {
    // Black commoners a8 and h8 share no blast zone.
    const APART: &str = "k6k/8/8/8/8/8/8/4RK2 w - - 0 1";
    assert_eq!(targets(APART), Bitboard::EMPTY);
    // Black commoners a8 and c8 with a rook on b8 beside both.
    const BOTH: &str = "krk5/8/8/8/8/8/8/1R4K1 w - - 0 1";
    assert_eq!(targets(BOTH), Bitboard::square_bb(Square::B8));
    let (found, _) = claim(BOTH);
    assert_eq!(found, Some(Move::make_move(Square::B1, Square::B8)));
}

/// No legal move at all, and a list of non-captures, are both unclaimed.
#[test]
fn empty_or_captureless_move_list_returns_none() {
    let pos = Position::from_fen("3rk3/8/8/8/8/8/8/3RK3 w - - 0 1").unwrap();
    let empty = MoveList::new();
    assert_eq!(immediate_exploding_capture(pos.board(), &empty), None);

    let mut quiet_only = MoveList::new();
    quiet_only.push(Move::make_move(Square::D1, Square::D2));
    quiet_only.push(Move::make_move(Square::E1, Square::F1));
    assert_eq!(immediate_exploding_capture(pos.board(), &quiet_only), None);
}

/// The claim is drawn from the supplied list only: no pseudo-legal
/// reconstruction happens.
#[test]
fn claim_is_drawn_from_the_supplied_list_only() {
    let pos = Position::from_fen("3rk3/8/8/8/8/8/8/3RK3 w - - 0 1").unwrap();
    let winning = Move::make_move(Square::D1, Square::D8);
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    let mut filtered = MoveList::new();
    for mv in &moves {
        if mv != winning {
            filtered.push(mv);
        }
    }
    assert_eq!(immediate_exploding_capture(pos.board(), &filtered), None);
    assert_eq!(
        immediate_exploding_capture(pos.board(), &moves),
        Some(winning)
    );
}
