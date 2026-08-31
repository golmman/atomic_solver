# Report: external trainer (Gate 2)

Plan: `docs/plans/nn/plan_external_trainer.md`. Status: **implemented and
verified**. The trainer consumes the Gate-1/1.5 NDJSON corpus
(`atomic-corpus/2`) and emits the `docs/spec/nn.md` §10 float32 weight file;
the Rust loader (Gate 3) is future work and out of scope here.

## What was built

| File | Contents |
|---|---|
| `trainer/features.py` | §2 two-perspective feature extraction (FEN parser, index layout, other-side transform), §5 `policy_index` |
| `trainer/corpus.py` | NDJSON parsing + validation, §6 label-pair generation |
| `trainer/model.py` | §3 network (768 → 128×2 → 32 → 4096, shared `W_1`, ClippedReLU) |
| `trainer/loss.py` | RankNet pairwise loss (§6): `softplus(-(s_i - s_j) * T)`, `T = 1.0` |
| `trainer/weights.py` | §10 weight-file writer/reader (16-byte header + six float32 tensors), `write_sample` fixture writer |
| `trainer/train.py` | CLI (`python -m trainer.train`) |
| `trainer/test_*.py` | 49 pytest tests (unit + full-corpus smoke) |
| `trainer/fixtures/weights.v1.bin` | deterministic sample weight file (seed 0), committed for the Gate-3 Rust loader test |
| `pyproject.toml`, `uv.toml`, `uv.lock`, `.python-version`, `.gitignore` | uv project (Python 3.12.13, torch 2.13.0+cpu, numpy, pytest) + ignore rules |
| `data/corpus/weights.v1.bin` (+ `.json` summary) | trained weights + training summary (generated, git-ignored) |

Dependencies are exactly numpy/torch (+ pytest as dev dep), resolved from
`https://download.pytorch.org/whl/cpu` (`uv.lock` asserts this —
`torch==2.13.0+cpu`). All config lives in repo-root `uv.toml`
(`cache-dir = ".uv-cache"` + the CPU-only PyTorch index);
`pyproject.toml` carries no `[tool.uv]` table, per the plan. `uv sync
--locked` installs the CPU wheel and nothing else. The trainer imports no
Rust and no solver code.

CLI as implemented:

```
uv run python -m trainer.train --corpus <ndjson> --out <weights.bin>
    [--epochs N=8] [--lr F=1e-3] [--batch N=4096] [--seed N=0]
    [--keep-partial] [--validation-fraction F=0.1] [--source-fraction F=1.0]
    [--sample-out <path>] [--sample-seed N=0] [--max-rows N]
    [--dropout F=0.1] [--l2 F=1e-4] [--device cpu|cuda]
```

Per-epoch train/val loss goes to stdout; the run ends with the §10 weight
file and a `<out>.json` summary (history, split, pair counts,
hyperparameters, platform, wall time).

## Feature extraction as implemented

`trainer/features.py` parses the corpus `fen` (standard piece letters only,
`k`/`K`; an en-passant target square is FEN syntax, not a piece; missing
clock fields default) and emits two 768-dim binary float32 tensors:

- square index `sq = file + 8 * rank` (`a1 = 0`, `h8 = 63`),
- piece index `p = 6 * view_color + type`, view color 0 = the view's own
  side, `type` 0..5 = pawn..king,
- feature index `f = 64 * p + sq` ∈ [0, 768).

