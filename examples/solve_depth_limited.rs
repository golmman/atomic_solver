//! Run a depth-limited DF-PN+ search on a position.
//!
//! This is useful for probing a position without the full iterative-deepening
//! bootstrap or for checking whether a win is within a small number of plies.
//!
//! Default: the m19 regression FEN with max_depth 4.
//!
//! Usage:
//!     cargo run --example solve_depth_limited
//!     cargo run --example solve_depth_limited -- "<fen>" <depth>

use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

fn main() {
    let default_fen = "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19";
    let mut args = std::env::args().skip(1);
    let fen = args.next().unwrap_or_else(|| default_fen.to_string());
    let max_depth: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(4);

    let mut pos = Position::from_fen(&fen).unwrap();
    let mut search = Search::new(256);
    search.set_timeout(5);
    let (outcome, pv, nodes) = search.search_depth(&mut pos, max_depth);
    eprintln!("outcome: {:?} nodes: {}", outcome, nodes);
    for m in pv {
        eprintln!("{}", atomic_solver::notation::move_to_uci(m));
    }
}
