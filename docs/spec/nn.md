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

### Exact feature index layout (v1)

Both views share the same index formula (this is part of the trainer/loader
contract; the external trainer and the Rust inference code must implement it
identically):

- **square index:** `sq = file + 8 * rank`, `file` ∈ [0, 8) = a..h,
  `rank` ∈ [0, 8) = 1..8; so `a1 = 0`, `h8 = 63`.
- **piece index:** `p = 6 * color + type`, `color` ∈ {0 = this view's own
  side, 1 = the other side}, `type` ∈ {0 = pawn, 1 = knight, 2 = bishop,
  3 = rook, 4 = queen, 5 = king}.
- **feature index:** `f = 64 * sq + p` ∈ [0, 768).

### FEN parsing and the corpus king convention

Features are extracted from the corpus `fen` field plus `stm`. The corpus FENs
are the engine's **round-trip** FENs (`Position::fen()` → `atomic-movegen`'s
`Board::fen()`). Since atomic-movegen 2.1.0, FEN parsing and output are
**standard notation only — kings label the commoner** (in atomic chess the
king is a capturable piece, not a standard-chess royal), e.g.
`4k3/8/8/8/8/8/8/4R1K1 w - - 0 1`. Every corpus row uses standard `k`/`K`.
The pre-2.1.0 `c`/`C` commoner spelling is obsolete: 2.1.0 rejects it on
input, corpora generated before 2.1.0 are stale and must be regenerated, and
the feature extractor only needs the standard piece letters (case = color).

### Split by perspective

Every position is encoded twice, relative to the side to move: once as seen
by the side to move and once as seen by the other side. The transform for the
other side's view is: **swap colors and mirror the board across the center
file** (`file f` → `7 - f`, rank unchanged); in that view `color 0` is the
side that was not the side to move. Both views share a single linear
projection (§3), so a feature means the same thing in either perspective.
The trainer and the Rust loader must apply this exact transform.

**Why the file mirror (and not another transform):** v1 has no castling-rights
or en-passant features, so a position and its horizontal mirror are
strategically identical — mirroring the file is therefore free augmentation:
each position trains `W_1` on both itself and its mirror image. A rank flip
(the NNUE-conventional look) would be unsound here: with absolute square
indices it would teach `W_1` a false vertical-mirror equivalence, since pawn
direction makes the board rank-asymmetric. A pure color swap (no spatial
transform) is correct but forfeits the augmentation.

## 3. Layer-by-layer architecture

| Stage | Operation                          | Input dim    | Output dim    | Learned?                    | Notes                                            |
| ----- | ---------------------------------- | ------------ | ------------- | --------------------------- | ------------------------------------------------ |
| 1a    | Feature transformer (side-to-move) | 768 (sparse) | 128           | Yes (`W_1`, shared weights) | `a_stm = W_1 x_stm + b_1`     |
| 1b    | Feature transformer (other side)   | 768 (sparse) | 128           | Yes (same `W_1`)            | `a_other = W_1 x_other + b_1` |
| 2     | Concatenate                        | 128 + 128    | 256           | No                          | `a = concat(a_stm, a_other)`, side-to-move first |
| 3     | Activation                        | 256          | 256           | No                          | ClippedReLU, clamp to `[0, 1]` (max pinned, see below) |
| 4     | Hidden (dense)                    | 256          | 32            | Yes (`W_2`, `b_2`)          | `h = ClippedReLU(W_2 a + b_2)`, same clamp |
| 5     | Output (dense)                    | 32           | `policy_size` | Yes (`W_3`, `b_3`)          | `s = W_3 h + b_3`, raw per-move scores |

**Clamp max is part of the inference contract.** Both ClippedReLU stages use
`max = 1.0`. The weight file (§10) does not record the activation function or
its clamp max — only the shapes are in the header — so the trainer and the
Rust loader (Gate 3) must hard-code the same value; changing it requires a
version bump of the weight file.

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

