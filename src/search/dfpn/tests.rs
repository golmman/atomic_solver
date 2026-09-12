//! Cross-module DF-PN unit tests.

use super::repetition_cache::RepetitionCache;
use super::{PvStatus, RoundTermination};
use crate::position::{Outcome, Position};
use crate::search::dfpn::Search;
use atomic_movegen::types::{Move, Square};

#[test]
fn repetition_cache_reused_within_a_run() {
    // plan9: a second bounded search over the cyclic rook position within the
    // same run (same `Search` instance, no `begin_run` in between) must hit
    // the per-search repetition cache — the draw proofs cached by the first
    // search are reused instead of being re-walked — and both searches must
    // return `Draw`, never `Win`. The cache is cleared only per run in
    // `begin_run`, so within-run reuse across work chunks is the mechanism
    // the stress case pays on.
    const FEN: &str = "8/8/8/8/2k5/8/8/4KR2 w - - 0 1";
    const ROUND_CAP: u64 = 2_000_000;

    let mut pos = Position::from_fen(FEN).unwrap();
    let mut search = Search::new(64);
    search.begin_run();

    let (first, _, _) = search.bounded_search(&mut pos, u32::MAX, ROUND_CAP);
    assert_eq!(first, Outcome::Draw, "the cyclic position must not be won");
    assert!(
        !search.repetition_cache.is_empty(),
        "the first search must have proven repetition-dependent draws"
    );
    let hits_before = search.repetition_cache.hits;

    // Second bounded search in the same run: the cached entries must be
    // probed and reused (no `begin_run` in between, so no cache clear).
    let (second, _, _) = search.bounded_search(&mut pos, u32::MAX, ROUND_CAP);
    assert_eq!(second, Outcome::Draw, "the second search must stay a draw");
    assert!(
        search.repetition_cache.hits > hits_before,
        "the second bounded search must reuse cached repetition draws"
    );

    // A cache-disabled run over the same protocol must agree on the outcome:
    // the cache may only reduce work, never change the decisive result.
    let mut pos = Position::from_fen(FEN).unwrap();
    let mut search = Search::new(64);
    search.repetition_cache = RepetitionCache::with_capacity(0);
    search.begin_run();
    let (first, _, _) = search.bounded_search(&mut pos, u32::MAX, ROUND_CAP);
    let (second, _, _) = search.bounded_search(&mut pos, u32::MAX, ROUND_CAP);
    assert_eq!(first, Outcome::Draw);
    assert_eq!(second, Outcome::Draw);
}

#[test]
fn local_repetition_in_prefix_returns_draw() {
    // The same cyclic rook-safe-area position, reached after a reversible
    // rook/king shuffle. Its own repetition key is supplied as a prefix, so
    // the solver should short-circuit to a draw.
    let mut pos = Position::from_fen("8/8/8/8/2k5/8/8/4KR2 w - - 0 1").unwrap();
    pos.do_move(Move::make_move(Square::F1, Square::G1));
    pos.do_move(Move::make_move(Square::C4, Square::B4));
    pos.do_move(Move::make_move(Square::G1, Square::F1));
    pos.do_move(Move::make_move(Square::B4, Square::C4));

    let rep_key = pos.repetition_key();
    let mut search = Search::new(64);
    search.set_timeout(5);

    let (outcome, depth, _nodes) = search.search_depth_with_prefix(&mut pos, u32::MAX, &[rep_key]);
    assert_eq!(outcome, Outcome::Draw);
    assert_eq!(depth, 0);
}

#[test]
fn tt_resolved_rejects_win_when_best_move_repeats() {
    // Store a win for a position whose winning move leads to a board already
    // on the search path. The one-ply repetition guard should reject the
    // cached result.
    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let key = pos.hash();
    let win_move = Move::make_move(Square::E1, Square::E8);

    let mut child = pos.clone();
    child.do_move(win_move);
    let child_rep_key = child.repetition_key();

    let mut search = Search::new(64);
    search.tt.store(
        key,
        win_move,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        crate::zobrist::INF,
        1,
        u32::MAX,
    );

    let entry = search.tt.probe(key).copied().unwrap();

    // With the child on the path, the cached win is invalid.
    search.path_stack.push(child_rep_key);
    assert!(
        search.best_move_repeats_path(&mut pos.clone(), win_move),
        "the one-ply guard should reject a win whose best move repeats a board on the path"
    );

    // Without the child on the path, the cached win is valid.
    search.path_stack.clear();
    assert!(
        !search.best_move_repeats_path(&mut pos.clone(), win_move),
        "cached win should be accepted when the child is not on the path"
    );
    let resolved = Search::resolved_from_entry(&entry, u32::MAX)
        .expect("cached win should resolve when the child is not on the path");
    assert_eq!(resolved.outcome, Outcome::Win);
    assert_eq!(resolved.depth, 1);
}

