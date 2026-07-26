//! Shared helpers for example binaries.

#![allow(dead_code)]

use atomic_movegen::types::{Move, MoveList, MoveType, PieceType, parse_sq};
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;

pub const M19_FEN: &str = "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19";

fn parse_promotion(s: Option<&str>) -> Option<PieceType> {
    match s? {
        "q" => Some(PieceType::Queen),
        "r" => Some(PieceType::Rook),
        "b" => Some(PieceType::Bishop),
        "n" => Some(PieceType::Knight),
        _ => None,
    }
}

pub fn parse_move(pos: &Position, from: &str, to: &str, promo: Option<&str>) -> Option<Move> {
    let from = parse_sq(from)?;
    let to = parse_sq(to)?;
    let promotion = parse_promotion(promo);
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    for i in 0..moves.len() {
        let m = moves[i];
        if m.from_sq() == from && m.to_sq() == to {
            match promotion {
                Some(pt) => {
                    if m.move_type() == MoveType::Promotion && m.promotion_type() == pt {
                        return Some(m);
                    }
                }
                None => return Some(m),
            }
        }
    }
    None
}

pub fn parse_uci(pos: &Position, uci: &str) -> Option<Move> {
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    for i in 0..moves.len() {
        let m = moves[i];
        if move_to_uci(m) == uci {
            return Some(m);
        }
    }
    None
}

#[allow(dead_code)]
fn main() {}
