//! Cross-module DF-PN unit tests.

use super::repetition_cache::RepetitionCache;
use super::{ExitReason, PvStatus, RestartSchedule, RoundTermination};
use crate::position::{Outcome, Position};
use crate::search::dfpn::Search;
use crate::zobrist::INF;
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
    search.path_push(child_rep_key);
    assert!(
        search.best_move_repeats_path(&mut pos.clone(), win_move),
        "the one-ply guard should reject a win whose best move repeats a board on the path"
    );

    // Without the child on the path, the cached win is valid.
    search.reset_search_state();
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
    // 4 men (outside the plan13 pre-phase class): the pre-phase never
    // consults the wall clock, so a timeout-0 run must be observed on an
    // ungated fixture for the timeout semantics under test.
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
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
    // 4 men (outside the plan13 pre-phase class): a pre-phase claim returns
    // directly without invoking the progress closure.
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
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
fn bounded_search_reports_resource_cut_when_budget_and_cap_tie() {
    // Budget == cap: both constraints are spent at the same time. Today's
    // tie rule reports the resource cut (the budget was spent too).
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_child_eval_budget(5);
    search.begin_run();
    let (outcome, _pv, termination) = search.bounded_search(&mut pos, u32::MAX, 5);
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

#[test]
fn budget_overshoot_is_bounded() {
    // The per-node budget check is crossed by at most the remainder of one
    // `evaluate_all_children` sweep (< 256 child evals).
    const FEN: &str = "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22";
    for budget in [1_000u64, 10_000, 100_000, 1_000_000] {
        let mut pos = Position::from_fen(FEN).unwrap();
        let mut search = Search::new(64);
        search.set_timeout(60);
        search.set_child_eval_budget(budget);
        let (outcome, _pv, _nodes) = search.search_depth(&mut pos, u32::MAX);
        assert_eq!(outcome, Outcome::Draw, "budget {budget}");
        assert_eq!(search.exit_reason(), ExitReason::BudgetExhausted);
        assert!(
            search.child_evaluations() <= budget + 256,
            "budget {budget} overshot to {}",
            search.child_evaluations()
        );
    }
}

#[test]
fn never_restarts() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_restart_schedule(RestartSchedule::NEVER);
    let (outcome, _pv, _nodes) = search.search_depth(&mut pos, u32::MAX);
    assert_eq!(outcome, Outcome::Win);
    assert_eq!(search.restarts(), 0);
}

#[test]
fn shipped_schedule_restarts_past_the_first_slice() {
    // dec31 needs 510,426 child evals: just past the shipped 500,000 slice,
    // so exactly one restart boundary is crossed.
    let mut pos =
        Position::from_fen("r1bq1k2/ppN3p1/2p1p2P/6P1/1b1P1p2/8/PP3P2/R2K1B1R b - - 0 15").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    assert_eq!(search.restart_schedule(), RestartSchedule::SHIPPED);
    let (outcome, _pv, _nodes) = search.search_depth(&mut pos, u32::MAX);
    assert_eq!(outcome, Outcome::Loss);
    assert!(search.restarts() >= 1);
}

#[test]
fn linear_schedule_restarts_end_to_end() {
    // m22 first-outcome at tt 256 under a linear schedule (slices
    // 500k, 1M, 1.5M, ...). Cumulative spend at the first boundaries is
    // 0.5, 1.5, 3.0, 5.0, 7.5, 10.5, 14.0, 18.0 M; a cold m22 solve needs
    // ~19.4 M under RestartSchedule::NEVER, so at least 7 boundaries are
    // necessarily crossed before a slice can be large enough.
    const FEN: &str = "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22";
    let mut pos = Position::from_fen(FEN).unwrap();
    let mut search = Search::new(256);
    search.set_timeout(60);
    search.set_restart_schedule(RestartSchedule::linear(500_000, 500_000));
    let (outcome, _pv, _nodes) = search.search_depth(&mut pos, u32::MAX);
    assert_eq!(outcome, Outcome::Win);
    assert!(
        search.restarts() >= 7,
        "linear growth stalled: {} restarts",
        search.restarts()
    );
    assert_eq!(search.child_evaluations(), 17_411_547);
}

