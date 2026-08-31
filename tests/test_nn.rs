//! Gate-3 integration tests for the move-ordering network
//! (`docs/spec/nn.md`, `docs/plans/nn/plan5.md`).
//!
//! The byte-frozen trainer fixture drives the loader checks; the search
//! checks verify that enabling `NnMoveScorer` keeps the solver correct.

mod common;

use atomic_solver::nn::{NnMoveScorer, NnWeights};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use std::sync::Arc;

const FIXTURE: &str = "docs/nn_trainer_ref/fixtures/weights.v1.bin";

fn fixture_scorer() -> NnMoveScorer {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let weights = NnWeights::from_path(&path).expect("fixture weight file must load");
    NnMoveScorer::new(Arc::new(weights))
}

#[test]
fn fixture_loads_with_expected_header() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let bytes = std::fs::read(&path).expect("fixture must be readable");
    assert_eq!(bytes.len(), 967_312);
    assert_eq!(&bytes[..4], b"ATNN");
    let weights = NnWeights::from_bytes(&bytes).expect("fixture must parse");
    // Corner entries from docs/nn_trainer_ref/test_weights.py.
    assert_eq!(weights.w1_at(0, 0), 1.0);
    assert_eq!(weights.w1_at(0, 1), -0.5);
    assert_eq!(weights.w1_at(127, 767), 0.25);
    assert_eq!(weights.b1()[0], 0.5);
    assert_eq!(weights.w3_row(0)[0], 2.0);
    assert_eq!(weights.b3_at(4095), -0.125);
}

#[test]
fn search_with_nn_scorer_solves_tactical_position() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let mut search = Search::new(8);
    search.set_nn_scorer(Some(fixture_scorer()));
    search.set_first_outcome_only(true);
    let (outcome, _pv, _) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
}

#[test]
fn search_without_nn_scorer_keeps_hand_crafted_ordering() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let mut search = Search::new(8);
    assert!(search.nn_scorer().is_none());
    search.set_first_outcome_only(true);
    let (outcome, _pv, _) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
}

#[test]
fn nn_scorer_deduplicates_promotion_indices() {
    let scorer = fixture_scorer();
    let pos = Position::from_fen("4k3/3P4/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let moves = pos.legal_moves_vec();
    let promos: Vec<_> = moves.iter().copied().filter(|m| m.is_promotion()).collect();
    assert!(!promos.is_empty(), "promotion test position must promote");
    let scores = scorer.move_scores(pos.board(), &moves);
    let base_idx = moves
        .iter()
        .position(|&m| m == promos[0])
        .expect("first promotion is in the move list");
    let base = scores[base_idx];
    for m in &promos {
        let idx = moves.iter().position(|&x| x == *m).unwrap();
        assert_eq!(scores[idx], base, "promotion variants share one score");
    }
}