#[test]
fn set_timeout_zero_causes_immediate_exit() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(0);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Draw, "timeout 0 should return Draw");
    assert!(
        search.time_exceeded(),
        "time should be exceeded after timeout 0"
    );
    assert!(
        matches!(
            search.exit_reason(),
            crate::search::dfpn::ExitReason::Timeout
        ),
        "exit reason should be Timeout"
    );
}

#[test]
fn first_outcome_only_skips_refinement() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_first_outcome_only(true);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
    assert!(
        !pv.is_empty(),
        "first-outcome mode should still return a winning PV"
    );
}

#[test]
fn solve_with_progress_calls_closure() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);

    let mut calls = 0;
    let (outcome, _pv, _nodes) = search.solve_with_progress(&mut pos, |_o, _pv| {
        calls += 1;
    });
    assert_eq!(outcome, Outcome::Win);
    assert!(
        calls > 0,
        "progress closure should be invoked at least once"
    );
}

#[test]
fn exit_reason_reports_complete() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let _ = search.solve(&mut pos);
    assert!(
        matches!(
            search.exit_reason(),
            crate::search::dfpn::ExitReason::Complete
        ),
        "a solved position should report Complete"
    );
}

#[test]
fn child_eval_budget_zero_causes_immediate_exit() {
    let mut pos = Position::from_fen("4k3/PP6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_child_eval_budget(0);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Draw, "budget 0 should return Draw");
    assert!(
        pv.is_empty(),
        "expected an empty PV after immediate budget cutoff, got {pv:?}"
    );
    assert!(
        search.child_eval_budget_exceeded(),
        "budget should be exceeded"
    );
    assert!(
        !search.time_exceeded(),
        "budget exhaustion must not be reported as a timeout"
    );
    assert_eq!(
        search.exit_reason(),
        crate::search::dfpn::ExitReason::BudgetExhausted,
        "exit reason should be BudgetExhausted"
    );
    let (_buckets, live_entries, solved_entries, _unsolved, _generation) = search.tt_stats();
    assert_eq!(live_entries, 0, "a budget-0 search must not store anything");
    assert_eq!(
        solved_entries, 0,
        "a budget-0 search must not cache a proven entry"
    );
}

#[test]
fn child_eval_budget_generous_still_solves() {
    // The full refined solve of the promotion-transposition position measures
    // W = 426,882 child evaluations (deterministic); 5,000,000 is ~10x W.
    let mut pos = Position::from_fen("4k3/PP6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_child_eval_budget(5_000_000);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win, "generous budget should still solve");
    assert!(!pv.is_empty(), "solved search should return a PV");
    assert!(
        !search.child_eval_budget_exceeded(),
        "the budget must not be exhausted by a solve inside it"
    );
    assert_eq!(
        search.exit_reason(),
        crate::search::dfpn::ExitReason::Complete
    );
}

#[test]
fn child_eval_budget_fraction_does_not_solve() {
    // The first decisive line needs W1 = 7,449 child evaluations; a budget
    // below that must return Draw, report the budget (not a timeout), and
    // cache only unsolved entries.
    let mut pos = Position::from_fen("4k3/PP6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_child_eval_budget(5_000);

    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(
        outcome,
        Outcome::Draw,
        "a budget below the solve effort must return Draw"
    );
    assert!(search.child_eval_budget_exceeded());
    assert!(
        !search.time_exceeded(),
        "budget exhaustion must not be reported as a timeout"
    );
    assert_eq!(
        search.exit_reason(),
        crate::search::dfpn::ExitReason::BudgetExhausted
    );

    // A budget-cut result must not poison the transposition table: re-solving
    // the same position on the same Search (and therefore the same TT) with
    // an unbounded budget must still find the win.
    search.set_child_eval_budget(u64::MAX);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(
        outcome,
        Outcome::Win,
        "a budget-cut search must not cache a proven result that hides the win"
    );
}

#[test]
fn budget_exhausted_pv_is_empty_or_valid() {
    // Tiny budget on a deeper position: the informational PV must never be a
    // wrong decisive line, so it must be empty or a fully legal line.
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_child_eval_budget(5);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Draw);
    assert!(search.child_eval_budget_exceeded());
    let mut current = pos.clone();
    for mv in &pv {
        assert!(
            current.try_do_move(*mv),
            "PV move must be legal in a budget-exhausted search"
        );
    }
}

/// Fixture that triggers at least one improving refinement round (50 -> 48
/// plies) and fully converges in well under 100 ms (release).
const REFINE_FIXTURE_FEN: &str = "r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35";

#[test]
fn refinement_counters_accumulate() {
    let mut pos = Position::from_fen(REFINE_FIXTURE_FEN).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Loss);
    assert!(!pv.is_empty());

    assert!(
        search.first_outcome_evaluations() > 0,
        "first-outcome phase work must be recorded"
    );
    assert!(
        search.refinement_rounds() >= 1,
        "the fixture must trigger at least one refinement round"
    );
    assert!(
        search.child_evaluations() >= search.first_outcome_evaluations(),
        "nodes/child_evals accumulate across refinement rounds (cumulative semantics)"
    );
    assert_eq!(
        search.refinement_evaluations(),
        search.child_evaluations() - search.first_outcome_evaluations(),
        "refinement_evals is the delta between total and first-outcome work"
    );
}

#[test]
fn refine_cap_zero_disables_capping() {
    // Factor 0.0 must preserve the uncapped (pre-cap) behavior: the fixture
    // still refines, so at least one round runs.
    let mut pos = Position::from_fen(REFINE_FIXTURE_FEN).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_refine_cap_factor(0.0);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Loss);
    assert!(!pv.is_empty());
    assert!(
        search.refinement_rounds() >= 1,
        "factor 0.0 disables capping, refinement must still run"
    );
}

