//! Shared helpers for integration tests.

#![allow(dead_code)]

use atomic_movegen::types::Move;
use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

pub const M19_FEN: &str = "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19";

fn solve_with_options(fen: &str, secs: u64, first_outcome_only: bool) -> (Outcome, Vec<Move>, u64) {
    let mut pos =
        Position::from_fen(fen).unwrap_or_else(|e| panic!("failed to parse FEN '{fen}': {e}"));
    let mut search = Search::new(64);
    search.set_timeout(secs);
    search.set_first_outcome_only(first_outcome_only);
    search.solve(&mut pos)
}

pub fn solve(fen: &str) -> Outcome {
    solve_with_pv(fen).0
}

pub fn solve_with_timeout(fen: &str, secs: u64) -> Outcome {
    solve_with_pv_timeout(fen, secs).0
}

pub fn solve_with_pv(fen: &str) -> (Outcome, Vec<String>, u64) {
    solve_with_pv_timeout(fen, 5)
}

pub fn solve_with_pv_timeout(fen: &str, secs: u64) -> (Outcome, Vec<String>, u64) {
    let (outcome, pv, nodes) = solve_with_options(fen, secs, false);
    (outcome, pv_strings(&pv), nodes)
}

pub fn solve_first_outcome(fen: &str) -> (Outcome, Vec<String>, u64) {
    let (outcome, pv, nodes) = solve_with_options(fen, 5, true);
    (outcome, pv_strings(&pv), nodes)
}

pub fn solve_refined_moves(fen: &str) -> (Outcome, Vec<Move>, u64) {
    solve_with_options(fen, 5, false)
}

pub fn solve_refined_moves_timeout(fen: &str, secs: u64) -> (Outcome, Vec<Move>, u64) {
    solve_with_options(fen, secs, false)
}

pub fn pv_strings(pv: &[Move]) -> Vec<String> {
    pv.iter().map(|&m| move_to_uci(m)).collect()
}

pub fn cli_bin() -> String {
    std::env::var("CARGO_BIN_EXE_atomic_solver")
        .unwrap_or_else(|_| "target/debug/atomic_solver".to_string())
}

/// Convert a UCI move list into the internal `Move` representation, replaying
/// each move on `start` and returning the vector of moves. Panics with a useful
/// message if any UCI string is illegal.
pub fn pv_from_uci(start: &Position, uci: &[String]) -> Vec<Move> {
    let mut moves = Vec::with_capacity(uci.len());
    let mut pos = start.clone();
    for u in uci {
        let mv = uci_to_move(u, &pos)
            .unwrap_or_else(|| panic!("PV move '{u}' is not legal in position '{}'", pos.fen()));
        pos.do_move(mv);
        moves.push(mv);
    }
    moves
}

/// Assert that a freshly loaded position satisfies its basic invariants.
pub fn assert_position_invariants(pos: &Position) {
    assert_eq!(
        pos.hash(),
        atomic_solver::zobrist::hash(pos.board(), pos.board().rule50()),
        "incremental hash must match full zobrist hash"
    );

    let legal = pos.legal_moves_vec();
    if legal.is_empty() {
        assert!(
            pos.outcome().is_some(),
            "position with no legal moves must be terminal"
        );
    }
}

/// Assert that `fen` solves to `expected` within the default timeout.
///
/// The `max_pv_len` argument is kept for test compatibility but is no longer
/// enforced.
pub fn assert_solves_to(fen: &str, expected: Outcome, _max_pv_len: Option<usize>) {
    let (outcome, _pv, _nodes) = solve_with_pv(fen);
    assert_eq!(
        outcome, expected,
        "expected {expected:?} for {fen}, got {outcome:?}"
    );
}

/// Assert that `fen` solves to `expected` with the given first move.
pub fn assert_solves_with_first_move(fen: &str, expected: Outcome, first: &str) {
    let (outcome, pv, _nodes) = solve_refined_moves(fen);
    assert_eq!(
        outcome, expected,
        "expected {expected:?} for {fen}, got {outcome:?}"
    );
    let first_mv = pv
        .first()
        .copied()
        .unwrap_or_else(|| panic!("expected a non-empty PV for {fen}"));
    assert_eq!(
        move_to_uci(first_mv),
        first,
        "expected first move {first} for {fen}"
    );
}

/// Assert that `pv` is a valid PV from `fen` ending in `expected`.
pub fn assert_pv_valid(fen: &str, expected: Outcome, pv: &[Move]) {
    if pv.is_empty() && expected != Outcome::Draw {
        panic!("expected a non-empty PV for decisive {expected:?} in {fen}");
    }

    let pos =
        Position::from_fen(fen).unwrap_or_else(|e| panic!("failed to parse FEN '{fen}': {e}"));
    let search = Search::new(1);
    assert!(
        search.validate_pv(pv, &pos, expected, None),
        "PV {:?} does not validate for {fen} expecting {expected:?}",
        pv_strings(pv)
    );
}

/// Assert that `fen` solves to `expected` with the given per-search timeout.
///
/// The `max_pv_len` argument is kept for test compatibility but is no longer
/// enforced.
pub fn assert_solves_to_timeout(
    fen: &str,
    expected: Outcome,
    _max_pv_len: Option<usize>,
    secs: u64,
) {
    let (outcome, _pv, _nodes) = solve_with_pv_timeout(fen, secs);
    assert_eq!(
        outcome, expected,
        "expected {expected:?} for {fen}, got {outcome:?}"
    );
}

/// Assert that `fen` solves to `expected` with the first-outcome setting.
pub fn assert_solves_first_outcome(fen: &str, expected: Outcome) {
    let (outcome, _pv, _nodes) = solve_first_outcome(fen);
    assert_eq!(
        outcome, expected,
        "expected {expected:?} for {fen}, got {outcome:?}"
    );
}