#[test]
fn cutoff_fires_mid_run_and_solves() {
    // SHIPPED boundaries on m22 are at spent 0.5, 1.5, 3.5, 7.5 M. With a
    // 2 M cutoff the third boundary (spent 3.5 M) returns u64::MAX, so exactly
    // three boundaries are ever counted and the final unbounded slice solves.
    const FEN: &str = "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22";
    let mut pos = Position::from_fen(FEN).unwrap();
    let mut search = Search::new(256);
    search.set_timeout(60);
    search.set_restart_schedule(RestartSchedule::SHIPPED.with_cutoff(Some(2_000_000)));
    let (outcome, _pv, _nodes) = search.search_depth(&mut pos, u32::MAX);
    assert_eq!(outcome, Outcome::Win);
    assert_eq!(search.restarts(), 3);
    assert_eq!(search.child_evaluations(), 16_967_609);
}

#[test]
fn evaluate_child_resolves_solved_entry_without_move_hint() {
    // A solved TT entry with `Move::NONE`, `work = 0`, and
    // `remaining_depth = u32::MAX` must be resolved by `evaluate_child`
    // with its proven depth preserved (rather than feeding the
    // unsolved-bounds reuse branch). This is a TT reuse invariant.
    let mut search = Search::new(1);
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mv = Move::make_move(Square::G1, Square::H1);
    pos.do_move(mv);
    let child_key = pos.hash();
    pos.undo_move(mv);

    let (pn, dn) = Outcome::Loss.to_pn_dn();
    search.tt.store(
        child_key,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Loss),
        pn,
        dn,
        9,
        u32::MAX,
    );
    let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
    assert_eq!(info.outcome, Some(Outcome::Loss));
    assert_eq!(info.depth, 9, "proven depth preserved");
}

// ---------- cheap screening probe (performance plan1) ----------

/// An easy non-terminal 3-man root: the ordinary DF-PN search finds a win in
/// a few hundred child evals (189 measured); the pre-phase closure would cost
/// ~5.8M.
const EASY_KQ_FEN: &str = "4k3/8/8/8/8/8/8/4KQ2 w - - 0 1";
/// A 3-man draw: the probe burns its cap and defers to the pre-phase.
const KR_DRAW_FEN: &str = "4k3/8/8/8/8/8/8/4KR2 w - - 0 1";

#[test]
fn cheap_probe_gate_reports_decisive_on_easy_root() {
    let pos = Position::from_fen(EASY_KQ_FEN).unwrap();
    let mut search = Search::new(1);
    // The gate expects a started run (its deadlines/stop flags come from
    // `begin_run`); `solve` always calls it that way.
    search.begin_run();
    let gate = search.cheap_probe_gate(&pos);

    assert!(gate.ran, "the detector fires on a 3-man non-terminal root");
    assert!(gate.decided, "the easy KQ win is inside the probe cap");
    assert!(
        gate.evals > 0
            && gate.evals <= crate::search::dfpn::cheap_probe::CHEAP_PROBE_EVAL_CAP + 255,
        "probe evals {} stay within the cap plus one sweep overshoot",
        gate.evals
    );
}

#[test]
fn cheap_probe_gate_defers_on_draw_and_leaves_production_tt_untouched() {
    let pos = Position::from_fen(KR_DRAW_FEN).unwrap();
    let mut search = Search::new(1);
    search.begin_run();
    let before = search.tt_stats();
    let gate = search.cheap_probe_gate(&pos);

    assert!(gate.ran);
    assert!(!gate.decided, "a Draw return defers to the pre-phase");
    assert!(
        gate.evals > 0
            && gate.evals <= crate::search::dfpn::cheap_probe::CHEAP_PROBE_EVAL_CAP + 255,
        "the deferred probe is bounded by the cap plus one sweep overshoot, got {}",
        gate.evals
    );
    assert_eq!(
        search.tt_stats(),
        before,
        "the probe runs on its own scratch TT: the production table is untouched"
    );
    assert_eq!(search.child_evaluations(), 0, "the gate accounts nothing");
}