**Spec discrepancy resolved here (the Gate-3 contract).** `nn.md` §2 writes
the feature index as `f = 64 * sq + p`, which cannot fit `[0, 768)`: with
`sq ∈ [0,64)` and `p ∈ [0,12)` it reaches 4043 (the plan's own worked
examples only stay below 768 because their squares are small — the real
corpus produced index 3147 on its very first row). Because the §2 feature
count (768), the §3 `W_1` shape (128×768) and the §10 header (`input = 768`)
are all pinned to 768, the pinned reading keeps the literal `64` multiplier
on the 12-valued axis: **`f = 64 * p + sq`** (piece-major, square-minor —
the NNUE/Stockfish convention §2 references). The worked examples under this
layout: lone white king a1 (stm w) → view A `f = 64*5 + 0 = 320`, view B
(mirror `a1 → h1`, color swap) `f = 64*11 + 7 = 711`. Both `nn.md` §2 and
plan §3 should be corrected to `f = 64 * p + sq` before Gate 3.

Other view (§2 "Split by perspective"): colors swapped relative to the side
to move **and** files mirrored (`file -> 7 - file`, rank unchanged). The
side-to-move view is the board as-is for either stm color (the mirror is a
property of the other-view transform, not of which color is "own").

Promotion moves collapse onto one §5 index (`policy_index = from_sq * 64 +
to_sq`, square indexing as above); the trainer only uses the mask through
legal-move UCI strings, so collapsing is harmless trainer-side.

## Label-pair generation as implemented

Rows are validated while parsing: `hash` decoded with exact `int` semantics
and unique, `len(legal_moves) == len(static_scores)`, every `children[].mv`
in `legal_moves`, `win` rows carry a decisive (`loss`) child, `atomic-corpus/2`
children carry `work`. Pairs:

- **OR** (`outcome == "win"`): one pair per (decisive child, legal move not
  decisive), unweighted. No pairs among non-decisive moves; no pairs between
  two decisive moves.
- **AND** (`outcome == "loss"`): children sorted by `work` ascending
  (stable); one pair per unordered pair with unequal `work`, cheaper child
  first. Zero-work children (none exist in this corpus — report4 verified
  `work ≥ 1` for all AND children) are excluded entirely, per nn.md §6
  censoring. Legal moves without a `children[]` entry are censored: on AND
  rows they appear in no pair; on OR rows they are the negative side of the
  one-vs-rest pairs (per the plan's risk table, excluding them there would
  leave zero pairs on all win rows).
- **`partial`**: dropped by default (the trusted-label run used the
  default); `--keep-partial` re-includes them.

Measured on `data/corpus/train.ndjson` (`atomic-corpus/2`, 26,475 rows,
`partial_rows = 17,345`): default mode keeps **9,130 rows** (6,545 win /
2,585 loss) from **26** of the 54 distinct sources (28 sources are entirely
partial and vanish with the drop) and yields **200,508 OR + 12,727 AND =
213,235 pairs**. `--keep-partial` gives 583,328 OR + 50,779 AND pairs. The
AND-pair count is small because most AND nodes have 2–3 children; the loss
is OR-dominated.

## Training run

```
uv run python -m trainer.train \
    --corpus data/corpus/train.ndjson --out data/corpus/weights.v1.bin \
    --epochs 3 --seed 0 --lr 3e-4 --dropout 0.4 --l2 3e-3 \
    --sample-out trainer/fixtures/weights.v1.bin
```

- torch 2.13.0+cpu, Python 3.12.13, aarch64 container, ~22 s wall.
- Corpus: 26,475 rows read, 17,345 partial rows dropped → 9,130 kept
  (6,545 win / 2,585 loss), 26 sources.
- Pairs: train 162,348 OR + 8,992 AND; val 38,160 OR + 3,735 AND
  (unweighted RankNet, `T = 1.0`; Adam, batch 4096 pairs, L2 `3e-3` on
  `W_2`/`W_3` only, dropout 0.4 on the 256-dim activation).

| Epoch | train loss | val loss |
|---|---|---|
| 1 | 0.650986 | 0.689854 |
| 2 | 0.383910 | 0.542960 |
| 3 | 0.249644 | 0.529742 |

