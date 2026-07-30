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
