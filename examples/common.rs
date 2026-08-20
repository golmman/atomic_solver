//! Shared helpers for example binaries.
//!
//! `M19_FEN` is intentionally duplicated from `tests/common/mod.rs` because
//! example binaries and integration tests cannot share modules.

#![allow(dead_code)]

use atomic_movegen::types::{Move, parse_sq};
use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::{Outcome, Position};

pub const M19_FEN: &str = "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19";

/// Fixture containing the move-order benchmark positions (m20 to m29).
pub const MOVE_ORDER_FIXTURE: &str = include_str!("../tests/fixtures/move_order_positions.txt");

/// Fixture containing the decisive benchmark positions (dec01 to dec23).
pub const DECISIVE_FIXTURE: &str = include_str!("../tests/fixtures/decisive_positions.txt");

/// A single move-order benchmark entry.
#[derive(Debug, Clone)]
pub struct MoveOrderCase {
    pub name: String,
    pub fen: String,
    pub expected: Option<Outcome>,
    pub note: Option<String>,
}

/// Load the move-order benchmark suite from the embedded fixture.
pub fn load_move_order_suite() -> Vec<MoveOrderCase> {
    parse_move_order_fixture(MOVE_ORDER_FIXTURE)
}

/// Look up a move-order benchmark position by name.
pub fn move_order_case(name: &str) -> Option<MoveOrderCase> {
    load_move_order_suite()
        .into_iter()
        .find(|case| case.name == name)
}

/// Load the decisive benchmark suite from the embedded fixture.
pub fn load_decisive_suite() -> Vec<MoveOrderCase> {
    parse_move_order_fixture(DECISIVE_FIXTURE)
}

/// Look up a decisive benchmark position by name.
pub fn decisive_case(name: &str) -> Option<MoveOrderCase> {
    load_decisive_suite()
        .into_iter()
        .find(|case| case.name == name)
}

fn parse_move_order_fixture(s: &str) -> Vec<MoveOrderCase> {
    s.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.splitn(4, ';');
            let name = parts.next().unwrap_or("").trim().to_string();
            let fen = parts.next().unwrap_or("").trim().to_string();
            let expected = parts.next().and_then(|p| {
                let p = p.trim();
                if p.is_empty() {
                    None
                } else {
                    p.parse::<Outcome>().ok()
                }
            });
            let note = parts
                .next()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(String::from);
            MoveOrderCase {
                name,
                fen,
                expected,
                note,
            }
        })
        .collect()
}

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