- `policy_size`: **pinned to 4096 for v1**: `policy_index = from_sq * 64 +
  to_sq`, with the square indexing of §2 (`a1 = 0`, `h8 = 63`). All four
  promotion variants of a pawn move map to the same `(from, to)` index;
  promotion is not distinguished in v1. A compact move-plane encoding is a
  v2 optimization only if Gate 4 shows the 4096-row output head is a
  measurable cost.
- At inference: compute `s ∈ ℝ^policy_size`, apply a legal-move mask (set
  illegal indices to `−∞` or drop them), then sort remaining entries
  descending. The mask comes from the legal move list the search already
  produces for expansion — no extra move-generation cost.
- No softmax needed if only a ranking is used; softmax is only relevant if
  training uses a probability-based loss instead of pairwise ranking.

## 6. Training target and loss

- **Label source:** the search engine's own results. Whenever the engine
  finishes proving a node, each child that received search effort is
  recorded as a `(position, move, work)` triple, where `work` is the
  cumulative `child_evals` the search spent proving that child's subtree,
  recorded at prove time (proof-tree dump v2, `docs/spec/proof_tree_dump.md`)
  and carried in the NDJSON corpus as `children[].work`.
- **OR nodes** (`outcome == "win"`): the proven decisive child(ren) must rank
  above every other legal move (one-vs-rest pairs).
- **AND nodes** (`outcome == "loss"`): every child is expanded; rank the
  children by their recorded `work`, lowest (cheapest) first.
- **Loss:** pairwise ranking loss (e.g. RankNet) over sibling pairs at a
  node, using the OR/AND targets above. Children with zero recorded work are
  excluded from the loss (they are censored — the solver never needed to
  resolve them — not "cheap"). "Excluded" means: never the *preferred*
  (first) element of a pair, and no pair between two censored moves. It does
  not remove them from the OR one-vs-rest pairs above — on OR nodes every
  legal move that is not a proven decisive child, including never-expanded
  moves that have no `children[]` entry at all, is the negative side of a
  pair. (In `atomic-corpus/2` every expanded child has `work >= 1`, so the
  zero-work censoring case does not occur in the shipped corpus.)
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

- Whether king-relative or capture-target/explosion-relative features
  improve ranking quality enough to justify their complexity (v2 candidate)
- Whether the 128-wide accumulator is sufficient, or 256 is needed
- Training data budget: how many recorded `(position, move, work)` triples
  are needed for the ranking loss to saturate

## 10. Weight-file format (v1, pinned)

This section is the byte-level contract between the external trainer
(producer) and the Rust weight loader (consumer, Gate 3). It must not change
without a version bump.

All integers little-endian. One header, then all tensors as IEEE-754 binary32
(float32), little-endian, row-major.

```
Header (16 bytes):
  u32 magic        0x4E4E5441   // ASCII "ATNN" as bytes 41 54 4E 4E
  u16 version      1
  u16 input        768
  u16 accumulator  128
  u16 hidden       32
  u16 policy       4096
  u16 flags        0            // 0 = float32, unquantized

Tensors, in this order (all float32, row-major):
  W_1  [128][768]   // rows = accumulator, cols = input
  b_1  [128]
  W_2  [32][256]
  b_2  [32]
  W_3  [4096][32]
  b_3  [4096]
```

The header is exactly 16 bytes: the `u32` magic plus six `u16` fields — no
padding, no reserved field, no alignment words. Total size:
`16 + 4 * (98,304 + 128 + 8,192 + 32 + 131,072 + 4,096)`
= `16 + 4 * 241,824` = 967,312 bytes. With `W_1` stored row-major
(rows = accumulator, columns = input), column `i` is exactly the
incremental-update vector `W_1[:, i]` used in §4.

The loader (Gate 3) must validate `magic`, `version`, and the four dimension
fields against the architecture before reading; any mismatch is a hard error.
`flags` reserves room for a quantized v2 (e.g. int8 + scale) without
renaming the file.