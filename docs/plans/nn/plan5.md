# Plan: Gate 3 — Rust weight loader and inference path for the move-ordering network

Status: implemented (see `docs/plans/nn/report5.md`).

Gate 2 (external trainer, `report_external_trainer.md`) produced the §10
weight file and its reference implementation. This plan is the Rust-side
consumer: the weight loader, the §2 feature extractor, the §3 forward pass,
and the move-ordering integration point. It implements from
`docs/spec/nn.md` (normative); `docs/nn_trainer_ref/` is reference material
and conformance vectors only. Gate 4 measurement (nodes-visited vs the
hand-crafted ordering) is out of scope.

## Step 0 — spec erratum check (done)

`docs/spec/nn.md` §2 in this repo states the feature index as
`f = 64 * p + sq ∈ [0, 768)` (piece-major, square-minor). The erratum was
already applied to this copy before the handoff; no correction was needed and
the weight-file version stays 1.

## Deliverables

1. **Weight loader** (`src/nn/weights.rs`): parse the §10 file — 16-byte
   header (`u32` magic `0x4E4E5441` LE, `u16` version 1, input 768,
   accumulator 128, hidden 32, policy 4096, flags 0) followed by six float32
   LE row-major tensors in order `W_1 [128][768]`, `b_1 [128]`,
   `W_2 [32][256]`, `b_2 [32]`, `W_3 [4096][32]`, `b_3 [4096]`
   (967,312 bytes total). Hard errors on wrong magic, version, any dimension
   field, `flags != 0`, or a file size that disagrees with the header dims.
2. **Feature extractor** (`src/nn/features.rs`): from a `Board` + side to
   move, build two sparse binary 768-feature views with
   `sq = file + 8 * rank` (a1 = 0, h8 = 63; this equals the movegen square
   index), `p = 6 * view_color + type` (view color 0 = the view's own side;
   type 0..5 = pawn..commoner), `f = 64 * p + sq`. The side-to-move view is
   the board as-is; the other view swaps colors relative to the side to move
   and mirrors the file (`file -> 7 - file`, rank unchanged). Also
   `policy_index(from, to) = from_sq * 64 + to_sq` (§5). Conformance
   vectors: `docs/nn_trainer_ref/test_features.py` (lone white king a1, stm w
   → view A `{320}`, view B `{711}`; `4k3/8/8/8/8/8/8/4R1K1 w - - 0 1` →
   view A `{196, 326, 764}`, view B `{379, 579, 705}`).
3. **Forward pass** (`src/nn/eval.rs`): stage 1 accumulates
   `a = b_1 + Σ W_1[:, f]` over each view's active features (shared `W_1`),
   concatenates stm-first (256), clamps to `[0, 1]` (ClippedReLU, max = 1.0
   **hard-coded**, it is not in the file), then `h = clamp(W_2 a + b_2)`, and
   `s[idx] = W_3[idx, :] · h + b_3[idx]` evaluated only at the requested
   policy indices (equivalent to computing all of `s` and masking). The
   stage-1 primitive is per-view over a feature list so the §4 incremental
   update rule (`a' = a + Σ on − Σ off`, make/unmake stack) is a drop-in
   later; only stage 1 is incremental.
4. **Loader test** against the byte-frozen fixture
   `docs/nn_trainer_ref/fixtures/weights.v1.bin` (967,312 bytes; sha256
   `cb6dafd458d6ad044204f65f4faf378223527eee4ef09e707c9771d4946db2e0`,
   verified out-of-band with `sha256sum` before implementation): assert the
   header fields and the known nonzero tensor entries from
   `docs/nn_trainer_ref/test_weights.py` (`W_1[0][0] = 1.0`,
   `W_1[0][1] = -0.5`, `W_1[127][767] = 0.25`, `b_1[0] = 0.5`,
   `W_3[0][0] = 2.0`, `b_3[4095] = -0.125`, plus the seed-0 derived entries
   and per-tensor nonzero counts). The trained production file
   `data/corpus/weights.v1.bin` is never used in tests.
