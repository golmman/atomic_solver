//! Move-ordering network inference (Gate 3).
//!
//! Consumer side of `docs/spec/nn.md`: the §10 weight-file loader, the §2
//! two-perspective feature extractor, the §3/§4 forward pass with the
//! hard-coded ClippedReLU (max = 1.0), and the [`scorer::NnMoveScorer`]
//! integration point that ranks legal moves behind
//! `Search::set_nn_scorer` / the CLI `--nn-weights` flag.
//!
//! The normative contract is `docs/spec/nn.md`; `docs/nn_trainer_ref/`
//! holds the trainer's reference implementation and conformance vectors.
//! Scores are RankNet margins — only their relative order is meaningful,
//! so they are sorted, never thresholded.

pub mod eval;
pub mod features;
pub mod scorer;
pub mod weights;

pub use features::{ACCUMULATOR_DIM, HIDDEN_DIM, INPUT_DIM, POLICY_SIZE};
pub use scorer::{NN_SCORE_SCALE, NnMoveScorer};
pub use weights::{NnWeights, WeightsError};
