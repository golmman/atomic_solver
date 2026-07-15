//! Print the static move-order scores for all legal moves in a position.
//!
//! This is useful for debugging the static scorer before adding dynamic
//! heuristics. The list is sorted from highest to lowest score, so the move
//! the solver would try first is at the top.
//!
//! Default position is the m19 regression FEN.
//!
//! Usage:
//!     cargo run --example static_move_scores
//!     cargo run --example static_move_scores -- "<fen>"

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::MoveList;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;
use atomic_solver::search::ordering::{MoveScorer, StaticAtomicScorer};

fn main() {
    let default = "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19";
    let fen = std::env::args()
        .nth(1)
        .unwrap_or_else(|| default.to_string());
    let pos = Position::from_fen(&fen).unwrap();

    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);

    let mut state = StateInfo::new();
    pos.board.populate_state(&mut state);

    let mut scored: Vec<(usize, i32)> = (0..moves.len())
        .map(|i| {
            let m = moves[i];
            (i, StaticAtomicScorer.score(&pos.board, m, &state))
        })
        .collect();
    scored.sort_by_key(|b| std::cmp::Reverse(b.1));

    for (i, s) in scored {
        let m = moves[i];
        println!("{} {}", move_to_uci(m), s);
    }
}
