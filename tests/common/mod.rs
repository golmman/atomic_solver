//! Shared helpers for integration tests.

#![allow(dead_code)]

use atomic_movegen::types::{Move, MoveList};
use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

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

/// Assert that `fen` solves to `expected` within the default timeout and that the
/// returned PV is valid and (for decisive, non-terminal results) non-empty.
pub fn assert_solves_to(fen: &str, expected: Outcome, max_pv_len: Option<usize>) {
    let (outcome, pv, _nodes) = solve_with_pv(fen);
    assert_eq!(
        outcome, expected,
        "expected {expected:?} for {fen}, got {outcome:?} with pv {pv:?}"
    );

    let pos =
        Position::from_fen(fen).unwrap_or_else(|e| panic!("failed to parse FEN '{fen}': {e}"));
    let is_terminal = pos.outcome().is_some();

    if expected != Outcome::Draw {
        assert_pv_valid(fen, expected, &pv);
        if !is_terminal {
            assert!(
                !pv.is_empty(),
                "expected a non-empty PV for decisive {expected:?} in {fen}, got {pv:?}"
            );
        }
    }
    if let Some(max) = max_pv_len {
        assert!(
            pv.len() <= max,
            "PV length {} exceeds max {max} for {fen}: {pv:?}",
            pv.len()
        );
    }
    if !pv.is_empty() && expected != Outcome::Draw {
        let mut legal = MoveList::new();
        pos.legal_moves(&mut legal);
        let first = uci_to_move(&pv[0], &pos)
            .unwrap_or_else(|| panic!("first PV move '{}' is not legal in {fen}", pv[0]));
        assert!(
            legal.as_slice().contains(&first),
            "first PV move '{}' is not among {} legal moves for {fen}",
            pv[0],
            legal.len()
        );
    }
}

/// Assert that `fen` solves to `expected` with the given per-search timeout and
/// that the returned PV is valid. This is useful for release-only stress tests.
pub fn assert_solves_to_timeout(
    fen: &str,
    expected: Outcome,
    max_pv_len: Option<usize>,
    secs: u64,
) {
    let (outcome, pv, _nodes) = solve_with_pv_timeout(fen, secs);
    assert_eq!(
        outcome, expected,
        "expected {expected:?} for {fen}, got {outcome:?} with pv {pv:?}"
    );

    let pos =
        Position::from_fen(fen).unwrap_or_else(|e| panic!("failed to parse FEN '{fen}': {e}"));
    let is_terminal = pos.outcome().is_some();

    if expected != Outcome::Draw {
        assert_pv_valid(fen, expected, &pv);
        if !is_terminal {
            assert!(
                !pv.is_empty(),
                "expected a non-empty PV for decisive {expected:?} in {fen}, got {pv:?}"
            );
        }
    }
    if let Some(max) = max_pv_len {
        assert!(
            pv.len() <= max,
            "PV length {} exceeds max {max} for {fen}: {pv:?}",
            pv.len()
        );
    }
}

/// Assert that `fen` solves to `expected` and that the first move of the
/// returned PV equals `first_uci`.
pub fn assert_solves_with_first_move(fen: &str, expected: Outcome, first_uci: &str) {
    let (outcome, pv, _nodes) = solve_with_pv(fen);
    assert_eq!(
        outcome, expected,
        "expected {expected:?} for {fen}, got {outcome:?}"
    );
    assert!(
        !pv.is_empty(),
        "expected a non-empty PV for {expected:?} in {fen}"
    );
    assert_eq!(
        pv[0], first_uci,
        "expected first move '{first_uci}', got '{}' for {fen}",
        pv[0]
    );
    assert_pv_valid(fen, expected, &pv);
}

/// Assert that `pv` (as UCI strings) is a valid principal variation for `fen`
/// ending in `expected`.
pub fn assert_pv_valid(fen: &str, expected: Outcome, pv: &[String]) {
    let pos =
        Position::from_fen(fen).unwrap_or_else(|e| panic!("failed to parse FEN '{fen}': {e}"));
    let moves = pv_from_uci(&pos, pv);
    assert!(
        Search::validate_pv(&moves, &pos, expected, None),
        "PV validation failed for {fen}: expected {expected:?}, pv {pv:?}"
    );
}
