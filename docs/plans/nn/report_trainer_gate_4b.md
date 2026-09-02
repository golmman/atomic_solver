# Report: Gate 4b — trainer-side v2 recipe (residual logits + work weighting)

Task: `docs/plans/nn/handoff_agent_train_plan7.md`. Plan:
`docs/plans/nn/plan7.md` (pinned decisions 2 and 3). Spec: `nn.md` §6a.
Status: **implemented, trained, verified**. Architecture, feature layout,
and the §10 byte format are unchanged; only the loss/recipe and run
planning changed. The ordering-quality measurement itself is the Rust-side
Gate-4 harness (plan7 step 5) and is out of scope here.

## What changed

| File | Change |
|---|---|
| `trainer/corpus.py` | `normalized_static_prior` (§6a ŝ), `Pair` gains `prior_i`/`prior_j`, `label_pairs(..., recipe="v1"|"v2")` — v2 fills the work weight and ŝ per pair; pair *construction* identical between recipes |
| `trainer/loss.py` | `pair_weight_v2` (`1 + log2(1 + w)`), `residual_ranknet_loss` (`z_ij = (s_i − s_j) + λ·(ŝ_i − ŝ_j)`) |
| `trainer/train.py` | `--recipe {v1,v2}` (default v1), `--lambda F` (default 1.0, v2 only); m20–m22 training-split assertion; summary gains `recipe`/`lambda`/`weighting`/`row_counts` |
| `trainer/test_*.py` | 14 new tests (63 total): ŝ normalization, residual logits (prior- vs margin-dominated), hand-weight values (1→2.0, 15→5.0, 255→9.0), v1 byte-reproducibility regression, v2 end-to-end smoke, refreshed real-corpus smoke numbers |

v1 code path untouched: `label_pairs` default and `--recipe v1` produce
field-identical pairs, and an explicit-v1 run reproduces the default run's
loss history and weight bytes (regression test).

## Input contract consumed

- Corpus `data/corpus/train.ndjson`, `_meta` = `atomic-corpus/2`,
  `suite quick`, `timeout 420`, `tt_size 64`, `pt_size 1024` — the plan7
  step-3 budgeted deep regeneration, verified, not assumed.
- **Row-count note:** the handoff text says 24,109 rows; the `_meta` line
  and the actual file both say **24,091** (`partial_rows` 14,539). The
  handoff itself declares `_meta` authoritative, so 24,091 is used; the
  clean count 9,552 matches the handoff exactly.
- `hash` parsed with exact `int` semantics (never float), uniqueness
  enforced at parse; `static_scores` aligned by UCI key against
  `legal_moves` (never key order); standard `k`/`K` FENs; the other-side
  transform is color swap + file mirror (`trainer/features.py`, unchanged).
- `partial` rows dropped by default (14,539 dropped → **9,552 kept**: 6,905
  win / 2,647 loss; 32 of 54 sources survive). No `--keep-partial` was used.
- Pair semantics unchanged from v1: OR one-vs-rest (censored negatives
  included), AND rank-by-work ascending, ties produce no pair, zero-work
  children never preferred. Every expanded child has `work >= 1`
  (min work in kept rows = 1).
- **λ = 1.0** (the §6a default; open parameter, `nn.md` §9).
- m20–m22 absent from the corpus by construction and asserted at run time
  (`HELD_OUT_MOVE_ORDER_SOURCES` check on the training split) plus a
  real-corpus test; the corpus contains no `m20*`/`m21*`/`m22*` source at all.

## Residual recipe as implemented

- `ŝ_k = static_k / max(1, max_l |static_l|)` per node
  (`trainer/corpus.py::normalized_static_prior`): the static top pick gets
  ŝ = ±1, an all-zero map normalizes to ŝ = 0 (the `max(1, …)` guard), and
  the worked example holds — `{capture: 1e8, quietA: 560, quietB: 0}` →
  `{1.0, 5.6e-6, 0}`, so capture-vs-quiet pairs are prior-dominated while
  quiet-vs-quiet pairs are decided by the network margins.
- Pair logit `z_ij = (s_i − s_j) + λ·(ŝ_i − ŝ_j)` (`residual_ranknet_loss`).
- Pair weight `1 + log2(1 + w)`: work of the *cheaper* child for AND pairs,
  of the *proven decisive* child for OR pairs (weight stays on the decisive
  side; censored negatives carry no work). Observed OR-weight range on the
  kept corpus: 2.0 – 23.5, mean 3.44 (max child work 5,839,372).

## Validation split (seed 0, fraction 0.1, case-level)

