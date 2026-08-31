//! Forward pass of the move-ordering network (`nn.md` §3, §4).
//!
//! Layer stack: `768 → 128×2 → 32 → policy_size`, shared `W_1` across both
//! perspectives, ClippedReLU with **max = 1.0 hard-coded** (the clamp max is
//! part of the inference contract and is not recorded in the weight file;
//! changing it requires a weight-file version bump).
//!
//! Stage 1 is expressed per view as `a = b_1 + Σ_{f active} W_1[:, f]`, which
//! is both the fresh sparse computation and the primitive of the §4
//! incremental update rule (`a' = a + Σ on − Σ off`, maintained on a
//! make/unmake stack); only stage 1 supports incremental updates.

use super::features::FeatureSets;
use super::{ACCUMULATOR_DIM, HIDDEN_DIM, INPUT_DIM};
use crate::nn::weights::NnWeights;

/// ClippedReLU upper bound, pinned by `nn.md` §3 (not stored in the file).
pub const CLAMP_MAX: f32 = 1.0;

/// Clamp a dense vector to `[0, CLAMP_MAX]` (ClippedReLU, max = 1.0).
pub fn clamp01(v: &mut [f32]) {
    for x in v {
        *x = x.clamp(0.0, CLAMP_MAX);
    }
}

/// Stage 1 for one view: `a = b_1 + Σ_{f in features} W_1[:, f]`.
///
/// `features` holds active feature indices (`f = 64 * p + sq`, §2); this is
/// also the §4 incremental primitive — an update adds and subtracts the same
/// 128-float [`NnWeights::w1_column`] slices.
#[must_use]
pub fn accumulator(weights: &NnWeights, features: &[u16]) -> [f32; ACCUMULATOR_DIM] {
    debug_assert!(
        features.iter().all(|&f| (f as usize) < INPUT_DIM),
        "feature index out of range"
    );
    let mut a = *weights.b1();
    for &f in features {
        for (a_r, w) in a.iter_mut().zip(weights.w1_column(f as usize)) {
            *a_r += w;
        }
    }
    a
}

/// Stages 2–4: concatenate the two clamped accumulators (stm half first),
/// clamp to `[0, 1]`, then `h = ClippedReLU(W_2 a + b_2)`.
#[must_use]
pub fn hidden(
    weights: &NnWeights,
    a_stm: &[f32; ACCUMULATOR_DIM],
    a_other: &[f32; ACCUMULATOR_DIM],
) -> [f32; HIDDEN_DIM] {
    let mut concat = [0.0f32; 2 * ACCUMULATOR_DIM];
    concat[..ACCUMULATOR_DIM].copy_from_slice(a_stm);
    concat[ACCUMULATOR_DIM..].copy_from_slice(a_other);
    clamp01(&mut concat);

    let mut h = [0.0f32; HIDDEN_DIM];
    for (r, h_r) in h.iter_mut().enumerate() {
        let mut acc = weights.b2()[r];
        for (w, x) in weights.w2_row(r).iter().zip(concat) {
            acc += w * x;
        }
        *h_r = acc.clamp(0.0, CLAMP_MAX);
    }
    h
}

/// Convenience: stages 1–4 from the §2 feature sets (clamps the
/// accumulators as stage 3 requires, then evaluates the hidden layer).
#[must_use]
pub fn hidden_for(weights: &NnWeights, features: &FeatureSets) -> [f32; HIDDEN_DIM] {
    let a_stm = accumulator(weights, features.stm_features());
    let a_other = accumulator(weights, features.other_features());
    hidden(weights, &a_stm, &a_other)
}

