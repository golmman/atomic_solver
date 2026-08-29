//! Shared helpers for integration tests.
//!
//! `M19_FEN` is intentionally duplicated from `examples/common.rs` because
//! example binaries and integration tests cannot share modules.

#![allow(dead_code)]

use atomic_movegen::types::Move;
use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

pub const M19_FEN: &str = "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19";

/// Fixture containing the move-order benchmark positions (m20 to m29).
pub const MOVE_ORDER_FIXTURE: &str = include_str!("../fixtures/move_order_positions.txt");

/// Fixture containing the decisive benchmark positions (dec01 to dec46).
pub const DECISIVE_FIXTURE: &str = include_str!("../fixtures/decisive_positions.txt");

/// Fixture containing decisive positions that are not solved in the default 5-second budget.
pub const DECISIVE_REMAINING_FIXTURE: &str = include_str!("../fixtures/decisive_remaining.txt");

/// Fixture for the always-on smoke suite of the fast tier.
pub const SMOKE_FIXTURE: &str = include_str!("../fixtures/smoke_positions.txt");

/// A single move-order benchmark entry.
#[derive(Debug, Clone)]
pub struct MoveOrderCase {
    pub name: String,
    pub fen: String,
    pub expected: Option<Outcome>,
    pub note: Option<String>,
}

/// Load the move-order benchmark suite from the embedded fixture.
pub fn load_move_order_suite() -> Vec<MoveOrderCase> {
    parse_move_order_fixture(MOVE_ORDER_FIXTURE)
}

/// Load the decisive benchmark suite from the embedded fixture.
pub fn load_decisive_suite() -> Vec<MoveOrderCase> {
    parse_move_order_fixture(DECISIVE_FIXTURE)
}

/// Load the remaining decisive positions that are too hard for the default budget.
pub fn load_decisive_remaining_suite() -> Vec<MoveOrderCase> {
    parse_move_order_fixture(DECISIVE_REMAINING_FIXTURE)
}

/// Load the smoke suite from the embedded fixture.
pub fn load_smoke_suite() -> Vec<MoveOrderCase> {
    parse_move_order_fixture(SMOKE_FIXTURE)
}

fn parse_move_order_fixture(s: &str) -> Vec<MoveOrderCase> {
    s.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.splitn(4, ';');
            let name = parts.next().unwrap_or("").trim().to_string();
            let fen = parts.next().unwrap_or("").trim().to_string();
            let expected = parts.next().and_then(|p| {
                let p = p.trim();
                if p.is_empty() {
                    None
                } else {
                    p.parse::<Outcome>().ok()
                }
            });
            let note = parts
                .next()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(String::from);
            MoveOrderCase {
                name,
                fen,
                expected,
                note,
            }
        })
        .collect()
}

/// Solve `fen` with `secs` timeout and assert that any decisive outcome matches
/// `expected`. A `Draw` result is allowed (it means the solver timed out), but a
/// `Draw` returned without exceeding the time budget is treated as a
/// misclassification and fails the test.
pub fn assert_solves_or_times_out(fen: &str, expected: Outcome, secs: u64) {
    let mut pos =
        Position::from_fen(fen).unwrap_or_else(|e| panic!("failed to parse FEN '{fen}': {e}"));
    let mut search = Search::new(64);
    search.set_timeout(secs);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    assert_ne!(
        (outcome, search.time_exceeded()),
        (Outcome::Draw, false),
        "position {fen} returned Draw without timing out; expected {expected:?}"
    );

    if outcome != Outcome::Draw {
        assert_eq!(
            outcome, expected,
            "expected {expected:?} for {fen}, got {outcome:?}"
        );
    }
}

/// Smoke-suite assertion: solve `fen` with the given timeout and assert that
/// the solver never returns a *wrong* result.
///
/// * decisive `expected`: a decisive outcome must match `expected`; a `Draw`
///   is accepted only when the search was cut short (`time_exceeded()`).
/// * `Outcome::Draw` expected: any decisive outcome is a misclassification,
///   so only `Draw` is accepted. This covers both terminal stalemate draws
///   (which return instantly, without timing out) and non-terminal drawn
///   positions (which exhaust the time budget).
///
/// Phase 3 of `docs/plans/testability/plan3.md` extends the acceptance rule
/// with `child_eval_budget_exceeded()` for the `m22` deep tripwire entry.
pub fn assert_smoke(fen: &str, expected: Outcome, secs: u64) {
    let mut pos =
        Position::from_fen(fen).unwrap_or_else(|e| panic!("failed to parse FEN '{fen}': {e}"));
    let mut search = Search::new(64);
    search.set_timeout(secs);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    if outcome == Outcome::Draw {
        if expected == Outcome::Draw {
            return;
        }
        assert!(
            search.time_exceeded(),
            "position {fen} returned Draw without exceeding the time budget; expected {expected:?}"
        );
    } else {
        assert_eq!(
            outcome, expected,
            "expected {expected:?} for {fen}, got {outcome:?}"
        );
    }
}

