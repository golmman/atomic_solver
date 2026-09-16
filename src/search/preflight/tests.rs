//! Unit tests for the detector-gated bounded pre-phase (plan13).
//!
//! Fast-tier by construction: claim tests use positions whose closure is
//! tiny (terminal roots) or bounded-abort paths. Full-component closures
//! (the KQvK ladder, the adjacency-immunity draw) are covered by the slow
//! integration tests in `tests/test_preflight.rs`, except one full-closure
//! draw claim kept in the fast tier as end-to-end coverage of the D2 path.
//!
//! Engine-semantics note (discovered while calibrating these tests, and
//! load-bearing for the class): commoners are pseudo-royal with
//! *adjacency immunity* — a lone commoner adjacent to the enemy commoner is
//! never "in check" (`atomic_movegen::board::Board::legal`). A KQvK position
//! with the kings adjacent (`8/8/8/8/8/1K6/1Q6/k7 b`) is a genuine Draw
//! (confirmed against `egtb3-q.bin`), which the pre-phase proves by full
//! closure.

use atomic_movegen::board::Board;

use super::*;
use crate::position::Position;
use crate::search::dfpn::{ExitReason, PvStatus, Search};

/// Checkmate at the root in the pseudo-royal sense: the black king is
/// adjacent to the white queen (unprotected), the white king is far away, so
/// there is no adjacency immunity and no legal reply. Terminal Loss,
/// region 1.
const MATED_ROOT_FEN: &str = "7K/8/8/8/8/8/1Q6/k7 b - - 0 1";

#[test]
fn preflight_claims_terminal_checkmate_root() {
    let mut pos = Position::from_fen(MATED_ROOT_FEN).unwrap();
    let mut search = Search::new(1);
    let (outcome, pv, _nodes) = search.solve(&mut pos);

    assert_eq!(outcome, Outcome::Loss, "the terminal mate must be claimed");
    assert_eq!(search.pv_status(), PvStatus::PreflightProof);
    assert!(pv.is_empty(), "a rank-0 claim has no line");

    let report = search.preflight_report().expect("hook ran");
    assert!(report.decided);
    assert_eq!(report.reason, "");
    assert_eq!(report.outcome, Some(Outcome::Loss));
    assert_eq!(report.rank, 0);
    assert_eq!(report.region, 1, "terminal root closes immediately");
    assert_eq!(
        search.child_evaluations(),
        report.evals,
        "pre-phase evals are the run's evals when the claim returns directly"
    );

    // No TT interaction of any kind (plan13 hard constraint).
    let (_buckets, live, solved, _unsolved, _gen) = search.tt_stats();
    assert_eq!(live, 0, "a pre-phase claim must store nothing in the TT");
    assert_eq!(solved, 0);

    // The (empty) PV must still validate to the claimed terminal.
    assert!(search.validate_pv(&pv, &pos, Outcome::Loss, Some(0)));
}

#[test]
fn preflight_draw_claim_on_terminal_stalemate_root() {
    let mut pos = Position::from_fen("7k/8/8/8/8/8/2q5/K7 w - - 0 1").unwrap();
    let mut search = Search::new(1);
    let (outcome, pv, _nodes) = search.solve(&mut pos);

    assert_eq!(outcome, Outcome::Draw);
    assert!(pv.is_empty(), "draw claims carry no certificate line (D2)");
    // Draws are not decisive: the PV status stays `None` and the decision is
    // reported via the pre-flight report.
    assert_eq!(search.pv_status(), PvStatus::None);
    let report = search.preflight_report().unwrap();
    assert!(report.decided);
    assert_eq!(report.outcome, Some(Outcome::Draw));
    assert_eq!(report.rank, 0);
    assert_eq!(report.region, 1);
}

