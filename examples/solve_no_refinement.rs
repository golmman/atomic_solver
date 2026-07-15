//! Solve a position without shortest-PV refinement.
//!
//! This is useful for comparing the raw DF-PN+ result to the refined search
//! and for measuring the overhead of the refinement bootstrap.
//!
//! Default: the m27 white-to-move FEN.
//!
//! Usage:
//!     cargo run --example solve_no_refinement
//!     cargo run --example solve_no_refinement -- "<fen>"

use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

fn main() {
    let default = "1R6/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 2 27";
    let fen = std::env::args()
        .nth(1)
        .unwrap_or_else(|| default.to_string());

    let mut pos = Position::from_fen(&fen).unwrap();
    let mut search = Search::new(256);
    search.set_timeout(5);
    let (outcome, pv, nodes) = search.solve(&mut pos);
    eprintln!("outcome: {:?} nodes: {}", outcome, nodes);
    for m in pv {
        eprintln!("{}", atomic_solver::notation::move_to_uci(m));
    }
}
