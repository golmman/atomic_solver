//! Neural move-ordering scorer: the Gate-3 integration point.
//!
//! `NnMoveScorer` ranks the legal moves of a position by the network's
//! `s[policy_index]` (§5, `policy_index = from_sq * 64 + to_sq`). Promotion
//! variants of one `(from, to)` share one policy index, so the indices are
//! deduplicated before the (expensive) stage-5 rows are evaluated and every
//! variant reads the same score.
//!
//! Scores are RankNet margins: only the relative order is meaningful. The
//! monotone round-after-scale mapping to `i32` can merge close scores but
//! never inverts them, and the scores are only ever sorted, never
//! thresholded. Composition follows `concept.md` §6: the network replaces
//! the static term; history, killer, and best-from-TT ordering stay additive.

use std::sync::Arc;

use atomic_movegen::board::Board;
use atomic_movegen::types::Move;

use super::eval;
use super::features::{feature_sets, policy_index};
use super::weights::NnWeights;

/// Default mapping scale from f32 RankNet margins to the `i32` ordering
/// scale. The mapping is monotone, so any positive scale preserves the
/// network's ranking; the value only sets the trade-off against the additive
/// history (≤ 10,000) and killer (50,000) bonuses. Tunable via
/// [`NnMoveScorer::with_scale`] for Gate 4.
pub const NN_SCORE_SCALE: f32 = 4096.0;

/// Ranks legal moves with the move-ordering network.
#[derive(Clone)]
pub struct NnMoveScorer {
    weights: Arc<NnWeights>,
    scale: f32,
}

impl NnMoveScorer {
    /// Create a scorer with the default [`NN_SCORE_SCALE`].
    #[must_use]
    pub fn new(weights: Arc<NnWeights>) -> Self {
        Self {
            weights,
            scale: NN_SCORE_SCALE,
        }
    }

    /// Create a scorer with an explicit margin-to-`i32` scale.
    #[must_use]
    pub fn with_scale(weights: Arc<NnWeights>, scale: f32) -> Self {
        Self { weights, scale }
    }

    /// Borrow the loaded weights.
    #[must_use]
    pub fn weights(&self) -> &NnWeights {
        &self.weights
    }

    /// One `i32` ordering score per move, in the same order as `moves`.
    ///
    /// Stages 1–4 run once per position; stage 5 is evaluated once per
    /// unique policy index (promotion variants deduplicate).
    #[must_use]
    pub fn move_scores(&self, board: &Board, moves: &[Move]) -> Vec<i32> {
        let features = feature_sets(board);
        let a_stm = eval::accumulator(&self.weights, features.stm_features());
        let a_other = eval::accumulator(&self.weights, features.other_features());
        let h = eval::hidden(&self.weights, &a_stm, &a_other);

        let mut indices: Vec<usize> = moves.iter().map(|&m| policy_index(m)).collect();
        indices.sort_unstable();
        indices.dedup();
        let unique_scores: Vec<f32> = indices
            .iter()
            .map(|&idx| eval::policy_score(&self.weights, &h, idx))
            .collect();

        moves
            .iter()
            .map(|&m| {
                let idx = policy_index(m);
                let found = indices
                    .binary_search(&idx)
                    .expect("policy index was collected from the same move list");
                margin_to_order_score(unique_scores[found], self.scale)
            })
            .collect()
    }
}

/// Map a RankNet margin onto the `i32` ordering scale.
///
/// Rounding after scaling is monotone: equal or close margins may merge but
/// never invert, so only the relative order (the only meaningful property of
/// a margin) is preserved.
#[must_use]
pub fn margin_to_order_score(s: f32, scale: f32) -> i32 {
    (s * scale).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;

    fn fixture_scorer() -> NnMoveScorer {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/nn_trainer_ref/fixtures/weights.v1.bin");
        let weights = NnWeights::from_path(path).expect("fixture weight file must load");
        NnMoveScorer::new(Arc::new(weights))
    }

    fn promo_position() -> Position {
        Position::from_fen("4k3/3P4/8/8/8/8/8/4K3 w - - 0 1").unwrap()
    }

    #[test]
    fn scores_align_with_move_list_and_are_deterministic() {
        let scorer = fixture_scorer();
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        let moves = pos.legal_moves_vec();
        assert!(!moves.is_empty());
        let scores = scorer.move_scores(pos.board(), &moves);
        assert_eq!(scores.len(), moves.len());
        let again = scorer.move_scores(pos.board(), &moves);
        assert_eq!(scores, again);
    }

    #[test]
    fn promotion_variants_share_one_score() {
        let scorer = fixture_scorer();
        let pos = promo_position();
        let moves = pos.legal_moves_vec();
        let promos: Vec<_> = moves.iter().copied().filter(|m| m.is_promotion()).collect();
        assert!(!promos.is_empty());
        let scores = scorer.move_scores(pos.board(), &moves);
        let first = promos[0];
        for m in &promos {
            assert_eq!(
                scores[moves.iter().position(|&x| x == *m).unwrap()],
                scores[moves.iter().position(|&x| x == first).unwrap()],
                "all promotion variants of one (from, to) share one score"
            );
        }
    }

    #[test]
    fn margin_mapping_is_monotone() {
        let scale = NN_SCORE_SCALE;
        let mut prev = margin_to_order_score(-100.0, scale);
        for step in [-100.0, -1.5, -0.0001, 0.0, 0.0001, 1.5, 100.0] {
            let cur = margin_to_order_score(step, scale);
            assert!(cur >= prev, "{step} inverts the order");
            prev = cur;
        }
        assert_eq!(margin_to_order_score(0.0, scale), 0);
    }

    #[test]
    fn scores_feed_a_correct_search() {
        // With the (near-zero) fixture weights the ordering is weak, but the
        // search must still solve the tactical position correctly.
        let scorer = fixture_scorer();
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        let mut search = crate::search::dfpn::Search::new(8);
        search.set_nn_scorer(Some(scorer));
        search.set_first_outcome_only(true);
        let (outcome, _pv, _) = search.solve(&mut pos);
        assert_eq!(outcome, crate::position::Outcome::Win);
        assert!(search.exit_reason() != crate::search::dfpn::ExitReason::Timeout);
    }
}