5. **Integration point** (`src/nn/scorer.rs` + `src/search/dfpn/`):
   `NnMoveScorer` ranks legal moves by `s[policy_index(from, to)]`.
   Promotion variants of one `(from, to)` map to one index — policy indices
   are collected, sorted, and deduplicated before the stage-5 rows are
   evaluated (the mask), then each move reads its index's score. Scores are
   RankNet margins: only the relative order is meaningful, so they are
   mapped monotonically to the `i32` ordering scale (round-after-scale never
   inverts) and only ever sorted, never thresholded. Composition follows
   `concept.md` §6: the network replaces the static term, history + killer
   stay additive, TT best move still goes first. The path is behind a flag
   (`Search::set_nn_scorer`, CLI `--nn-weights <FILE>`): unset, the
   hand-crafted `StaticAtomicScorer` ordering remains exactly as before for
   Gate 4 comparison.

## Design decisions

- **`W_1` is stored transposed in memory** (`[input][accumulator]`, i.e. one
  contiguous 128-float column per feature). Both the fresh sparse stage-1
  pass and the future §4 incremental update want `W_1[:, f]` contiguous; the
  file's row-major `[128][768]` layout is transposed once at load time.
  (Note: `report_external_trainer.md` open item 5 describes the column as
  living at file offset `16 + 4 * 128 * f`, which describes this transposed
  layout, not the §10 row-major file layout where element `(r, c)` sits at
  `16 + 4 * (r * 768 + c)`. The file format is untouched; the transposition
  happens in memory at load.)
- **Fixed-capacity feature sets** (`[u16; 64]` per view, a square can hold at
  most one piece) — no allocation in the scoring path.
- **Stage 5 is evaluated lazily per unique policy index** instead of
  materializing all 4096 outputs: a node has far fewer legal moves than
  4096, `W_3` is 4096×32, and the mask-with-dedup semantics are identical.
- **Scale mapping**: `nn_i32 = round(s * NN_SCORE_SCALE)` with
  `NN_SCORE_SCALE = 4096.0` (monotone, saturating), exposed via
  `NnMoveScorer::with_scale` so Gate 4 can tune the trade-off against
  history (≤ 10,000) and killer (50,000) bonuses without code changes.
- **Module layout**: `src/nn/{mod,weights,features,eval,scorer}.rs`, all
  under the 10 KB guideline; `search` gains an
  `Option<NnMoveScorer>` field but no dependency on the forward-pass
  internals (`search` already depends on `ordering`; `nn` sits beside it and
  only the scorer type crosses the boundary).

## Tests

- `src/nn/weights.rs`: fixture header fields, known tensor values (fixed
  corners + seed-0 derived entries), per-tensor nonzero counts, and the hard
  error paths (magic, version, each dim, flags, truncated file, short
  header, missing file).
- `src/nn/features.rs`: the six hand-computed conformance vectors from
  `test_features.py` (both stms, both views), startpos feature counts
  (32 active per view, binary), `policy_index` layout
  (`a1a2 = 8`, `e2e4 = 796`, `h8h1 = 4039`), and index bounds.
- `src/nn/eval.rs`: hand-computed fixture forward pass for the lone-king
  position (`h = [0.5, 0, …, 0]`, `s[0] = 1.125`, `s[1] = 0.25`,
  `s[4095] = −0.125`), accumulator-vs-dense cross-check on the startpos,
  and ClippedReLU clamping at both ends via a synthetic in-memory weight
  file (clamp max = 1.0 is exercised, not just assumed).
- `src/nn/scorer.rs`: score vector aligned with the move list, promotion
  variants of one `(from, to)` share one score (dedup), determinism, and
  monotone `i32` mapping.
- `tests/test_nn.rs`: integration — with `--nn-weights` semantics
  (`Search::set_nn_scorer`), a tactical position still solves to the correct
  outcome, and promotion moves deduplicate to identical scores through the
  public scorer API.

## Verification

`cargo fmt --check`, `cargo clippy --all-targets`, `make test`
(`CARGO_PROFILE_RELEASE_LTO=thin cargo test --release`), plus
`cargo test --release -p atomic_solver test_nn` for the new suite.

## Out of scope

- Gate 4 measurement (nodes-visited / wall time vs the hand-crafted ordering).
- Any training, quantization, or change to the §10 file format.
- The full incremental make/unmake accumulator stack in the search loop
  (stage 1 is shaped for it; wiring it into `Search` is future work).
- Modifications under `docs/nn_trainer_ref/` (byte-frozen) and any use of
  `data/corpus/weights.v1.bin` values in tests.

## Final task

Write the implementation report `docs/plans/nn/report5.md` (tools used,
problems, deviations, missing tests, next steps).