#[test]
fn cheap_probe_gate_is_skipped_when_conditions_do_not_hold() {
    // Terminal root: the pre-phase's region-1 claim needs no probe.
    let mated = Position::from_fen("7K/8/8/8/8/8/1Q6/k7 b - - 0 1").unwrap();
    let search = Search::new(1);
    let gate = search.cheap_probe_gate(&mated);
    assert!(!gate.ran, "terminal roots must not be probed");
    assert_eq!(gate.evals, 0);

    // Detector miss (4 men).
    let four_men = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    assert!(!search.cheap_probe_gate(&four_men).ran);

    // Pre-phase disabled (`--no-preflight` disables the gate too).
    let mut disabled = Search::new(1);
    disabled.set_preflight_enabled(false);
    let easy = Position::from_fen(EASY_KQ_FEN).unwrap();
    assert!(!disabled.cheap_probe_gate(&easy).ran);

    // No remaining child-eval budget (the gate is called before any work,
    // so a zero budget must not be spent on a probe).
    let mut spent = Search::new(1);
    spent.set_child_eval_budget(0);
    assert!(
        !spent.cheap_probe_gate(&easy).ran,
        "a zero budget cannot fund a probe"
    );
}

// --- Repetition-path bookkeeping (performance plan4) -----------------------

#[test]
fn incremental_context_hash_matches_fold() {
    // Push/pop a deterministic key sequence and compare the incrementally
    // maintained field against the reference fold after every operation.
    let mut search = Search::new(1);
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    for i in 0..64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        search.path_push(state);
        search.assert_path_bookkeeping_consistent();
        if i % 3 == 2 {
            search.path_pop();
            search.assert_path_bookkeeping_consistent();
        }
    }
    while !search.path_stack.is_empty() {
        search.path_pop();
        search.assert_path_bookkeeping_consistent();
    }
    assert_eq!(search.path_context_hash, 0);
}

#[test]
fn context_hash_replays_prefix_path() {
    let keys = [0xdead_beef, 0x1234_5678, 0x0f0f_0f0f, 42];
    let mut search = Search::new(1);
    search.prefix_path = Some(keys.to_vec());
    search.reset_search_state();

    assert_eq!(search.path_stack, keys);
    assert_eq!(
        search.path_context_hash,
        RepetitionCache::context_hash(&keys),
        "the reset seed must equal the reference fold of the prefix path"
    );
    for &key in &keys {
        assert!(search.path_contains(key), "prefix key must be on the path");
    }
    search.assert_path_bookkeeping_consistent();
}

#[test]
fn path_bloom_has_no_false_negatives() {
    let slots = super::PATH_BLOOM_SLOTS as u64;

    // Two keys in the same slot: collision means a fallback scan, never a
    // missed key.
    let mut search = Search::new(1);
    search.path_push(0);
    assert!(search.path_contains(0));
    assert!(
        !search.path_contains(slots),
        "an absent colliding key must still be reported absent"
    );
    search.path_push(slots);
    assert!(search.path_contains(slots), "a pushed key must be found");
    search.path_pop();
    assert!(
        !search.path_contains(slots),
        "a popped key must be absent even while its slot stays nonzero"
    );
    search.assert_path_bookkeeping_consistent();

    // A key whose slot is zero is decided without the linear scan and bumps
    // the test-only negative counter.
    let mut search = Search::new(1);
    search.path_push(0);
    let before = search.path_bloom_negatives.get();
    assert!(
        !search.path_contains(1),
        "an absent key in a zero Bloom slot must be absent"
    );
    assert_eq!(
        search.path_bloom_negatives.get(),
        before + 1,
        "the zero-slot negative must be counted"
    );
    assert!(
        search.path_bloom_negatives.get() > 0,
        "the Bloom-negative counter must fire on this fixture"
    );
}