#[test]
fn refine_cap_bounds_round_work() {
    let mut pos = Position::from_fen(REFINE_FIXTURE_FEN).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    // Force a round cap far below the first-outcome work. On this fixture the
    // rounds are TT-cushioned and complete within the cap (see
    // `refine_cap_cut_reports_unproven_status` for a cap that actually
    // binds), so this verifies the capped-round accounting: rounds stay far
    // below the first-outcome work and near the cap itself.
    search.set_refine_round_cap_for_test(1_000);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    // A cap-cut round is abandoned: the best result so far is returned.
    assert_eq!(outcome, Outcome::Loss);
    assert!(!pv.is_empty());
    assert!(
        search.refinement_rounds() >= 1,
        "at least one refinement round must run"
    );
    assert!(
        search.refinement_evaluations() < search.first_outcome_evaluations(),
        "capped rounds must spend far less work than the first-outcome phase"
    );
    // Every round is bounded by the 1_000-eval cap plus dfpn's bounded
    // overshoot (children evaluated after the last work check), so the total
    // refinement work stays within a small constant times the round count.
    assert!(
        search.refinement_evaluations() <= u64::from(search.refinement_rounds()) * 10_000,
        "each capped round must stay near the 1_000-eval cap"
    );
}

#[test]
fn solve_twice_identical_counts() {
    // Pool/scratch state must not leak between runs: solving dec44 twice
    // with fresh `Search` objects yields identical `nodes` and `child_evals`.
    let mut results = Vec::new();
    for _ in 0..2 {
        let mut pos = Position::from_fen(REFINE_FIXTURE_FEN).unwrap();
        let mut search = Search::new(64);
        search.set_timeout(5);
        let (outcome, pv, nodes) = search.solve(&mut pos);
        results.push((outcome, pv.len(), nodes, search.child_evaluations()));
    }
    assert_eq!(
        results[0], results[1],
        "identical solve, identical counters"
    );
}