/// Stage 5 for one policy index: `s[idx] = W_3[idx, :] · h + b_3[idx]`.
///
/// Callers evaluate only the (deduplicated) legal-move indices instead of
/// materializing all 4096 outputs — identical semantics to computing `s`
/// densely and masking.
#[must_use]
pub fn policy_score(weights: &NnWeights, h: &[f32; HIDDEN_DIM], policy_index: usize) -> f32 {
    let mut s = weights.b3_at(policy_index);
    for (w, x) in weights.w3_row(policy_index).iter().zip(h) {
        s += w * x;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nn::weights::NnWeights;
    use crate::nn::{POLICY_SIZE, features::policy_index};
    use crate::position::Position;

    fn fixture() -> NnWeights {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/nn_trainer_ref/fixtures/weights.v1.bin");
        NnWeights::from_path(path).expect("fixture weight file must load")
    }

    fn lone_king_features() -> FeatureSets {
        let pos = Position::from_fen("8/8/8/8/8/8/8/K7 w - - 0 1").unwrap();
        crate::nn::features::feature_sets(pos.board())
    }

    /// Hand-computed fixture forward pass for the lone white commoner on a1.
    ///
    /// a_stm = b_1 (col 320 is zero) = [0.5, 0, …, −0.25]; same for a_other.
    /// After the clamp the −0.25 tail is 0. W_2 sees concat[0] = concat[128]
    /// = 0.5, so h[0] = 0.5·0.5 + 0.25 = 0.5 and every other h entry is 0
    /// (h[31] = −0.125·(−0.25) − 0.5 < 0 clamps to 0).
    #[test]
    fn fixture_forward_lone_king_matches_hand_computation() {
        let w = fixture();
        let features = lone_king_features();
        let h = hidden_for(&w, &features);
        assert_eq!(h[0], 0.5);
        assert!(h[1..].iter().all(|&x| x == 0.0), "h = {h:?}");

        // s[idx] = 0.5 * W_3[idx][0] + b_3[idx] with the fixture values.
        assert_eq!(policy_score(&w, &h, 0), 0.5 * 2.0 + 0.125); // 1.125
        assert_eq!(policy_score(&w, &h, 1), 0.5 * 0.5); // W_3[1][0] = 0.5
        assert_eq!(policy_score(&w, &h, 4095), -0.125);
        assert_eq!(policy_score(&w, &h, 4094), 0.0);
    }

    /// The accumulator must equal the dense `W_1 x + b_1` reference.
    #[test]
    fn accumulator_matches_dense_reference() {
        let w = fixture();
        let pos = Position::from_fen(Position::STARTPOS_FEN).unwrap();
        let sets = crate::nn::features::feature_sets(pos.board());
        for (label, features) in [
            ("stm", sets.stm_features()),
            ("other", sets.other_features()),
        ] {
            let a = accumulator(&w, features);
            for (r, a_r) in a.iter().enumerate() {
                let mut expect = w.b1()[r];
                for &f in features {
                    expect += w.w1_at(r, f as usize);
                }
                assert_eq!(*a_r, expect, "row {r} of the {label} accumulator");
            }
        }
    }

    /// Build an in-memory §10 file with the given W_2 rows 0/1 and b_1[0].
    fn synthetic_weights(w2_00: f32, w2_1_c128: f32, b1_0: f32) -> NnWeights {
        let mut bytes = Vec::with_capacity(crate::nn::weights::TOTAL_SIZE);
        bytes.extend_from_slice(&crate::nn::weights::MAGIC.to_le_bytes());
        for v in [
            crate::nn::weights::VERSION,
            INPUT_DIM as u16,
            ACCUMULATOR_DIM as u16,
            HIDDEN_DIM as u16,
            POLICY_SIZE as u16,
            0,
        ] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let put = |bytes: &mut Vec<u8>, idx: usize, x: f32| {
            let at = crate::nn::weights::HEADER_SIZE + 4 * idx;
            if bytes.len() < at + 4 {
                bytes.resize(at + 4, 0);
            }
            bytes[at..at + 4].copy_from_slice(&x.to_le_bytes());
        };
        // W_1 row-major [128][768]: all zero.
        put(&mut bytes, 0, 0.0);
        // b_1 lives after the 128*768 W_1 floats.
        put(&mut bytes, 128 * INPUT_DIM, b1_0);
        // W_2 after W_1 + b_1.
        let w2_base = 128 * INPUT_DIM + ACCUMULATOR_DIM;
        put(&mut bytes, w2_base, w2_00);
        put(
            &mut bytes,
            w2_base + 2 * ACCUMULATOR_DIM + ACCUMULATOR_DIM,
            w2_1_c128,
        );
        // W_3 / b_3 after W_2 + b_2.
        let w3_base = w2_base + HIDDEN_DIM * 2 * ACCUMULATOR_DIM + HIDDEN_DIM;
        put(&mut bytes, w3_base + 5 * HIDDEN_DIM, 2.0);
        put(&mut bytes, w3_base + 6 * HIDDEN_DIM + 1, -2.0);
        let b3_base = w3_base + POLICY_SIZE * HIDDEN_DIM;
        put(&mut bytes, b3_base + 5, 0.25);
        put(&mut bytes, b3_base + 6, 0.5);
        bytes.resize(crate::nn::weights::TOTAL_SIZE, 0);
        NnWeights::from_bytes(&bytes).expect("synthetic file must parse")
    }

    #[test]
    fn clipped_relu_clamps_both_ends() {
        // b_1[0] = 2.0 clamps a_stm[0] and a_other[0] to the hard max 1.0;
        // W_2 rows 0/1 then push h[0] to +3.0 and h[1] to −3.0, clamping to
        // 1.0 and 0.0 respectively.
        let w = synthetic_weights(3.0, -3.0, 2.0);
        let pos = Position::from_fen("8/8/8/8/8/8/8/K7 w - - 0 1").unwrap();
        let sets = crate::nn::features::feature_sets(pos.board());
        let h = hidden_for(&w, &sets);
        assert_eq!(h[0], CLAMP_MAX);
        assert_eq!(h[1], 0.0);
        // s[5] = 2.0·h[0] + 0.25 = 2.25; s[6] = −2.0·h[1] + 0.5 = 0.5.
        assert_eq!(policy_score(&w, &h, 5), 2.25);
        assert_eq!(policy_score(&w, &h, 6), 0.5);
    }

    #[test]
    fn promotion_variants_share_one_policy_row() {
        let w = fixture();
        let pos = Position::from_fen("4k3/3P4/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let moves = pos.legal_moves_vec();
        let promos: Vec<_> = moves.iter().copied().filter(|m| m.is_promotion()).collect();
        assert!(!promos.is_empty(), "test position must offer promotions");
        let h = hidden_for(&w, &crate::nn::features::feature_sets(pos.board()));
        // Group the promotion variants by their (from, to) index: every
        // variant of one (from, to) must share one policy row and score.
        let mut groups: Vec<(usize, f32)> = Vec::new();
        for m in promos {
            let idx = policy_index(m);
            let s = policy_score(&w, &h, idx);
            match groups.iter_mut().find(|(g, _)| *g == idx) {
                Some((_, gs)) => assert_eq!(*gs, s, "variant of index {idx} shares one score"),
                None => groups.push((idx, s)),
            }
        }
        assert!(
            groups.len() >= 2,
            "the test position has two promotion targets"
        );
    }
}
