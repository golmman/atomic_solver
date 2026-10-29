//! Shared helpers for example binaries.
//!
//! The fixture parser is single-sourced with `tests/common/mod.rs`: both
//! include `tests/common/fixture_parse.rs` by path (example binaries and
//! integration tests cannot share a library module).

#![allow(dead_code)]

use atomic_movegen::types::{Move, parse_sq};
use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::Position;

#[path = "../tests/common/fixture_parse.rs"]
mod fixture_parse;

// Each example binary uses only a subset of these items; the unused ones are
// the same per-binary compilation artifact the `dead_code` allow above covers.
#[allow(unused_imports)]
pub use fixture_parse::*;

pub const M19_FEN: &str = "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19";

/// Look up a move-order benchmark position by name.
#[must_use]
pub fn move_order_case(name: &str) -> Option<MoveOrderCase> {
    load_move_order_suite()
        .into_iter()
        .find(|case| case.name == name)
}

/// Build a candidate UCI move from the supplied components, convert it to a
/// string, and look it up in the legal moves of `pos`.
#[must_use]
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
