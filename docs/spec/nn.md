# Move-Ordering Network Specification

## 1. Purpose

Replace / augment `StaticAtomicScorer` with a learned `MoveScorer` that ranks
legal moves at a df-pn node by predicted resolution cost (proof/disproof
effort), rather than by hand-crafted heuristics.

## 2. Input features

- **Encoding:** sparse one-hot, `(piece type, color, square)`
- **Feature count:** `n = 6 (piece types) × 2 (colors) × 64 (squares) = 768`
  (the "12" seen elsewhere, e.g. in Stockfish docs, is just `6 × 2` collapsed
  into one axis — same 768 either way)
- **Active features per position:** ≈ number of pieces on the board (typically 2–32)
- **No king-relative features in v1** (candidate extension for v2 if benchmarks
  justify the added complexity)

Each feature is a binary indicator: 1 if that (piece, color, square)
combination is present on the board, 0 otherwise.

## 3. Layer-by-layer architecture

| Stage | Operation                          | Input dim    | Output dim    | Learned?                    | Notes                                            |
| ----- | ---------------------------------- | ------------ | ------------- | --------------------------- | ------------------------------------------------ |
| 1a    | Feature transformer (side-to-move) | 768 (sparse) | 128           | Yes (`W_1`, shared weights) | `a_stm = W_1 x_stm + b_1`                        |
| 1b    | Feature transformer (other side)   | 768 (sparse) | 128           | Yes (same `W_1`)            | `a_other = W_1 x_other + b_1`                    |
| 2     | Concatenate                        | 128 + 128    | 256           | No                          | `a = concat(a_stm, a_other)`, side-to-move first |
| 3     | Activation                         | 256          | 256           | No                          | ClippedReLU, clamp to `[0, max]`                 |
| 4     | Hidden (dense)                     | 256          | 32            | Yes (`W_2`, `b_2`)          | `h = ClippedReLU(W_2 a + b_2)`                   |
| 5     | Output (dense)                     | 32           | `policy_size` | Yes (`W_3`, `b_3`)          | `s = W_3 h + b_3`, raw per-move scores           |

Compact notation: **`768 → 128×2 → 32 → policy_size`**

### Weight matrix shapes

- `W_1`: `128 × 768` (shared across both perspectives)
- `W_2`: `32 × 256`
- `W_3`: `policy_size × 32`

## 4. Incremental update rule (search-time only)

Only stage 1 (feature transformer) supports incremental updates. On
make/unmake, given the set of features that turned on/off for a given
perspective:

```
a' = a + Σ W_1[:, i] for i in features_turned_on
       − Σ W_1[:, j] for j in features_turned_off
```

Both `a_stm` and `a_other` are maintained on a stack, pushed before a move is
made and popped on unmake — no recomputation needed on unmake.

Stages 2–5 are recomputed fully and densely after every move (no incremental
path exists past the feature transformer).

## 5. Output encoding and masking

- `policy_size`: move-index space. v1 recommendation: `64 × 64 = 4096`
  (from-square × to-square), with promotions handled as a fixed multiplier or
  special-cased subset. AlphaZero-style move-plane encodings (~1858) are a
  possible later optimization.
- At inference: compute `s ∈ ℝ^policy_size`, apply a legal-move mask (set
  illegal indices to `−∞` or drop them), then sort remaining entries
  descending. The mask is derived from the move list the solver already
  generates for expansion — no extra move-generation cost.
- No softmax needed at inference if only a ranking is used (softmax only
  relevant if training uses a probability-based loss instead of pairwise
  ranking).

## 6. Training target and loss

- **Label source:** solver's own `ProofEvent` / finalized proof tree.
  For each expanded internal node, record `(position, move, subtree_size)`
  for each child that received search effort.
- **Loss:** pairwise ranking loss (RankNet-style) over sibling pairs at a
  node, comparing the finalized/confidently-cheaper child against other
  siblings. Children with zero recorded work are excluded from the loss
  (censored, not "cheap").
- **No absolute regression target** — only relative order between siblings
  matters for move ordering.

## 7. Sizing rationale (v1)

| Parameter                   | Value          | Rationale                                                                          |
| --------------------------- | -------------- | ---------------------------------------------------------------------------------- |
| Accumulator width           | 128            | Matches Stockfish's "small net"; ranking needs less resolution than eval           |
| Hidden layer width          | 32             | Matches Stockfish's L2/L3 scale; small trunk keeps dense recompute cheap           |
| Depth past accumulator      | 1 hidden layer | Avoid overfitting given limited (10^5–10^6 position) training data                 |
| Quantization                | None in v1     | Validate architecture in float32 first; quantize only after benchmarking           |
| PSQT buckets / layer stacks | None in v1     | Stockfish-specific eval-precision trick; unnecessary complexity for a ranking task |

## 8. Open parameters to determine empirically

- `policy_size` exact scheme (plain 4096 vs. compact move-plane encoding)
- Whether king-relative or blast-relative features improve ranking quality
  enough to justify their complexity (v2 candidate)
- Whether accumulator width 128 is sufficient, vs. needing 256
- Benchmark target: nodes-to-solve and wall-clock time-to-solve on
  `benchmark --suite move-order`, compared against `StaticAtomicScorer`
