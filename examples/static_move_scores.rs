//! Print the static move-order scores for all legal moves in a position.
//!
//! This is useful for debugging the static scorer before adding dynamic
//! heuristics. The list is sorted from highest to lowest score, so the move
//! the solver would try first is at the top.
//!
//! Default position is the `m19` regression FEN. Use `--name <case>` to inspect
//! one of the move-order benchmark positions.
//!
//! Usage:
//!     cargo run --example `static_move_scores`
//!     cargo run --example `static_move_scores` -- --name m25_white
//!     cargo run --example `static_move_scores` -- "<fen>"

mod common;

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::MoveList;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;
use atomic_solver::search::ordering::{StaticAtomicScorer, nearest_commoner_map};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut fen: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--name" if i + 1 < args.len() => {
                let name = &args[i + 1];
                fen = Some(
                    common::move_order_case(name)
                        .unwrap_or_else(|| panic!("unknown benchmark name '{name}'"))
                        .fen,
                );
                i += 2;
            }
            _ if fen.is_none() => {
                fen = Some(args[i].clone());
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    let fen = fen.unwrap_or_else(|| common::M19_FEN.to_string());
    let pos = Position::from_fen(&fen).unwrap();

    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);

    let mut state = StateInfo::new();
    pos.populate_state(&mut state);

    let nearest = nearest_commoner_map(pos.board(), pos.side_to_move().flip());

    let mut scored: Vec<(usize, i32)> = (0..moves.len())
        .map(|i| {
            let m = moves[i];
            (
                i,
                StaticAtomicScorer.score_with_map(pos.board(), m, &state, &nearest),
            )
        })
        .collect();
    scored.sort_by_key(|b| std::cmp::Reverse(b.1));

    for (i, s) in scored {
        let m = moves[i];
        let uci = move_to_uci(m);
        println!("{uci} {s}");
    }
}
