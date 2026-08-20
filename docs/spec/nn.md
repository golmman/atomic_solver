# Move-Ordering Network Specification

## 1. Purpose

Replace or augment hand-crafted move-ordering heuristics with a learned move
prioritizer that ranks the legal moves of a position by predicted resolution
cost (the proof/disproof effort a search would need to spend on the resulting
child position), rather than by hand-crafted heuristics.

The model is designed for a proof-number-style proof search engine: the
engine expands tree nodes by iterating over legal moves in ranking order, so
a better ranking means the engine resolves a node after visiting fewer
children. The metric to optimize is therefore nodes-visited (and wall-clock
time) per resolved root position, not single-move prediction accuracy.

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

### Split by perspective

Every position is encoded twice, relative to the side to move: once as seen
by the side to move and once as seen by the other side (colors swapped,
king/files mirrored consistently between the two views). Both views share a
single linear projection (see §3), so a feature means the same thing in
either perspective.

## 3. Layer-by-layer architecture

| Stage | Operation                          | Input dim    | Output dim    | Learned?                    | Notes                                            |
| ----- | ---------------------------------- | ------------ | ------------- | --------------------------- | ------------------------------------------------ |
| 1a    | Feature transformer (side-to-move) | 768 (sparse) | 128           | Yes (`W_1`, shared weights) | `a_stm = W_1 x_stm + b_1`     |
| 1b    | Feature transformer (other side)   | 768 (sparse) | 128           | Yes (same `W_1`)            | `a_other = W_1 x_other + b_1` |
| 2     | Concatenate                        | 128 + 128    | 256           | No                          | `a = concat(a_stm, a_other)`, side-to-move first |
| 3     | Activation                        | 256          | 256           | No                          | ClippedReLU, clamp to `[0, max]` |
| 4     | Hidden (dense)                    | 256          | 32            | Yes (`W_2`, `b_2`)          | `h = ClippedReLU(W_2 a + b_2)` |
| 5     | Output (dense)                    | 32           | `policy_size` | Yes (`W_3`, `b_3`)          | `s = W_3 h + b_3`, raw per-move scores |

Compact notation: **`768 → 128×2 → 32 → policy_size`**

### Weight matrix shapes

- `W_1`: `128 × 768` (shared across both perspectives)
- `W_2`: `32 × 256`
- `W_3`: `policy_size × 32`

## 4. Incremental update rule (search-time only)

Only stage 1 (the feature transformer) supports incremental updates. On move
make/unmake, given the set of features that turned on/off for a given
perspective:

```
a' = a + Σ W_1[:, i] for i in features_turned_on
       − Σ W_1[:, j] for j in features_turned_off
```

Both `a_stm` and `a_other` are maintained on a stack, pushed before a move
is made and popped on unmake — no recomputation needed on unmake.

Stages 2–5 are recomputed fully and densely after every move; there is no
incremental path past the feature transformer.

## 5. Output encoding and masking

- `policy_size`: move-index space. v1 recommendation: `64 × 64 = 4096`
  (from-square × to-square), with promotions handled as a fixed multiplier
  or a special-cased subset. More compact move-plane encodings are a
  possible later optimization.
- At inference: compute `s ∈ ℝ^policy_size`, apply a legal-move mask (set
  illegal indices to `−∞` or drop them), then sort remaining entries
  descending. The mask comes from the legal move list the search already
  produces for expansion — no extra move-generation cost.
- No softmax needed if only a ranking is used; softmax is only relevant if
  training uses a probability-based loss instead of pairwise ranking.

## 6. Training target and loss

- **Label source:** the search engine's own results. Whenever the engine
  finishes proving a node, each child that received search effort can be
  recorded as a `(position, move, work)` triple, where `work` is the number
  of nodes visited while resolving that child's subtree.
- **Loss:** pairwise ranking loss (e.g. RankNet) over sibling pairs at a
  node, ordering the child with the smallest resolved subtree against its
  siblings. Children with zero recorded work are excluded from the loss
  (they are censored — the solver never needed to resolve them — not
  "cheap").
- **No absolute regression target** — only the relative order between
  siblings matters for move ordering.

## 7. Sizing rationale (v1)

| Parameter                   | Value          | Rationale                                                       |
| --------------------------- | -------------- | --------------------------------------------------------------- |
| Accumulator width           | 128            | Matches Stockfish's "small net"; ranking needs less resolution than eval |
| Hidden layer width          | 32             | Matches Stockfish's L2/L3 scale; small trunk keeps dense recompute cheap |
| Depth past accumulator      | 1 hidden layer | Avoid overfitting given limited (10^5–10^6 positions) training data |
| Quantization                | None in v1     | Validate architecture in float32 first; quantize only after benchmarking |
| PSQT buckets / layer stacks | None in v1     | Eval-precision tricks from engine evaluation; unnecessary for a ranking+search task |

## 8. Evaluation

Evaluate the trained network on root positions from a fixed suite chosen to
be move-ordering sensitive:

- **Primary metric:** total nodes visited to resolve each root position,
  with the same engine and the same time budget for the network ordering and
  the hand-crafted baseline ordering.
- **Secondary metric:** wall-clock time-to-solve per root position.

The suite must be held out from training; report wins/losses per position as
well as aggregates, since a handful of hard positions can dominate cost.

## 9. Open parameters to determine empirically

- `policy_size` exact scheme (plain 4096 vs. compact move-plane encoding)
- Whether king-relative or capture-target/explosion-relative features
  improve ranking quality enough to justify their complexity (v2 candidate)
- Whether the 128-wide accumulator is sufficient, or 256 is needed
- Training data budget: how many recorded `(position, move, work)` triples
  are needed for the ranking loss to saturate