11 val sources, **993 val rows** vs 8,559 train rows (10.4%). Per-source
val row counts (sources are heavily skewed, so this matters):
dec03 (51), dec18 (31), dec28 (17), dec29 (38), dec32 (36), dec36 (59),
dec39 (129), dec44 (511), m24_black (115), m26_black (1), m27_black (5).
Same leakage caveat as the Gate-2 report: m23+ are corpus data by design;
the honestly held-out move-order cases remain m20–m22 (absent here).

Pairs: train 193,803 OR + 12,288 AND; val 18,848 OR + 1,298 AND.

## Training run

```
uv run python -m trainer.train --recipe v2 --lambda 1.0 \
    --corpus data/corpus/train.ndjson --out data/corpus/weights.v2.bin \
    --epochs 3 --seed 0 --lr 3e-4 --dropout 0.4 --l2 3e-3
```

Hyperparameters identical to the delivered Gate-2 run for comparability.
torch 2.13.0+cpu, Python 3.12.13, aarch64, ~9 s wall.

| Epoch | v2 train (weighted) | v2 val (weighted) | v1 train (unweighted) | v1 val (unweighted) |
|---|---|---|---|---|
| 1 | 1.364717 | 1.316154 | 0.628067 | 0.600748 |
| 2 | 0.826064 | 1.199025 | 0.336155 | 0.517352 |
| 3 | 0.665736 | 1.196350 | 0.244495 | 0.503875 |

- **v1 reference on the same corpus** (same hyperparameters/seed, run for
  this report): train 0.244 / val 0.504 — consistent with the published
  Gate-2 numbers (train 0.25 / val 0.53 on the older corpus), so the
  corpus regeneration did not disturb the v1 baseline.
- **Loss-scale caveat:** the v2 losses are work-weighted means, so they are
  *not* comparable to the v1 numbers above. For a like-for-like check, the
  final v2 weights were re-scored on the identical validation split under
  the unweighted v1 loss: **0.505** vs the v1 reference's **0.504** — the
  network's own margins rank siblings about as well as v1's. That is the
  expected shape for a residual net: it is trained to output only the
  *correction*, and its ordering quality is realized at inference through
  the §5 composition `static + s (scaled)`, not through the margins alone.
  The real verdict is the plan7 step-5 Rust benchmark (`benchmark
  --suite move-order`), not validation loss.
- Val loss bottoms at epoch 3 under this regularization (same shape as
  v1); no tuning pass was made — one bounded iteration, per plan7.

## Outputs

- `data/corpus/weights.v2.bin` — 967,312 bytes, §10 v1 layout
  (header `(1, 768, 128, 32, 4096, flags=0)`), `trainer.weights.read`
  round-trips it byte-identically. No format change, no version bump.
- `data/corpus/weights.v2.bin.json` — summary: `recipe: "residual-v2"`,
  `lambda: 1.0`, `weighting: "1 + log2(1 + w)"`, corpus `_meta` verbatim,
  `row_counts` (total 24,091 / after-dedup 24,091 / clean-kept 9,552 /
  partial-dropped 14,539), split with per-source val row counts,
  hyperparameters (incl. recipe and λ), seed, and the loss history.
- `data/corpus/weights.v1.regen.bin{,.json}` — the v1 reference run on the
  regenerated corpus (not a deliverable; kept for the comparison table
  above). The original Gate-2 `weights.v1.bin` was not touched.

## Fixture integrity

`trainer/fixtures/weights.v1.bin` byte-verified after all changes:

```
sha256 cb6dafd458d6ad044204f65f4faf378223527eee4ef09e707c9771d4946db2e0
```

matches the handoff. No v2 fixture was added; `write_sample` and the
fixture writer path are recipe-independent and untouched.

## Verification

- `uv run pytest` — **63 passed** (14 new; real-corpus smoke updated to the
  regenerated counts: 24,091/14,539/9,552 rows, 212,651 OR + 13,586 AND
  default-mode pairs, 553,233 + 42,680 with `--keep-partial`).
- Real-corpus v2 run executed (above); weight file byte-verified.
- m20–m22: asserted absent at run time and in the smoke test.
- uv discipline: run from repo root, `uv sync --locked` clean, torch
  2.13.0+cpu (CPU index unchanged), no new dependencies.

## Deviations

1. **24,091 vs the handoff's 24,109 rows** — `_meta` (and the file) say
   24,091; `_meta` is authoritative per the handoff itself. Clean count
   (9,552) and all other pinned facts match.
2. **AGENTS.md trainer section refreshed** (CLI line gains
   `--recipe`/`--lambda`, test count 49 → 63) — the only doc touched
   outside this report.
3. Nothing else. No fixture regeneration, no corpus modification, no
   architecture/format change, no Rust-side work.
