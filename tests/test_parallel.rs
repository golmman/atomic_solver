//! Integration tests for the opt-in parallel SPDFPN mode (`--threads N`,
//! `docs/plans/parallel/plan5.md`).
//!
//! Soundness contract under test: with N > 1 the run is nondeterministic in
//! which valid proof wins and in work counts, but the decisive outcome must
//! always agree with the sequential solver (N = 1) — never a false decisive
//! outcome. With N = 1 the parallel machinery is never constructed.

use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

const WIN_FEN: &str = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";
const DEC44_FEN: &str = "r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35";

fn solve(fen: &str, threads: usize) -> (Outcome, usize) {
    let mut search = Search::new(16);
    search.set_threads(threads);
    search.set_timeout(20);
    let mut pos = Position::from_fen(fen).unwrap();
    let (outcome, pv, _) = search.solve(&mut pos);
    (outcome, pv.len())
}

#[test]
fn threads_one_matches_default_sequential_surface() {
    // N = 1 must be exactly the sequential solver (decision 2): identical
    // outcome and child-eval count with and without the explicit setting.
    let mut a = Search::new(16);
    a.set_timeout(20);
    let mut pos_a = Position::from_fen(DEC44_FEN).unwrap();
    let (out_a, _, evals_a) = a.solve(&mut pos_a);

    let mut b = Search::new(16);
    b.set_threads(1);
    b.set_timeout(20);
    let mut pos_b = Position::from_fen(DEC44_FEN).unwrap();
    let (out_b, _, evals_b) = b.solve(&mut pos_b);

    assert_eq!(out_a, out_b);
    assert_eq!(evals_a, evals_b, "N = 1 must be the sequential path");
}

#[test]
fn parallel_win_agrees_with_sequential() {
    let (seq_outcome, _) = solve(WIN_FEN, 1);
    assert_eq!(seq_outcome, Outcome::Win);
    for threads in [2, 4] {
        let (outcome, _) = solve(WIN_FEN, threads);
        assert_eq!(outcome, seq_outcome, "threads={threads}");
    }
}

#[test]
fn parallel_loss_agrees_with_sequential() {
    let (seq_outcome, _) = solve(DEC44_FEN, 1);
    assert_eq!(seq_outcome, Outcome::Loss);
    for threads in [2, 4] {
        let (outcome, _) = solve(DEC44_FEN, threads);
        assert_eq!(outcome, seq_outcome, "threads={threads}");
    }
}

#[test]
fn parallel_runs_are_repeatable_in_outcome() {
    // Repeated N > 1 solves may differ in PV (nondeterministic which valid
    // proof wins) but never in the decisive outcome.
    for threads in [2, 4] {
        for _ in 0..2 {
            let (outcome, _) = solve(WIN_FEN, threads);
            assert_eq!(outcome, Outcome::Win, "threads={threads}");
        }
    }
}

#[test]
fn threads_zero_panics() {
    let result = std::panic::catch_unwind(|| {
        let mut search = Search::new(1);
        search.set_threads(0);
    });
    assert!(result.is_err(), "set_threads(0) must panic");
}
