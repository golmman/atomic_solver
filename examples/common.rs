//! Shared helpers for example binaries.
//!
//! `M19_FEN` is intentionally duplicated from `tests/common/mod.rs` because
//! example binaries and integration tests cannot share modules.

#![allow(dead_code)]

use atomic_movegen::types::{Move, parse_sq};
use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::Position;

pub const M19_FEN: &str = "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19";

/// Build a candidate UCI move from the supplied components, convert it to a
/// string, and look it up in the legal moves of `pos`.
pub fn parse_move(pos: &Position, from: &str, to: &str, promo: Option<&str>) -> Option<Move> {
    let from = parse_sq(from)?;
    let to = parse_sq(to)?;
    let candidate = if let Some(p) = promo {
        let pt = match p {
            "q" => atomic_movegen::types::PieceType::Queen,
            "r" => atomic_movegen::types::PieceType::Rook,
            "b" => atomic_movegen::types::PieceType::Bishop,
            "n" => atomic_movegen::types::PieceType::Knight,
            _ => return None,
        };
        Move::make_promotion(from, to, pt)
    } else {
        Move::make_move(from, to)
    };
    uci_to_move(&move_to_uci(candidate), pos)
}

#[allow(dead_code)]
fn main() {}
