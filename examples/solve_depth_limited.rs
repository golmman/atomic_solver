//! Run a depth-limited DF-PN+ search on a position.
//!
//! This is useful for probing a position without the full iterative-deepening
//! bootstrap or for checking whether a win is within a small number of plies.
//!
//! Default: the `m19` regression FEN with `max_depth` 4.
//!
//! Usage:
//!     cargo run --example `solve_depth_limited`
//!     cargo run --example `solve_depth_limited` -- "<fen>" <depth>

mod common;

use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

fn main() {
    let mut args = std::env::args().skip(1);
    let fen = args.next().unwrap_or_else(|| common::M19_FEN.to_string());
    let max_depth: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(4);

    let mut pos = Position::from_fen(&fen).unwrap();
    let mut search = Search::new(256);
    search.set_timeout(5);
    let (outcome, pv, nodes) = search.search_depth(&mut pos, max_depth);
    eprintln!("outcome: {outcome:?} nodes: {nodes}");
    for m in pv {
        eprintln!("{}", atomic_solver::notation::move_to_uci(m));
    }
}