#[test]
fn preflight_win_claim_on_extinct_opponent_root() {
    // Only a black commoner: white has already won (extinction at the root).
    let mut pos = Position::from_fen("8/8/8/8/8/8/8/4k3 b - - 0 1").unwrap();
    let mut search = Search::new(1);
    let (outcome, pv, _nodes) = search.solve(&mut pos);

    assert_eq!(outcome, Outcome::Win);
    assert_eq!(search.pv_status(), PvStatus::PreflightProof);
    assert!(pv.is_empty(), "rank-0 claim has no line");
    let report = search.preflight_report().unwrap();
    assert!(report.decided);
    assert_eq!(report.outcome, Some(Outcome::Win));
    assert_eq!(report.rank, 0);
    assert_eq!(report.region, 1);
}

#[test]
fn preflight_disabled_flag_runs_ordinary_search() {
    let mut pos = Position::from_fen(MATED_ROOT_FEN).unwrap();
    let mut search = Search::new(1);
    search.set_preflight_enabled(false);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    assert_eq!(outcome, Outcome::Loss, "the search finds the same outcome");
    let report = search.preflight_report().unwrap();
    assert!(!report.decided);
    assert_eq!(report.reason, "disabled");
    assert_eq!(report.evals, 0);
    assert_eq!(report.region, 0);
    assert_eq!(search.pv_status(), PvStatus::ProvenShortest);
}

#[test]
fn preflight_defers_on_detector_miss_and_runs_search() {
    // 4 men: the two-rook smoke position must never be gated. (Note: the
    // report7 cyclic-rook position itself is 3 men and DOES gate — it is
    // covered by the test_repetition gates, where the pre-phase claims a
    // Draw, never a Win.)
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    let mut search = Search::new(1);
    search.set_child_eval_budget(5_000);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    let report = search.preflight_report().unwrap();
    assert!(!report.decided);
    assert_eq!(report.reason, "detector", "4 men must not gate");
    assert_eq!(report.evals, 0);
    assert_eq!(report.region, 0);

    // The ordinary search ran (and solved the easy mate within the budget):
    // deferral leaves the search fully in charge.
    assert_eq!(outcome, Outcome::Win);
    assert!(search.child_evaluations() > 0);
    assert_ne!(search.exit_reason(), ExitReason::BudgetExhausted);
    assert_eq!(search.pv_status(), PvStatus::ProvenShortest);
}

#[test]
fn preflight_eval_budget_defers_then_budget_contract_holds() {
    // A 3-men non-terminal position with a tiny budget: the pre-phase
    // exhausts the budget in its closure and defers; the ordinary search
    // then immediately reports `BudgetExhausted` (R4 contract verbatim).
    let mut pos = Position::from_fen("8/2K5/k7/8/8/8/8/4Q3 w - - 0 1").unwrap();
    let mut search = Search::new(1);
    search.set_child_eval_budget(1_000);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    let report = search.preflight_report().unwrap();
    assert!(!report.decided, "a budget abort must defer, never claim");
    assert_eq!(report.reason, "eval-budget");
    assert_eq!(search.child_evaluations(), report.evals);
    assert_eq!(outcome, Outcome::Draw);
    assert_eq!(search.exit_reason(), ExitReason::BudgetExhausted);
}

#[test]
fn rule50_boundary_semantics_of_the_engine() {
    // The D3 guard's premise, verified against the solver's own terminal
    // detector: rule50 expiry is a Draw at clock >= 100 for non-terminal
    // positions (never a decisive outcome), while mate/extinction outrank
    // rule50. The guard therefore only has to keep non-terminal certificate
    // positions below clock 100, and `halfmove + rank <= 99` does.
    let quiet = Position::from_fen("8/2K5/k7/8/8/8/8/4Q3 w - - 100 1").unwrap();
    assert_eq!(
        quiet.outcome(),
        Some(Outcome::Draw),
        "clock 100 on a non-terminal position is a draw"
    );
    let mated = Position::from_fen("7K/8/8/8/8/8/1Q6/k7 b - - 100 1").unwrap();
    assert_eq!(
        mated.outcome(),
        Some(Outcome::Loss),
        "checkmate outranks the rule50 draw"
    );
    // Extinction also outranks rule50 (the terminal branches precede it).
    let extinct = Position::from_fen("8/8/8/8/8/8/8/4K3 w - - 100 1").unwrap();
    assert_eq!(extinct.outcome(), Some(Outcome::Win));
}

