//! List all legal moves and the terminal outcome for a FEN.
//!
//! This is the fastest way to validate that a position is parsed as expected
//! and that the atomic-movegen legal move generator agrees with the intended
//! semantics.
//!
//! Default position is the `m19` regression FEN.
//!
//! Usage:
//!     cargo run --example list_legal
//!     cargo run --example list_legal -- "<fen>"

mod common;

use atomic_movegen::types::MoveList;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut fen: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--fen" && i + 1 < args.len() {
            fen = Some(args[i + 1].clone());
            i += 2;
        } else if fen.is_none() {
            fen = Some(args[i].clone());
            i += 1;
        } else {
            i += 1;
        }
    }
    let fen = fen.unwrap_or_else(|| common::M19_FEN.to_string());
    let pos = if fen == "startpos" {
        Position::new()
    } else {
        Position::from_fen(&fen).unwrap()
    };

    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);

    let outcome = pos.outcome();
    println!("fen: {}", pos.fen());
    println!("outcome: {:?}", outcome);
    println!("legal_moves ({}):", moves.len());
    for i in 0..moves.len() {
        println!("  {}", move_to_uci(moves[i]));
    }
}
