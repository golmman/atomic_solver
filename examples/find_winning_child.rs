//! Enumerate the first moves of a position and find one that wins.
//!
//! For each legal first move, the child position is solved with a short timeout.
//! A child returning `Outcome::Loss` for the side to move means the root side
//! wins with that first move.
//!
//! Default: the m19 regression FEN.
//!
//! Usage:
//!     cargo run --example find_winning_child
//!     cargo run --example find_winning_child -- "<fen>"

use atomic_movegen::types::MoveList;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

fn main() {
    let default = "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19";
    let fen = std::env::args()
        .nth(1)
        .unwrap_or_else(|| default.to_string());
    let pos = Position::from_fen(&fen).unwrap();

    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);

    for i in 0..moves.len() {
        let mut p = pos.clone();
        let m = moves[i];
        p.do_move(m);

        let mut search = Search::new(128);
        search.set_timeout(5);
        let (outcome, _, nodes) = search.solve(&mut p);
        eprintln!(
            "{} child: outcome={:?} nodes={}",
            move_to_uci(m),
            outcome,
            nodes
        );

        if outcome == Outcome::Loss {
            eprintln!("  WINNING MOVE for root: {}", move_to_uci(m));
            return;
        }
    }
}
