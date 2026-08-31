# Handoff: Gate 2 trainer → Gate 3 Rust loader

Status: Gate 2 (external trainer) is implemented and verified — see
`docs/plans/nn/report_external_trainer.md` for the full report. This file is
the short list of what to hand the Rust side and why.

## Must share — build-time (write and verify the Rust loader)

| #   | File                                                                                                                                                                                                    | Purpose                                                                                                                                                                                                                                                                                                                                                                    | Role               |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ |
| 1   | `docs/spec/nn.md` (§2 already corrected to `f = 64 * p + sq`)                                                                                                                                           | The normative contract: feature layout §2, architecture + ClippedReLU max = 1.0 §3, incremental update §4, policy indexing §5, weight-file bytes §10. The Rust side implements from this.                                                                                                                                                                                  | Specification      |
| 2   | `docs/plans/nn/report_external_trainer.md`                                                                                                                                                              | What was actually decided and built: the pinned index formula and why (the spec's literal `f = 64 * sq + p` cannot fit `[0, 768)`), the exact input contract consumed, and the "Open items for Gate 3" checklist.                                                                                                                                                          | Errata + decisions |
| 3   | `trainer/fixtures/weights.v1.bin`                                                                                                                                                                       | 967,312-byte §10 weight file with 16 known nonzero values at known tensor positions (13 fixed corner entries plus one seed-0 entry per weight tensor). The Rust loader test asserts exact header fields and exact tensor reads against it. Byte-stable across runs/platforms; regenerate anywhere with `trainer.weights.write_sample(path, seed=0)` (in the trainer repo). | Test fixture       |
| 4   | `trainer/test_features.py` — at minimum its hand-computed index vectors (lone white king a1 → view A `[320]`, view B `[711]`; spec FEN `4k3/8/8/8/8/8/8/4R1K1` → `[196, 326, 764]` / `[379, 579, 705]`) | Conformance vectors for the Rust feature extractor. Feature bugs do not crash — they silently scramble inputs; these vectors catch that.                                                                                                                                                                                                                                   | Test vectors       |

## Must share — runtime (the model the Rust app loads)

| #   | File                              | Purpose                                                                                                                                                                                                                                                                    | Role             |
| --- | --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------- |
| 5   | `data/corpus/weights.v1.bin`      | The trained model itself — the deliverable Gate 2 exists to produce and Gate 3 exists to consume. Without it there is nothing to integrate. Generated and git-ignored (regenerate with the command in the report); ship the current one whenever the model is (re)trained. | Production input |
| 6   | `data/corpus/weights.v1.bin.json` | Provenance of the trained file: corpus version, seed, hyperparameters, loss history — reproduction and audit.                                                                                                                                                              | Model metadata   |

## Reference (share for questions, not as the source of truth)

| #   | File                  | Purpose                                                                                                                                        |
| --- | --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| 7   | `trainer/features.py` | Readable reference implementation of §2 — use to disambiguate prose, not to replace it                                                         |
| 8   | `trainer/weights.py`  | Reference §10 reader/writer; its validation logic (magic/version/dims/flags/size hard errors) is exactly what the Rust loader should replicate |

These are shared as-is (no snapshot copies inside the trainer repo): a
copied file would immediately start drifting from the live trainer code,
and the single-source-of-truth rule (spec normative, trainer code the one
reference implementation) is what the §2 erratum taught us to protect.
Inside the Rust repo they land in `docs/nn_trainer_ref/` (see the copying
section) — reference material and test vectors, out of `src/`. Each shared
module carries a docstring note stating that `docs/spec/nn.md` is
normative.

## Do NOT need to share

- Rest of `trainer/` (`corpus.py`, `model.py`, `loss.py`, `train.py`,
  `test_corpus.py`, `test_loss.py`, `test_train.py`) — training-only.
  Exception: ship `trainer/test_weights.py` alongside fixture #3 — the
  Gate-3 prompt references it for the fixture's expected values.
- `pyproject.toml`, `uv.toml`, `uv.lock`, `.python-version` — the Rust side
  does not run Python.
- `data/corpus/train.ndjson` — trainer-only input.

## Copying into the Rust repo

Trainer reference material lives under `docs/` in the Rust repo — from that
repo's perspective the Python files are documentation (unreadable as build
input, cargo ignores them), so they go to `docs/nn_trainer_ref/` instead of
polluting the repo root. Mapping:

