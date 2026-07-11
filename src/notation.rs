//! UCI notation for moves.

use atomic_movegen::types::{Move, MoveType, PieceType, Square};

pub fn move_to_uci(m: Move) -> String {
    if m == Move::NONE {
        return "0000".to_string();
    }
    let from = atomic_movegen::types::sq_str(m.from_sq()).unwrap_or("??");
    let to = atomic_movegen::types::sq_str(m.to_sq()).unwrap_or("??");
    let mut s = format!("{from}{to}");
    if m.move_type() == MoveType::Promotion {
        let prom = match m.promotion_type() {
            PieceType::Queen => 'q',
            PieceType::Rook => 'r',
            PieceType::Bishop => 'b',
            PieceType::Knight => 'n',
            _ => 'q',
        };
        s.push(prom);
    }
    s
}

pub fn square_to_uci(sq: Square) -> &'static str {
    atomic_movegen::types::sq_str(sq).unwrap_or("??")
}