Validation loss decreases monotonically (0.690 → 0.530) over the run, per
the plan's verification bar. Longer runs (6–8 epochs) were tried and settle
at a val minimum around epoch 3 (≈ 0.530) before creeping up — the tiny net
saturates the ~213k-pair signal in 2–3 epochs; the run above stops at the
bottom of the val curve. This is a feasibility check, not a tuned model
(Gate 4 decides whether the ranking is good enough to matter).

## Validation split

Case-level split (`--validation-fraction 0.1`, seed 0): sources shuffled
with the run seed, added until the held-out row count reaches the target.
Whole cases move together, so transposition duplicates never straddle the
split.

- 54 sources exist in the raw corpus; after the default partial drop only
  **26** survive, and all 26 were used (`--source-fraction 1.0`).
- Validation sources and their kept-row counts: dec02 (688), dec33 (45),
  dec38 (1241), dec45 (9), m26_black (2) — 1,985 val rows vs 7,145 train
  rows (21.7% held out; the case-level split overshoots the nominal 10%
  because surviving sources are few and skewed: dec38 alone is 1,241 rows).
- **Leakage caveat (carried from the plan):** the `quick` suite contains
  move-order cases ≥ m23 by design, so m23+ are training data. The honestly
  held-out move-order cases are m20–m22 only (they contribute 0 rows here —
  none of their rows survived dedup as kept-row sources — so nothing in this
  corpus is a clean move-order holdout). Gate 4's evaluation must use m20–
  m22 (and fresh positions), not validation loss, as the ordering metric.

## Emitted weight file

`data/corpus/weights.v1.bin` — the §10 layout, verified:

- header 16 bytes: magic `0x4E4E5441` (bytes `41 54 4E 4E` = "ATNN"),
  version 1, input 768, accumulator 128, hidden 32, policy 4096, flags 0;
- six float32 little-endian row-major tensors in §10 order
  (`W_1 [128][768]`, `b_1 [128]`, `W_2 [32][256]`, `b_2 [32]`,
  `W_3 [4096][32]`, `b_3 [4096]`);
- total size 967,312 bytes; `trainer.weights.read` round-trips it
  byte-identically (write → read → write reproduces the file exactly).

Sample fixture: `trainer/fixtures/weights.v1.bin`, written by
`trainer.weights.write_sample(path, seed=0)` — all-zero tensors plus 16
nonzero entries (13 fixed corner entries plus one seed-dependent entry per
weight tensor; the exact values are asserted in `trainer/test_weights.py`),
967,312 bytes, byte-stable across processes and platforms (no RNG, no float
formatting). This is the Gate-3 round-trip fixture.

The training summary (`data/corpus/weights.v1.bin.json`) records the corpus
meta, partial mode, split, pair counts, hyperparameters, per-epoch losses,
torch/python versions, and the output file size.

## Input contract consumed

- `data/corpus/train.ndjson`, `_meta` line = `atomic-corpus/2`
  (`suite quick`, `timeout 20`, 59 cases/bins), then one compact JSON row
  per line (serde_json key order is irrelevant; the parser reads by key).
- `hash` decoded with Python exact `int` semantics (never float); uniqueness
  is enforced at parse time with exact u64 integer comparison.
- `len(legal_moves) == len(static_scores)` and every `children[].mv ∈
  legal_moves` are hard validation errors; `children[].work` must be present
  (atomic-corpus/2), a v1 corpus without `work` fails loudly (the loader
  warns on a non-`atomic-corpus/2` `_meta`).
- `partial` rows dropped by default; `--keep-partial` re-enables.
- Features from `fen` + `stm` only; `legal_moves` gives the mask
  (multiple promotions collapse onto one policy index, all still legal);
  `static_scores`/`first_decisive_rank`/`subtree_size` are parsed but not
  used as training labels.

## Diff vs plan