#[test]
fn region_analyze_aborts_on_region_budget() {
    // The region-budget abort is the second-level gate (D4): abort, never
    // claim. Exercised directly on the fixpoint with a tiny budget.
    let board = Board::from_fen("8/2K5/k7/8/8/8/8/4Q3 w - - 0 1").unwrap();
    let abort = region::analyze(&board, u64::MAX, None, 10).expect_err("must abort");
    assert_eq!(abort.reason, "region-budget");
    assert!(abort.region > 10);
    assert!(abort.evals > 0);
}

#[test]
fn region_analyze_aborts_on_eval_budget() {
    let board = Board::from_fen("8/2K5/k7/8/8/8/8/4Q3 w - - 0 1").unwrap();
    let abort = region::analyze(&board, 500, None, REGION_BUDGET).expect_err("must abort");
    assert_eq!(abort.reason, "eval-budget");
}

#[test]
fn region_analyze_proves_a_full_closure_draw() {
    // End-to-end D2 coverage on a non-terminal root: with adjacency
    // immunity the defender holds (egtb3-q.bin says Draw), the fixpoint
    // remainder is a board-only Draw, and the pre-phase claims it. This is
    // the one full-component closure in the fast tier (~0.8 s release).
    let mut pos = Position::from_fen("8/8/8/8/8/1K6/1Q6/k7 b - - 0 1").unwrap();
    let mut search = Search::new(1);
    let (outcome, pv, _nodes) = search.solve(&mut pos);

    assert_eq!(outcome, Outcome::Draw, "adjacency-immunity draw");
    assert!(pv.is_empty());
    assert_eq!(search.pv_status(), PvStatus::None);
    let report = search.preflight_report().unwrap();
    assert!(report.decided);
    assert_eq!(report.outcome, Some(Outcome::Draw));
    assert!(report.region > 100_000, "the full component must close");
}

#[test]
fn preflight_claim_emits_no_proof_events_r2_gap() {
    // Documented round-1 gap (R2): the proof pipeline is TT-driven offline
    // and cannot reproduce a pre-phase proof, so the pre-phase emits no
    // `ProofEvent`s. A live worker wired to a claiming solve therefore stays
    // empty — the claim does not pretend to be a DF-PN proof.
    use crate::proof_event::ProofEvent;
    use std::sync::mpsc;

    let mut pos = Position::from_fen(MATED_ROOT_FEN).unwrap();
    let (sender, receiver) = mpsc::channel::<ProofEvent>();
    let mut search = Search::new(1);
    search.set_proof_event_sender(Some(sender));
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Loss, "pre-phase claimed");
    assert!(
        receiver.try_recv().is_err(),
        "the pre-phase must not synthesize proof events (R2)"
    );
    assert_eq!(search.pv_status(), PvStatus::PreflightProof);
}

// ---------- packed region keys and board-only classification ----------

mod key_tests {
    use super::*;
    use atomic_movegen::board::Board;