#[test]
fn path_bloom_insert_remove_returns_to_zero() {
    let mut search = Search::new(1);
    let keys = [1_u64, 300, 70_000, 0xdead_beef, 0];
    for &key in &keys {
        search.path_push(key);
    }
    for _ in 0..keys.len() {
        search.path_pop();
    }
    assert!(search.path_stack.is_empty());
    assert_eq!(search.path_context_hash, 0);
    assert!(
        search.path_bloom.iter().all(|&count| count == 0),
        "inserting and removing every key must restore all Bloom slots"
    );
    search.assert_path_bookkeeping_consistent();
}

// ---------- static immediate-explosion closure (provari plan4) ----------

/// A one-move exploding capture for the side to move: Rd1xd8 explodes the
/// black commoner on e8 (d8 is beside it and not beside our commoner on e1).
const STATIC_WIN_FEN: &str = "3rk3/8/8/8/8/8/8/3RK3 w - - 0 1";

fn static_win_move() -> Move {
    Move::make_move(Square::D1, Square::D8)
}

fn dfpn_from(fen: &str, static_tactics: bool, max_depth: u32) -> (Search, Position, Outcome) {
    let mut search = Search::new(1);
    search.set_static_tactics(static_tactics);
    search.begin_run();
    let mut pos = Position::from_fen(fen).unwrap();
    let outcome = search.dfpn(&mut pos, INF, INF, max_depth, u64::MAX, true);
    (search, pos, outcome)
}

#[test]
fn static_tactics_claims_a_beside_commoner_capture_without_evals() {
    let (search, pos, outcome) = dfpn_from(STATIC_WIN_FEN, true, u32::MAX);

    assert_eq!(outcome, Outcome::Win);
    assert_eq!(search.nodes(), 1, "the claim must not expand the node");
    assert_eq!(
        search.child_evaluations(),
        0,
        "the static scan is internal work: no child evaluation is billed"
    );

    let entry = search.tt.probe(pos.hash()).copied().expect("stored entry");
    assert_eq!(entry.outcome, Some(Outcome::Win));
    assert_eq!(entry.depth, 1);
    assert_eq!(
        entry.best_move,
        static_win_move(),
        "hint is the winning move"
    );
    assert_eq!(entry.best_child, u8::MAX, "no child index for a free claim");
    assert_eq!(entry.remaining_depth, u32::MAX);
    assert_eq!(entry.work, 0, "the proof was free");
}

#[test]
fn static_tactics_emits_a_node_proven_win_at_depth_one() {
    let mut search = Search::new(1);
    search.set_static_tactics(true);
    search.begin_run();
    let (tx, rx) = std::sync::mpsc::channel();
    search.set_proof_event_sender(Some(tx));

    let mut pos = Position::from_fen(STATIC_WIN_FEN).unwrap();
    let key = pos.hash();
    assert_eq!(
        search.dfpn(&mut pos, INF, INF, u32::MAX, u64::MAX, true),
        Outcome::Win
    );
    drop(search);

    let events: Vec<crate::proof_event::ProofEvent> = rx.iter().collect();
    assert_eq!(events.len(), 1, "exactly one proven node");
    let crate::proof_event::ProofEvent::NodeProven(event) = &events[0];
    assert_eq!(event.path, Vec::<Move>::new(), "the root path is empty");
    assert_eq!(event.hash, key);
    assert_eq!(event.outcome, Outcome::Win);
    assert_eq!(event.depth, 1);
}

#[test]
fn static_tactics_flag_off_keeps_the_shipped_search() {
    let (search, pos, outcome) = dfpn_from(STATIC_WIN_FEN, false, u32::MAX);

    assert_eq!(outcome, Outcome::Win, "the ordinary search finds it too");
    assert!(
        search.child_evaluations() > 0,
        "the flag-off search bills the child evaluations the static claim spares"
    );
    // The ordinary search's own entry is the same value, but with a real
    // best-child hint or a deeper proof — never the free static claim.
    let entry = search.tt.probe(pos.hash()).copied().expect("stored entry");
    assert_eq!(entry.outcome, Some(Outcome::Win));
}

