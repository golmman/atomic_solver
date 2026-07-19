//! UCI notation for moves.

use atomic_movegen::types::Move;

#[must_use]
pub fn move_to_uci(m: Move) -> String {
    m.to_uci()
}