/// A fixture note carrying a deterministic child-eval budget, e.g.
/// `solvable_evals:15000000` or `unproven_evals:5000000` (plan3 task 5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalBudget {
    /// The position must solve to its expected outcome within the budget.
    Solvable(u64),
    /// The position must remain unproven within the budget.
    Unproven(u64),
}

/// Parse a fixture note carrying a deterministic child-eval budget.
///
/// The budget is the first whitespace-delimited token after the prefix, so
/// notes may carry a human-readable suffix. Returns `None` for any other note
/// (e.g. `solvable_60s`), so callers can route entries between the
/// deterministic and wall-clock test tiers.
pub fn parse_eval_budget_note(note: &str) -> Option<EvalBudget> {
    let note = note.split_whitespace().next()?;
    if let Some(value) = note.strip_prefix("solvable_evals:") {
        Some(EvalBudget::Solvable(value.parse().ok()?))
    } else if let Some(value) = note.strip_prefix("unproven_evals:") {
        Some(EvalBudget::Unproven(value.parse().ok()?))
    } else {
        None
    }
}

/// Assert that `fen` stays unproven within `budget` cumulative child
/// evaluations (deterministic replacement for the wall-clock stress budget).
///
/// The search must return `Draw` *and* exhaust the budget. A decisive result
/// means the solver improved enough to solve the position and the entry should
/// be re-categorized as solvable; a `Draw` without budget exhaustion means the
/// bounded tree was exhausted, i.e. the position may be genuinely drawn and
/// should also be re-categorized.
pub fn assert_unproven_within_evals(fen: &str, budget: u64) {
    let mut pos =
        Position::from_fen(fen).unwrap_or_else(|e| panic!("failed to parse FEN '{fen}': {e}"));
    let mut search = Search::new(64);
    // The eval budget binds long before the clock; the timeout only guards
    // against hangs on pathologically slow machines.
    search.set_timeout(3600);
    search.set_child_eval_budget(budget);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    assert_eq!(
        outcome,
        Outcome::Draw,
        "position {fen} was proven decisive within the {budget}-eval budget; \
         re-categorize it as solvable and record the expected outcome"
    );
    assert!(
        search.child_eval_budget_exceeded(),
        "position {fen} returned Draw without spending its {budget}-eval budget; \
         it may be genuinely drawn — re-categorize it"
    );
}

/// Assert that `fen` solves to `expected` within `budget` cumulative child
/// evaluations (deterministic replacement for the wall-clock regression
/// budget). Uses first-outcome mode, so the budget bounds the work to the
/// first decisive line.
pub fn assert_solves_within_evals(fen: &str, expected: Outcome, budget: u64) {
    let mut pos =
        Position::from_fen(fen).unwrap_or_else(|e| panic!("failed to parse FEN '{fen}': {e}"));
    let mut search = Search::new(64);
    search.set_timeout(3600);
    search.set_first_outcome_only(true);
    search.set_child_eval_budget(budget);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    assert_eq!(
        outcome, expected,
        "expected {expected:?} for {fen} within {budget} child evals, got {outcome:?}"
    );
    assert!(
        !search.child_eval_budget_exceeded(),
        "position {fen} solved only at/after the {budget}-eval budget boundary; \
         the solver needs more than the budgeted work"
    );
}

/// Assert that `fen` is not proven decisive within `secs`.
///
/// This is the stress-test contract: the solver should return `Draw` only
/// because it ran out of time, not because it found a decisive line.
pub fn assert_unproven_in_secs(fen: &str, secs: u64) {
    let mut pos =
        Position::from_fen(fen).unwrap_or_else(|e| panic!("failed to parse FEN '{fen}': {e}"));
    let mut search = Search::new(64);
    search.set_timeout(secs);

    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    assert!(
        search.time_exceeded(),
        "expected search to time out for unproven stress position {fen}"
    );
    assert_eq!(
        outcome,
        Outcome::Draw,
        "unproven stress position should return Draw on timeout, got {outcome:?} for {fen}"
    );
}

pub fn assert_unproven_in_60s(fen: &str) {
    assert_unproven_in_secs(fen, 60);
}

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
