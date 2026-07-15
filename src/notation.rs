//! UCI notation for moves.

use atomic_movegen::types::{Move, Square};

pub fn move_to_uci(m: Move) -> String {
    m.to_uci()
}

pub fn square_to_uci(sq: Square) -> &'static str {
    atomic_movegen::types::sq_str(sq).unwrap_or("??")
}
