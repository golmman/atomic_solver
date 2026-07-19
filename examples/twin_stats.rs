//! Instrument the twin table while solving GHI-sensitive positions.
//!
//! Usage:
//!     cargo run --example `twin_stats`
//!     cargo run --example `twin_stats` -- "<fen>"

use atomic_movegen::types::{Move, Square};
use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

fn solve_and_report(label: &str, pos: &mut Position) {
    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, _pv, nodes) = search.solve(pos);
    let (insertions, evictions) = search.twin_stats();
    let peak = search.peak_twins();
    println!("{label}:");
    println!("  outcome: {outcome:?}, nodes: {nodes}");
    println!("  twin insertions: {insertions}, evictions: {evictions}");
    println!("  peak live twins in one entry: {peak}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let mut pos = Position::from_fen(&args[1]).unwrap();
        solve_and_report("custom", &mut pos);
        return;
    }

    // Promotion transposition start and transposed board.
    let mut pos = Position::from_fen("4k3/PP6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    solve_and_report("promotion start", &mut pos);

    let mut pos = Position::from_fen("QQ2k3/8/8/8/8/8/8/4K3 b - - 0 1").unwrap();
    solve_and_report("promotion transpose", &mut pos);

    // Cyclic rook-safe-area position.
    let mut pos = Position::from_fen("8/8/8/8/2k5/8/8/4KR2 w - - 0 1").unwrap();
    solve_and_report("cyclic rook safe", &mut pos);

    // Same board after a reversible rook/king shuffle (rule50 changed).
    let mut pos = Position::from_fen("8/8/8/8/2k5/8/8/4KR2 w - - 0 1").unwrap();
    let moves = [
        Move::make_move(Square::F1, Square::G1),
        Move::make_move(Square::C4, Square::B4),
        Move::make_move(Square::G1, Square::F1),
        Move::make_move(Square::B4, Square::C4),
    ];
    for mv in moves {
        pos.do_move(mv);
    }
    solve_and_report("cyclic rook safe (after 4 reversible moves)", &mut pos);
}
