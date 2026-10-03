//! plan6 sequential-reference driver (measurement only, not a repo example).
//!
//! Runs one first-outcome sequential solve at product defaults and prints
//! `name wall_s outcome nodes child_evals` — the per-case equivalent of
//! `benchmark --suite move-order --json` for cases that are not in any
//! suite (rem11/rem12). `child_evals` is the inflation denominator.
//!
//! Build: cargo build --release --lib
//! Compile: rustc -O --edition 2024 --extern atomic_solver=<rlib path> \
//!              -L target/release/deps seq_ref.rs -o seq_ref
//! Usage:   ./seq_ref "<name>" "<fen>" <timeout_s>
use atomic_solver::search::dfpn::Search;
use atomic_solver::position::Position;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (name, fen, timeout) = (args[1].clone(), args[2].clone(), args[3].parse::<u64>().unwrap());
    let mut pos = Position::from_fen(&fen).expect("valid FEN");
    let mut search = Search::new(128); // product default TT
    search.set_timeout(timeout);
    search.set_epsilon(0.125);
    search.set_refine_cap_factor(0.25);
    search.set_first_outcome_only(true);
    let start = Instant::now();
    let (outcome, _pv, nodes) = search.solve(&mut pos);
    let wall = start.elapsed().as_secs_f64();
    println!(
        "{}\t{:.3}\t{:?}\t{}\t{}",
        name,
        wall,
        outcome,
        nodes,
        search.child_evaluations()
    );
}
