# Task: Implement Gate 3 — Rust weight loader and inference path

> Meta note for the human, not the agent: this file is the prompt for the
> Rust-side coding agent. Either paste its contents into the agent, or point
> the agent at this file ("read and follow
> `docs/plans/nn/handoff-prompt.md`"). The file inventory it references
> lives in `docs/plans/nn/handoff.md`. Delete this blockquote before
> pasting if the agent is confused by it.

Implement Gate 3 of docs/plans/nn/concept.md: the Rust weight loader and
inference path for the move-ordering network, plus its integration point.
A Python trainer (Gate 2) already produced the weight file; you are writing
the consumer. The spec is normative — do not run, import, or port the
Python trainer wholesale. The shared `trainer/` files are reference
material: read them to disambiguate the spec, and use the two test files
as conformance vectors, but implement from nn.md.

Read first (in this order):
1. docs/spec/nn.md — the normative contract (§2 features, §3 architecture,
   §4 incremental accumulator, §5 output/masking, §6 loss semantics,
   §10 weight-file byte layout).
2. docs/plans/nn/report_external_trainer.md — what the trainer actually
   built; especially "Feature extraction as implemented" and
   "Open items for Gate 3".
3. docs/plans/nn/handoff.md — the file inventory and the contract summary.

Step 0 — verify the spec erratum was applied: nn.md §2 must state the
feature index as "f = 64 * p + sq ∈ [0, 768)" (piece-major: the 12-valued
piece axis is the outer, 64-strided axis). The trainer-side copy was
corrected before the handoff; the earlier draft's "f = 64 * sq + p" cannot
fit [0, 768) (it reaches 4043). If your copy of nn.md still shows the old
formula, you have a stale copy — correct it, flag the mismatch in your
report, and do not implement from it. The §10 file format itself is
unchanged — do not bump the weight-file version.

Deliverables:
1. Weight loader (e.g. src/nn/weights.rs): parse the §10 file — 16-byte
   header (u32 magic 0x4E4E5441 little-endian = ASCII "ATNN", u16 version 1,
   u16 input 768, u16 accumulator 128, u16 hidden 32, u16 policy 4096,
   u16 flags 0) followed by six float32 LE row-major tensors in order:
   W_1 [128][768], b_1 [128], W_2 [32][256], b_2 [32], W_3 [4096][32],
   b_3 [4096] (total 967,312 bytes). Hard-error on wrong magic, version,
   any dim, flags != 0, or a file size that disagrees with the header dims.
2. Feature extractor (e.g. src/nn/features.rs): from a Position + stm,
   build two 768-dim sparse binary views with f = 64*p + sq; the other view
   swaps colors and mirrors the file (file -> 7 - file, rank unchanged).
   The side-to-move view is never mirrored. Unit-test against the
   hand-computed vectors in docs/nn_trainer_ref/test_features.py (shared
   with you): lone white king a1, stm w -> view A index 320, view B index
   711; FEN 4k3/8/8/8/8/8/8/4R1K1 w - - 0 1 -> view A {196, 326, 764},
   view B {379, 579, 705}.
3. Forward pass (e.g. src/nn/eval.rs): a_stm = W_1 x_stm + b_1,
   a_other = W_1 x_other + b_1 (shared W_1), concat (stm first, 256),
   clamp to [0, 1] (ClippedReLU, max = 1.0, hard-coded — it is NOT in the
   file), h = clamp(W_2 a + b_2), s = W_3 h + b_3. Keep the incremental
   accumulator path (§4) in mind but only stage 1 needs it.
4. Loader test against the fixture
   docs/nn_trainer_ref/fixtures/weights.v1.bin
   (967,312 bytes; sha256
   cb6dafd458d6ad044204f65f4faf378223527eee4ef09e707c9771d4946db2e0):
   verify the copy's hash, then assert header fields and the known nonzero
   tensor entries (see docs/nn_trainer_ref/test_weights.py for the exact
   values, e.g. W_1[0][0] = 1.0, W_1[0][1] = -0.5, W_1[127][767] = 0.25,
   b_1[0] = 0.5, W_3[0][0] = 2.0, b_3[4095] = -0.125).
5. Integration point: rank legal moves by s[policy_index] with
   policy_index = from_sq*64 + to_sq (square indexing as in §2). Multiple
   promotion variants map to one index — deduplicate before masking. Scores
   are RankNet margins: only the relative order is meaningful, so sort and
   never threshold. Compose additively or as the primary order per
   concept.md §6 ("nn + history + killer") behind a config flag so the
   hand-crafted ordering remains available for Gate 4 comparison.

Constraints:
- Write a plan first (docs/plans/nn/plan5.md)
- Do not modify anything under docs/nn_trainer_ref/; the fixture is
  byte-frozen.
- data/corpus/weights.v1.bin is the trained production model (regenerated
  on retrain — never hard-code values from it in tests; use the fixture).
- Out of scope: Gate 4 measurement (nodes-visited vs hand-crafted ordering),
  any training, quantization, changing the file format.
- Verify with the repo's usual gates (cargo fmt --check, clippy, tests).