    #[test]
    fn detector_fires_on_three_men_without_pawns_or_castling() {
        let board = Board::from_fen("8/2K5/k7/8/8/8/8/4Q3 w - - 0 1").unwrap();
        assert!(detector_applies(&board));
        let board = Board::from_fen("8/8/8/8/8/8/8/K6k w - - 0 1").unwrap();
        assert!(detector_applies(&board), "two men");
        let board = Board::from_fen("8/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert!(detector_applies(&board), "one man");
    }

    #[test]
    fn detector_skips_pawns_and_castling_rights() {
        let board = Board::from_fen("4k3/PP6/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert!(!detector_applies(&board), "pawns");
        let board =
            Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        assert!(!detector_applies(&board), "castling rights");
        let board = Board::from_fen("8/2K5/k7/8/8/8/8/4Q3 w - - 0 21").unwrap();
        assert!(
            detector_applies(&board),
            "the halfmove clock is not part of the predicate"
        );
    }

    #[test]
    fn region_key_is_exact_and_order_independent() {
        // The key packs pieces in ascending square order, so the same
        // position always yields the same key regardless of construction.
        let a = Board::from_fen("8/2K5/k7/8/8/8/8/4Q3 w - - 7 42").unwrap();
        let b = Board::from_fen("8/2K5/k7/8/8/8/8/4Q3 b - - 99 1").unwrap();
        assert_ne!(
            region_key(&a),
            region_key(&b),
            "side to move is part of the key"
        );
        assert_eq!(region_key(&a), region_key(&a), "self-consistency");

        // Piece identity matters.
        let q = Board::from_fen("8/2K5/k7/8/8/8/8/4Q3 w - - 0 1").unwrap();
        let r = Board::from_fen("8/2K5/k7/8/8/8/8/4R3 w - - 0 1").unwrap();
        assert_ne!(region_key(&q), region_key(&r));

        // Square swaps matter.
        let k1 = Board::from_fen("8/8/8/3K4/3k4/8/8/8 w - - 0 1").unwrap();
        let k2 = Board::from_fen("8/8/8/3k4/3K4/8/8/8 w - - 0 1").unwrap();
        assert_ne!(region_key(&k1), region_key(&k2));
    }

    #[test]
    fn key_round_trips_through_board_reconstruction() {
        for fen in [
            "8/2K5/k7/8/8/8/8/4Q3 w - - 0 1",
            "8/8/8/3K4/3k4/8/8/8 b - - 0 1",
            "7K/8/8/8/8/8/1Q6/k7 b - - 0 1",
            "8/8/8/8/8/8/8/K6k w - - 0 1",
            "8/2K5/k7/8/8/8/8/4R3 b - - 0 1",
            "8/2K5/k7/8/8/8/8/4B3 b - - 0 1",
            "8/2K5/k7/8/8/8/8/4N3 b - - 0 1",
        ] {
            let board = Board::from_fen(fen).unwrap();
            let key = region_key(&board).expect("three men or fewer");
            let rebuilt = board_from_key(key);
            assert_eq!(
                Some(key),
                region_key(&rebuilt),
                "round-trip stability for {fen}"
            );
            assert_eq!(board.side_to_move(), rebuilt.side_to_move());
            for r in (0..8).rev() {
                for f in 0..8 {
                    let idx = r * 8 + f;
                    assert_eq!(
                        board.piece_on(atomic_movegen::types::Square::from_u8(idx)),
                        rebuilt.piece_on(atomic_movegen::types::Square::from_u8(idx)),
                        "piece mismatch at square {idx} for {fen}"
                    );
                }
            }
        }
    }

    #[test]
    fn classify_terminal_matches_position_outcome_at_clock_zero() {
        use crate::position::Position;
        for fen in [
            "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1",
            "7K/8/8/8/8/8/1Q6/k7 b - - 0 1",
            "7k/8/8/8/8/8/2q5/K7 w - - 0 1",
            "8/8/8/8/8/8/8/K6k w - - 0 1",
            "8/2K5/k7/8/8/8/8/4Q3 w - - 0 1",
            "8/8/8/8/8/8/8/k7 w - - 0 1",
            "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
        ] {
            let board = Board::from_fen(fen).unwrap();
            let position = Position::from_fen(fen).unwrap();
            assert_eq!(
                classify_terminal(&board),
                position.outcome(),
                "classification parity for {fen}"
            );
        }
    }
}