| This repo                                                                     | Rust repo                                               |
| ----------------------------------------------------------------------------- | ------------------------------------------------------- |
| `docs/spec/nn.md`                                                             | `docs/spec/nn.md` (overwrite — the §2 erratum fix)      |
| `docs/plans/nn/report_external_trainer.md`, `handoff.md`, `handoff-prompt.md` | unchanged                                               |
| `trainer/features.py`, `weights.py`, `test_features.py`, `test_weights.py`    | `docs/nn_trainer_ref/`                                  |
| `trainer/fixtures/weights.v1.bin`                                             | `docs/nn_trainer_ref/fixtures/weights.v1.bin`           |
| `data/corpus/weights.v1.bin` (+ `.json`)                                      | `data/corpus/` (or wherever the Rust loader expects it) |

Commands, from the trainer repo root:

```bash
rust=/path/to/rust/repo
ref=docs/nn_trainer_ref   # trainer reference material inside the Rust repo

mkdir -p "$rust/$ref/fixtures" "$rust/docs/plans/nn" "$rust/data/corpus"

# trainer reference implementations + conformance vectors (docs, not code)
cp trainer/features.py trainer/weights.py \
   trainer/test_features.py trainer/test_weights.py "$rust/$ref/"
cp trainer/fixtures/weights.v1.bin "$rust/$ref/fixtures/"

# normative docs (nn.md overwrites the stale Rust-side copy — intended)
cp docs/spec/nn.md "$rust/docs/spec/"
cp docs/plans/nn/report_external_trainer.md \
   docs/plans/nn/handoff.md \
   docs/plans/nn/handoff-prompt.md "$rust/docs/plans/nn/"

# the trained model + provenance
cp data/corpus/weights.v1.bin data/corpus/weights.v1.bin.json "$rust/data/corpus/"
```

Notes:

- The copy list is exactly the must-share + reference tables above; do not
  copy the rest of `trainer/` or the uv scaffolding (see "Do NOT need to
  share").
- `handoff-prompt.md` references the **destination** paths
  (`docs/nn_trainer_ref/...`) — if you relocate the model file, adjust the
  prompt accordingly.
- Fixture integrity is self-verifying: `handoff-prompt.md` deliverable 4
  has the agent check sha256
  `cb6dafd458d6ad044204f65f4faf378223527eee4ef09e707c9771d4946db2e0`
  before use.
- The trained weights are regenerable; whether the Rust repo tracks
  `data/corpus/weights.v1.bin` in git is its own choice.

## The contract in one paragraph

The weight file (§10) is 16 bytes of header (magic `0x4E4E5441`/"ATNN",
version 1, input 768, accumulator 128, hidden 32, policy 4096, flags 0) plus
six float32 little-endian row-major tensors — `W_1 [128][768]`, `b_1 [128]`,
`W_2 [32][256]`, `b_2 [32]`, `W_3 [4096][32]`, `b_3 [4096]` — 967,312 bytes
total. It contains tensors only. Everything else needed at inference is
pinned in the spec and must be implemented identically on the Rust side:
feature index `f = 64 * p + sq` (piece-major, square-minor; `sq = file +
8 * rank`, `p = 6 * view_color + type`), the other-view transform (color
swap + file mirror), ClippedReLU clamp max = 1.0 hard-coded (not in the
file), policy index `from_sq * 64 + to_sq` with promotion variants collapsed
onto one index (dedup the mask), and score semantics where only the relative
order is meaningful (sort, never threshold).

## Gate 3 checklist (from the report, repeated for convenience)

1. Fix nn.md §2's formula text first, then implement `f = 64 * p + sq` plus
   the other-view transform exactly as `trainer/features.py`; cross-check
   against items 3 and 4 above.
2. Loader validation: magic/version/dims/flags hard errors; reject
   `flags != 0` and any file size that disagrees with the header dims.
3. Hard-code the ClippedReLU clamp max = 1.0; changing it requires a
   weight-file version bump.
4. Promotions: multiple variants of one `(from, to)` map to one policy
   index; deduplicate indices before masking.
5. Incremental accumulator (§4) reads `W_1` columns directly from the
   row-major file (column `i` at offset `16 + 4 * 128 * feature`).
6. Scores are RankNet margins: sort by score, never threshold.

## Agent prompt (Gate 3)

The ready-to-use prompt for the Rust-side coding agent lives in its own
file: **`docs/plans/nn/handoff-prompt.md`**. Copy the files listed above
into the Rust repo — **including both handoff files** (`handoff.md` and
`handoff-prompt.md`, since the prompt's "Read first" list references the
inventory) — then either paste the prompt file's contents to the agent or
simply tell it:

> Read and follow `docs/plans/nn/handoff-prompt.md`.

Keeping the prompt as a file means the agent can re-read it mid-task, you
can diff future revisions of it, and no four-backtick fence gymnastics are
needed. The prompt expects the inventory above (spec, report, fixture,
feature-test vectors) to be present in the Rust repo; it instructs the
agent to produce `docs/plans/nn/plan_gate3.md` + `report_gate3.md` per repo
convention.
