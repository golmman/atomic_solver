//! Solve a position and print transposition-table statistics.
//!
//! This is useful for debugging graph-history interaction (GHI) issues: a
//! position reached by different move orders should appear in the same TT
//! bucket, with solved entries outnumbering unsolved bounds once the search has
//! finished.
//!
//! Default position is the `m19` regression FEN.
//!
//! Usage:
//!     cargo run --example twin_stats
//!     cargo run --example twin_stats -- "<fen>"

mod common;

use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

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
    let mut pos = if fen == "startpos" {
        Position::new()
    } else {
        Position::from_fen(&fen).unwrap()
    };

    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, pv, nodes) = search.solve(&mut pos);

    let (buckets, live, solved, unsolved, generation) = search.tt_stats();
    let distribution = search.tt_best_child_counts();

    println!("fen: {fen}");
    println!("outcome: {outcome:?}");
    println!(
        "pv: {}",
        pv.iter().map(|m| m.to_uci()).collect::<Vec<_>>().join(" ")
    );
    println!("nodes: {nodes}");
    println!("tt_buckets: {buckets}");
    println!("tt_live_entries: {live}");
    println!("tt_solved_entries: {solved}");
    println!("tt_unsolved_entries: {unsolved}");
    println!("tt_generation: {generation}");
    println!("best_child distribution:");
    for (child, count) in distribution {
        println!("  child {child}: {count}");
    }
}