- **§2 index formula corrected (user-confirmed):** the spec's literal
  `f = 64 * sq + p` cannot fit `[0, 768)`; the trainer pins
  `f = 64 * p + sq` (piece-major, square-minor) and the worked-example
  indices become 320 (view A) / 711 (view B). nn.md §2 has since been
  corrected in the source repo (`docs/spec/nn.md`); the historical plan
  documents keep their original text, with this bullet as the record.
- **The plan's worked example FEN was mislabeled "lone king":**
  `4k3/8/8/8/8/8/8/4R1K1` has three pieces; the lone-king FEN is
  `8/8/8/8/8/8/8/K7`. Tests cover both.
- **`--source-fraction` semantics:** fraction of *sources* (cases) kept,
  seed-shuffled, smallest overshoot; used for quick runs (default 1.0).
- **Hyperparameters chosen empirically:** the plan left epochs/LR open.
  Defaults: epochs 8, lr 1e-3, batch 4096, dropout 0.1, L2 1e-4. The
  delivered run uses 3 epochs / lr 3e-4 / dropout 0.4 / L2 3e-3 — the
  val curve bottoms at epoch 3 and rises beyond it at any weaker
  regularization tried (9k training rows vs ~242k weights overfits fast).
- **Sources after the partial drop:** the plan speaks of 54 distinct
  sources; that is the full-corpus count. With partial rows dropped only 26
  sources (7,145 + 1,985 rows) remain, and the case-level validation split
  overshoots the 10% target to 21.7% — recorded in the summary JSON.
- **AND pairs are a small minority** (12,727 of 213,235) — worth a
  class-weighting or OR-pair-subsampling ablation in a future tuning pass.
- `AGENTS.md` did not exist in this repo snapshot; created with the
  trainer note instead of updated.

## Verification

- `uv run pytest trainer/` — 49 passed (unit tests + full-corpus smoke
  tests: row counts 26,475/9,130, unique-hash parse, 213,235 default-mode
  pairs, 583,328/50,779 pairs with `--keep-partial`, features binary and
  piece-count-consistent over the kept rows).
- `data/corpus/weights.v1.bin`: 967,312 bytes, header
  `(1, 768, 128, 32, 4096, flags=0)`, byte-identical write→read→write.
- `trainer/fixtures/weights.v1.bin`: byte-identical across processes
  (sha256 prefix `cb6dafd458d6ad04`, seed 0).
- Val loss decreases from epoch 1 to the last epoch (0.690 → 0.530).
- No Rust dependency: `trainer/` imports only stdlib + numpy + torch.

## Open items for Gate 3 (Rust loader)

1. **Feature layout:** implement `f = 64 * p + sq` (piece-major,
   square-minor) with the §2 square/piece indices and the other-view
   transform (color swap + file mirror) exactly as `trainer/features.py`;
   the fixture `trainer/fixtures/weights.v1.bin` plus the hand-computed
   index tests are the cross-check. (Done during the handoff: nn.md §2's
   formula was corrected from the impossible `f = 64 * sq + p` to
   `f = 64 * p + sq` in the source repo; the Gate-3 agent only verifies
   its copy — see `handoff.md` / `handoff-prompt.md`.)
2. Loader validation: magic/version/dims/flags hard errors (nn.md §10);
   reject `flags != 0` and any file size that disagrees with the header
   dims (the trainer's `read` implements exactly these checks).
3. ClippedReLU clamp max = 1.0 must be hard-coded in Rust (not in the
   file); changing it needs a weight-file version bump.
4. Promotion handling: multiple promotion variants of one `(from, to)` map
   to one policy index; the mask must deduplicate indices before masking
   (the trainer's `policy_index` ignores the promotion suffix).
5. Incremental accumulator (§4) requires `W_1` column-major access
   (`W_1[:, i]` = file offset `16 + 4*(128*feature)`); the row-major
   file layout supports this directly.
6. Sibling-score scale: trained scores live in a RankNet-margin space
   (differences meaningful, absolute values arbitrary); the loader must
   sort by score, never threshold it.