#[test]
fn static_tactics_is_inert_when_no_explosion_exists() {
    // A two-rook mate with no beside-commoner capture anywhere: the flag may
    // not change the outcome, the nodes, or the billed evaluations.
    const FEN: &str = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";
    let mut off_search = Search::new(1);
    off_search.set_timeout(10);
    let mut on_search = Search::new(1);
    on_search.set_timeout(10);
    on_search.set_static_tactics(true);
    let mut off_pos = Position::from_fen(FEN).unwrap();
    let mut on_pos = Position::from_fen(FEN).unwrap();

    let (off_outcome, _, off_nodes) = off_search.search_depth(&mut off_pos, u32::MAX);
    let (on_outcome, _, on_nodes) = on_search.search_depth(&mut on_pos, u32::MAX);

    assert_eq!(
        on_outcome, off_outcome,
        "the lever may not change an outcome"
    );
    assert_eq!(on_nodes, off_nodes, "no claim ⇒ identical trajectory");
    assert_eq!(
        on_search.child_evaluations(),
        off_search.child_evaluations()
    );
}

/// Full-depth cross-check on a deep draw that *does* contain beside-commoner
/// captures below the root: the closure may only reduce work, never change
/// the proven value.
#[test]
#[ignore = "slow: ~20 s; full-depth draw cross-check on the static closure"]
fn static_tactics_agrees_on_a_deep_draw() {
    const FEN: &str = "r3k3/8/8/8/8/8/8/R3K3 w - - 0 1";
    let mut results = Vec::new();
    for static_tactics in [false, true] {
        let mut search = Search::new(1);
        search.set_static_tactics(static_tactics);
        search.set_timeout(120);
        let mut pos = Position::from_fen(FEN).unwrap();
        let (outcome, _, _) = search.search_depth(&mut pos, u32::MAX);
        results.push((outcome, search.child_evaluations()));
    }
    assert_eq!(results[0].0, results[1].0, "outcomes must agree");
    assert!(
        results[1].1 <= results[0].1,
        "the closure may only reduce billed evaluations: {} vs {}",
        results[1].1,
        results[0].1
    );
}

#[test]
fn static_tactics_keeps_the_depth_horizon() {
    let (search, pos, outcome) = dfpn_from(STATIC_WIN_FEN, true, 0);

    assert_eq!(outcome, Outcome::Draw, "a depth-0 leaf stays a Draw");
    assert_eq!(search.nodes(), 1);
    let entry = search.tt.probe(pos.hash()).copied().expect("leaf entry");
    assert_eq!(entry.outcome, None, "the leaf stores unsolved bounds");
    assert_eq!((entry.pn, entry.dn), (1, 1));
    assert_eq!(entry.remaining_depth, 0);
}

#[test]
fn static_tactics_keeps_the_local_repetition_draw() {
    let mut search = Search::new(1);
    search.set_static_tactics(true);
    search.begin_run();
    let mut pos = Position::from_fen(STATIC_WIN_FEN).unwrap();
    search.path_push(pos.repetition_key());

    assert_eq!(
        search.dfpn(&mut pos, INF, INF, u32::MAX, u64::MAX, true),
        Outcome::Draw,
        "a repeated position stays a Draw even with a static win available"
    );
    assert_eq!(search.child_evaluations(), 0);
}

#[test]
fn static_tactics_yields_to_a_solved_tt_entry() {
    let mut search = Search::new(1);
    search.set_static_tactics(true);
    search.begin_run();
    let mut pos = Position::from_fen(STATIC_WIN_FEN).unwrap();
    // A solved non-Win entry for this exact root must be preferred: the
    // static claim only preempts *expansion*, never an existing proof.
    search.tt.store(
        pos.hash(),
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Draw),
        INF,
        0,
        3,
        u32::MAX,
    );
    assert_eq!(
        search.dfpn(&mut pos, INF, INF, u32::MAX, u64::MAX, true),
        Outcome::Draw
    );
    assert_eq!(search.child_evaluations(), 0);
}