#[test]
fn default_refine_cap_leaves_improving_rounds() {
    // The plan1 default cap (factor 0.25, 1M-eval floor) must not bind on the
    // quick fixtures: dec44 refines 50 -> 48 plies and converges, so with the
    // default factor the run still performs at least one improving round and
    // reaches the converged 48-ply PV.
    let mut pos = Position::from_fen(REFINE_FIXTURE_FEN).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Loss);
    assert_eq!(pv.len(), 48, "the converged shortest PV is 48 plies");
    assert!(
        search.refinement_rounds() >= 1,
        "default cap must leave improving refinement rounds"
    );
}

#[test]
fn bounded_search_reports_exhaustion_below_mate_depth() {
    // KRR vs K has no mate in 1, so a depth-1 bound fully explores the
    // 1-ply tree without a decisive line.
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.begin_run();
    let (outcome, _pv, termination) = search.bounded_search(&mut pos, 1, u64::MAX);
    assert_eq!(outcome, Outcome::Draw);
    assert_eq!(termination, RoundTermination::Exhausted);
}

#[test]
fn bounded_search_reports_resource_cut_on_budget() {
    // A global child-eval budget smaller than the root's branching factor
    // cuts the very first chunk: the loop condition exits with the budget
    // spent, which must be reported as a resource cut (not exhaustion).
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_child_eval_budget(10);
    search.begin_run();
    let (outcome, _pv, termination) = search.bounded_search(&mut pos, u32::MAX, u64::MAX);
    assert_eq!(outcome, Outcome::Draw);
    assert_eq!(termination, RoundTermination::ResourceCut);
}

#[test]
fn bounded_search_reports_cap_cut_on_round_cap() {
    // A per-round cap smaller than the root's branching factor exhausts the
    // round cap but not the global budget: the `call_max_work == 0` break
    // must be attributed to the round cap.
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.begin_run();
    let (outcome, _pv, termination) = search.bounded_search(&mut pos, u32::MAX, 5);
    assert_eq!(outcome, Outcome::Draw);
    assert_eq!(termination, RoundTermination::CapCut);
}

#[test]
fn converged_fixture_is_proven_shortest() {
    // With default settings the dec44 fixture refines 50 -> 48 plies and the
    // final round (bound 46) exhausts naturally, proving no win of length
    // <= 46 exists: the 48-ply PV is proven shortest.
    let mut pos = Position::from_fen(REFINE_FIXTURE_FEN).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Loss);
    assert_eq!(pv.len(), 48, "the converged shortest PV is 48 plies");
    assert_eq!(
        search.pv_status(),
        PvStatus::ProvenShortest,
        "the final refinement round must exhaust naturally"
    );
    assert!(
        !search.last_refine_round_cap_cut(),
        "a proven-shortest result must not carry the cap-cut flag"
    );
}

#[test]
fn refine_cap_cut_reports_unproven_status() {
    // The fixture's refinement rounds are TT-cushioned and cheap (round 1 is
    // decisive after ~52 evals, the final bound-46 round exhausts naturally
    // after ~340 evals), so the cap must sit below that exhaustion cost to
    // actually bind. At 100 evals the final round is cut by the per-round
    // cap: the PV is unproven, with the cap-cut cause.
    let mut pos = Position::from_fen(REFINE_FIXTURE_FEN).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_refine_round_cap_for_test(100);

    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Loss);
    assert_eq!(search.pv_status(), PvStatus::Unproven);
    assert!(
        search.last_refine_round_cap_cut(),
        "the round was cut by the per-round cap, not a resource limit"
    );
}

#[test]
fn resource_cut_reports_unproven_without_cap_flag() {
    // A global child-eval budget that admits the first outcome (7,449 evals
    // on the promotion fixture) but is spent before the bounded refinement
    // tree can be exhausted: unproven via resource cut, not cap cut.
    let mut pos = Position::from_fen("4k3/PP6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_child_eval_budget(8_000);

    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
    assert_eq!(search.pv_status(), PvStatus::Unproven);
    assert!(
        !search.last_refine_round_cap_cut(),
        "the round was cut by the global budget, not the per-round cap"
    );
}

#[test]
fn first_outcome_only_reports_first_outcome_status() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_first_outcome_only(true);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
    assert_eq!(search.pv_status(), PvStatus::FirstOutcome);
}

#[test]
fn drawn_position_reports_none_status() {
    // Bare kings are a terminal Draw (occupied == 2): no PV to qualify.
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Draw);
    assert!(pv.is_empty());
    assert_eq!(search.pv_status(), PvStatus::None);
}
