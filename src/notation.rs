//! UCI notation and binary move encodings.

use atomic_movegen::types::{Move, MoveList, MoveType, PROMOTION_PIECES, Square};

use crate::position::Position;

/// Encode an `atomic_movegen` `Move` into a 16-bit code using only the public
/// API.
///
/// The bit layout matches `Move`'s documented encoding:
/// - bits 0-5: `to_sq`
/// - bits 6-11: `from_sq`
/// - bits 12-13: move type
/// - bits 14-15: promotion piece index
///
/// This is the encoding used by the proof-tree binary dump and the TT
/// snapshot (`tt_snapshot`); it never produces `0xFFFF`, which those formats
/// reserve as the `Move::NONE` sentinel.
#[must_use]
pub fn move_to_bits(mv: Move) -> u16 {
    let to = (mv.to_sq() as u16) & 0x3f;
    let from = ((mv.from_sq() as u16) & 0x3f) << 6;
    let type_bits = match mv.move_type() {
        MoveType::Normal => 0u16,
        MoveType::Promotion => 1u16 << 12,
        MoveType::EnPassant => 2u16 << 12,
        MoveType::Castling => 3u16 << 12,
        _ => unreachable!(),
    };
    let promotion_bits = if mv.move_type() == MoveType::Promotion {
        let idx = PROMOTION_PIECES
            .iter()
            .position(|&pt| pt == mv.promotion_type())
            .unwrap_or(0) as u16;
        idx << 14
    } else {
        0u16
    };
    from | to | type_bits | promotion_bits
}

/// Decode a 16-bit move code back into a `Move` using only the public API.
///
/// Returns `None` for codes whose promotion index is out of range.
#[must_use]
pub fn bits_to_move(code: u16) -> Option<Move> {
    let to = Square::from_u8((code & 0x3f) as u8);
    let from = Square::from_u8(((code >> 6) & 0x3f) as u8);
    let move_type_bits = (code >> 12) & 0x3;
    let promotion_idx = ((code >> 14) & 0x3) as usize;

    match move_type_bits {
        0 => Some(Move::make_move(from, to)),
        1 => {
            let pt = *PROMOTION_PIECES.get(promotion_idx)?;
            Some(Move::make_promotion(from, to, pt))
        }
        2 => Some(Move::make_enpassant(from, to)),
        3 => Some(Move::make_castling(from, to)),
        _ => unreachable!(),
    }
}

#[must_use]
pub fn move_to_uci(m: Move) -> String {
    m.to_uci()
}

#[must_use]
pub fn moves_to_uci_path(moves: &[Move]) -> String {
    if moves.is_empty() {
        "root".to_string()
    } else {
        let mut s = "root".to_string();
        for mv in moves {
            s.push('.');
            s.push_str(&move_to_uci(*mv));
        }
        s
    }
}

/// Convert a UCI move string into a legal `Move` for the given position.
///
/// Promotion piece is expected as the optional fifth character of the UCI
/// string (e.g. `c7c8q`).
#[must_use]
pub fn uci_to_move(uci: &str, pos: &Position) -> Option<Move> {
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    for i in 0..moves.len() {
        let mv = moves[i];
        if move_to_uci(mv) == uci {
            return Some(mv);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_move_round_trips() {
        let pos = Position::from_fen(Position::STARTPOS_FEN).unwrap();
        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);
        assert!(!moves.is_empty());
        for i in 0..moves.len() {
            let mv = moves[i];
            let uci = move_to_uci(mv);
            assert_eq!(uci_to_move(&uci, &pos), Some(mv), "{uci} should round-trip");
        }
    }

    #[test]
    fn promotion_round_trips() {
        let pos = Position::from_fen("4k3/1P6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);
        let promotion = moves.as_slice().iter().find(|m| m.is_promotion());
        assert!(promotion.is_some(), "expected promotion moves");
        for mv in moves.as_slice().iter().filter(|m| m.is_promotion()) {
            let uci = move_to_uci(*mv);
            assert_eq!(uci_to_move(&uci, &pos), Some(*mv));
        }
    }

    #[test]
    fn castling_round_trips() {
        let pos =
            Position::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1").unwrap();
        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);
        let castles: Vec<_> = moves
            .as_slice()
            .iter()
            .filter(|m| m.is_castling())
            .copied()
            .collect();
        assert_eq!(
            castles.len(),
            2,
            "expected both king-side and queen-side castles"
        );
        for mv in castles {
            let uci = move_to_uci(mv);
            assert_eq!(uci_to_move(&uci, &pos), Some(mv));
        }
    }

    #[test]
    fn en_passant_round_trips() {
        let pos =
            Position::from_fen("rnbqkbnr/pppppppp/8/8/4pP2/8/PPPP1PPP/RNBQKBNR b KQkq f3 0 2")
                .unwrap();
        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);
        let ep = moves.as_slice().iter().find(|m| m.is_en_passant());
        assert!(ep.is_some(), "expected en-passant capture");
        let mv = ep.unwrap();
        let uci = move_to_uci(*mv);
        assert_eq!(uci_to_move(&uci, &pos), Some(*mv));
    }

    #[test]
    fn malformed_uci_returns_none() {
        let pos = Position::new();
        assert!(uci_to_move("gibberish", &pos).is_none());
        assert!(uci_to_move("e2", &pos).is_none());
        assert!(uci_to_move("e2e9", &pos).is_none());
    }

    #[test]
    fn illegal_uci_returns_none() {
        let pos = Position::new();
        // e2e5 is not a legal pawn move from the starting position.
        assert!(uci_to_move("e2e5", &pos).is_none());
    }
}
