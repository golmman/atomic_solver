//! UCI notation for moves.

use atomic_movegen::types::{Move, MoveList};

use crate::position::Position;

#[must_use]
pub fn move_to_uci(m: Move) -> String {
    m.to_uci()
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
