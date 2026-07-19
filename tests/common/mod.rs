//! Shared helpers for integration tests.

#![allow(dead_code)]

use atomic_movegen::types::Move;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

pub fn solve(fen: &str) -> Outcome {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    outcome
}

pub fn solve_with_timeout(fen: &str, secs: u64) -> Outcome {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(secs);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    outcome
}

pub fn solve_with_pv(fen: &str) -> (Outcome, Vec<String>, u64) {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, pv, nodes) = search.solve(&mut pos);
    (outcome, pv_strings(&pv), nodes)
}

pub fn solve_refined(fen: &str) -> (Outcome, Vec<String>, u64) {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.refine_shortest(true);
    search.set_timeout(5);
    let (outcome, pv, nodes) = search.solve(&mut pos);
    (outcome, pv_strings(&pv), nodes)
}

pub fn solve_refined_moves(fen: &str) -> (Outcome, Vec<Move>, u64) {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.refine_shortest(true);
    search.set_timeout(5);
    search.solve(&mut pos)
}

pub fn pv_strings(pv: &[Move]) -> Vec<String> {
    pv.iter().map(|&m| move_to_uci(m)).collect()
}

pub fn cli_bin() -> String {
    std::env::var("CARGO_BIN_EXE_atomic_solver")
        .unwrap_or_else(|_| "target/debug/atomic_solver".to_string())
}
