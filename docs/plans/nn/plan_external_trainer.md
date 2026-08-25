# Plan: external trainer (Gate 2 proposal)

## Status

Rough proposal. This plan is the Gate 2 of `docs/plans/nn/concept.md`: the
external (Python/PyTorch) training toolchain that consumes the Gate-1 NDJSON
corpus and emits the float32 weight file whose byte layout is pinned in
`docs/spec/nn.md` §10. Gate 1 (corpus generation) is implemented and verified
(`docs/plans/nn/report2.md`).

The corpus has since moved to design B (`docs/plans/nn/plan4.md` +
`report4.md`): every `children[]` entry carries the recorded real `work`
(cumulative `child_evals` spent proving that child's subtree) and the corpus
version is `atomic-corpus/2`. The AND label is therefore **"rank the children
by `work`"** (descending), not by `subtree_size`. The trainer and its
development run in a Docker container; the setup handoff (which files to copy
over, what to preinstall) is `docs/plans/nn/trainer_init.md`.

The two open gaps from report2's "next step" are now pinned:

- **`policy_size` = 4096** (nn.md §5): `from_sq * 64 + to_sq`, promotions
  collapse onto the same `(from, to)` index.
- **Weight-file layout** = nn.md §10: 16-byte header + six float32 tensors,
  967,312 bytes total.

This plan pins nothing else new about the network; it proposes a minimal,
auditable trainer that produces exactly that file. The Rust loader (Gate 3)
is a separate future plan; this plan's final deliverable includes a sample
weight file that Gate 3 will use as its round-trip fixture.

## Goal

Add a self-contained external training pipeline under `trainer/` that:

1. Loads the Gate-1 NDJSON corpus (meta first line + one row per line).
2. Builds the 768-dim two-perspective sparse features per `docs/spec/nn.md`
   §2 for every row (from `fen` + `stm`), with the exact index layout and
   the other-side transform pinned there.
3. Produces one ranking dataset: for each row, the label pairs described in
   concept.md §5 and report2 ("Input contract for the Gate 2 trainer"),
   with censored children excluded from the loss.
4. Trains the network from `docs/spec/nn.md` §3 (768 → 128×2 → 32 → 4096,
   shared `W_1`, ClippedReLU) with a RankNet-style pairwise loss (§6).
5. Writes the trained tensors as the §10 weight file, plus a tiny
   deterministic sample weight file for the future Rust loader test.

Out of scope (future plans): the Rust weight loader and Gate 3/4 integration
and measurement. The trainer must not depend on the Rust toolchain.

## Background

- The corpus schema and the trainer input contract are documented in
  `docs/plans/nn/report2.md` ("NDJSON schema (v1)" and "Input contract for
  the Gate 2 trainer"). The NDJSON is the only required input; the `.bin`
  dumps and manifest are not needed by the trainer.
- Label semantics (concept.md §5):
  - **OR rows** (`outcome == "win"`): the proven decisive child(ren) — the
    `children[]` with `outcome == "loss"` — must rank above every other
    legal move. `first_decisive_rank` is a summary, not the training target;
    the target is the pairwise order.
  - **AND rows** (`outcome == "loss"`): every child is expanded, so rank the
    `children[]` by recorded `work` descending (design B,
    `docs/plans/nn/plan4.md`; `report4.md` rejects `subtree_size` as the
    label).
  - **Censoring**: legal moves that are not in `children[]` were never
    expanded (OR-node early stop). They are censored in the sense that no
    pairs are formed *among them* and they are never treated as "cheap" —
    but the OR one-vs-rest target explicitly ranks each proven child above
    every one of them (accepting the multiple-winning-move noise of
    concept.md §3).
- The network architecture and feature contract are in `docs/spec/nn.md`:
  §2 (features + exact index layout), §3 (layers/weight shapes), §4
  (incremental accumulator — trainer-side irrelevant but the `W_1` layout
  must allow it), §5 (output/masking), §6 (loss), §7 (sizing), §10 (weight
  file).
- The NDJSON was produced by Rust's `serde_json` (compact, one object per
  line, object keys sorted). Python parses it with stdlib `json`. The
  `hash` field is a JSON decimal integer; decode with Python's exact `int`
  semantics (never float) per report2's input contract.

## Decisions (pinned here)

- **Toolchain**: Python 3.12 + PyTorch (>=2.0). Numpy only where the weight
  writer needs to avoid torch tensor host/device concerns. No other
  dependencies beyond numpy/torch.
- **Feature extraction**: a standalone module (`features.py`) with a pure
  Python (numpy) implementation of the §2 layout and the §2 other-side
  transform (color swap + file mirror), with unit tests against hand-built
  positions. This module is the single source of truth that both the trainer
  and (later) the Rust loader's review will refer to.
- **Ranking data**: for OR rows, generate one pair per (decisive, other
  legal) combination, weighted by... (implementer choice: unweighted pairs
  are fine for v1; a pair-weighting variant using `abs(log2(work_i /
  work_j))` is a cheap ablation). For AND rows, one pair per `(children[i],
  children[j])` with `work_i > work_j`.
- **`partial` rows**: dropped by default for the "trusted-label" run (a
  `--keep-partial` flag re-enables them). The report must state which mode
  was used.
- **Split**: a `--validation-fraction` (default 0.1) of rows held out from
  training, chosen per `source` (case-level) to keep transpositions
  together. The move-order suite is NOT in the corpus and therefore cannot
  leak; the trainer never sees it.
- **Regularization**: the net is small by design (nn.md §7); dropout on the
  256-dim activation plus `L2` on `W_2`/`W_3` is enough. The success target
  is validation loss not overfit.
- **Output**: the §10 weight file, written by a small `weights.py` module
  (struct-based, tested for byte-exact round-trip against a reference numpy
  reader). Plus `<out>.json` with the training summary (epochs, final loss,
  per-row count, seed).

## Scope

In scope:

- `trainer/` python package: `features.py`, `corpus.py`, `model.py`,
  `loss.py`, `weights.py`, `train.py` (CLI), `test_*.py` unit tests.
- `docs/plans/nn/report_external_trainer.md` (final report; the plan's final
  task).
- `AGENTS.md` "Examples" section: a brief "Trainer (Gate 2)" note (the
  trainer is not a Rust example; put it near the examples list or in the
  dependencies section).
- `docs/plans/nn/trainer_init.md` already exists and pins the Docker
  handoff (files to copy over, preinstalled packages, runtime mounts); this
  plan implements inside that container and adds no dependency beyond the
  pinned Python toolchain.
- A sample weight file fixture: `trainer/fixtures/weights.v1.bin` written by
  the trainer with a fixed seed (used by the future Gate 3 Rust loader
  test).

Out of scope:

- Any change to the Rust solver, `src/`, the `.bin` dump format, or the
  corpus schema.
- The Rust weight loader and Gate 3/4 integration (separate plans).
- Quantization, int8, or anything beyond float32.
- The `child_evals` counter question is settled by design B
  (`docs/plans/nn/plan4.md`/`report4.md`): recorded `work` is now a corpus
  field, so no ablation remains.

## Design

### 1. Data model

`corpus.py` parses NDJSON. Per row:

- `hash` as Python `int` (JSON numbers are arbitrary-precision decimals;
  convert via `int(line_field)`; never float).
- `fen` (string), `stm` ("w"/"b").
- `outcome` ("win"/"loss").
- `legal_moves` (list of UCI).
- `children`: list of `{mv, subtree_size, work, outcome}` (`work` is the
  label source for AND rows; `subtree_size` is kept for comparison).
- `partial` (bool).
- `source` (string), `depth`, `subtree_size`.

Rows are validated: `len(legal_moves) == len(static_scores)` (the scores map
is used only for the optional static-rank residual baseline, not for
training labels); every `children[].mv` must be in `legal_moves`; hashes
unique.

### 2. Label pairs

For each kept row:

- `outcome == "win"` (OR): `decisive = [c for c in children if c.outcome ==
  "loss"]`; one-vs-rest pairs: `(decisive.mv, other.mv)` for every legal
  move NOT in `decisive`. No pairs are formed between two non-decisive
  moves (they were never expanded — no ordering signal among them).
- `outcome == "loss"` (AND): sort `children` by `work` descending (ties: any
  stable order); pairs `(higher.mv, lower.mv)` for each `(i, j)` with
  `children[i].work > children[j].work`. All legal moves not in `children`
  are censored.
- `partial` rows are dropped unless `--keep-partial`.

### 3. Features

`features.py`: `features_for(fen: str, stm: str) -> tuple[torch.Tensor,
torch.Tensor]` returning two 768-dim binary tensors (the STM view and the
other view per §2), using the §2 index formula. Implement the FEN
parser inside `features.py` (standard piece letters, case = color; kings are
`k`/`K` per `docs/spec/nn.md` §2 — the corpus uses standard notation only,
no `c`/`C` commoners; missing halfmove-clock fields default). Unit tests:
hand-built FENs (`"8/8/.../8/8/... w - - 0 1"` etc.), e.g. a lone white king on
a1 is only index `0*64 +
0*6+5 = 5` in view A and becomes... in view B, mirrored file `f=0 -> 7`, so
square index `sq = 7 + 0 = 7`, color swapped → `p = 6*1 + 5 = 11` → index
`7*64 + 11 = 459`. These tests exist to pin the transform.

### 4. Model

`model.py`: the §3 stack with the pinned shapes:

```
W1 = torch.nn.Linear(768, 128, bias=True)   # shared, applied to both views
a_stm  = W1(x_stm)
a_other = W1(x_other)
a = concat(a_stm, a_other)                   # 256
a = ClippedReLU(a, max=1.0)                 # clamp
h = ClippedReLU(W2(a) + b2)                 # 32
s = W3(h) + b3                              # 4096
```

The shared-`W_1` requirement is realized by applying the same `W1` linear
to both `x_stm` and `x_other` (two forward calls on the same module — no
tied-parameter hack needed).

### 5. Loss

`loss.py`: pairwise RankNet-style BCE over the pairs of §2:

```
loss = mean over pairs (i, j) of:
    log(1 + exp(-(s_i - s_j) * T))       # T = 1.0 v1
```

Optionally weighted by `abs(log2(work_i / work_j))`; v1 keeps it unweighted.

### 6. Weight file I/O

`weights.py`:

- `write(path, W1, b1, W2, b2, W3, b3)`: pack the §10 header (magic
  `0x4E4E5441`, version 1, dims 768/128/32/4096, flags 0, reserved 0), then
  each tensor row-major as float32 little-endian via `struct.pack`/`array`.
- `read(path)` returning dims + numpy arrays; `roundtrip_test` verifies
  byte-identical write→read for a tiny tensor set.
- Also `write_sample(path, seed)`: a small fixed-seed weight file (e.g. all
  zeros + a few known entries) for the future Rust loader test.

### 7. CLI

```
python3 -m trainer.train --corpus <ndjson> --out <weights.bin>
    [--epochs N] [--lr F] [--batch N] [--seed N]
    [--keep-partial] [--source-fraction F] [--sample-out <path>]
    [--max-rows N]
```

The trainer prints per-epoch train/val loss to stdout and writes the weight
file + a `<out>.json` summary at the end. The corpus file may be the full
~26k-row corpus; a `--max-rows` flag caps it for quick runs.

### 8. Tests

`trainer/test_features.py`, `test_corpus.py`, `test_loss.py`,
`test_weights.py` via plain `pytest`:

- `features.py`: the hand-built FEN cases from §3; also a full-corpus
  smoke test: load the real `data/corpus/train.ndjson` and assert row
  counts, label-pair counts, unique hashes.
- `weights.py`: round-trip byte-exact.
- `train.py` on a tiny synthetic NDJSON (5 rows): loss decreases, weight
  file is written with the correct dims and size.

## Implementation steps

1. Create `trainer/` package: `features.py`, `corpus.py`, `model.py`,
   `loss.py`, `weights.py`, `train.py`.
2. Add the unit tests.
3. `pytest trainer/` on a small synthetic corpus and on the real
   `data/corpus/train.ndjson` (reduced epochs).
4. Run a real training pass (small net, ~1-2 epochs over the current ~26k-row
   corpus on CPU or small GPU); produce `weights.v1.bin` + the sample fixture.
5. Update `AGENTS.md` (trainer note).
6. Write `docs/plans/nn/report_external_trainer.md`.

## Files changed

- `trainer/` (new, python)
- `docs/plans/nn/report_external_trainer.md` (new, final report)
- `AGENTS.md` (note)
- `data/corpus/weights.v1.bin` (generated artifact, git-ignored by `/data/`)

## Verification

- `pytest trainer/` passes.
- A real run on the current corpus: final validation RankNet loss decreases
  from epoch 1 to last; the emitted weight file has the exact §10 size and
  reads back with `weights.read`.
- The sample fixture is byte-stable across runs (fixed seed).
- The trainer does NOT import any Rust code and runs from a plain Python
  environment.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Feature-index mismatch between trainer and future Rust loader | The §2 layout is pinned in the spec; the sample fixture + the layout tests in `features.py` are the canonical cross-check. |
| Train overfits (~26k rows vs ~242k weights) | Tiny net, dropout, small LR, validation by case-source split; treat the output as a feasibility check, not a full model. |
| `partial` rows dominate | Dropped by default; `--keep-partial` is an explicit escape hatch. |
| Censored children leak negative signal | Pairwise loss excludes censored moves from both sides of the comparison. |
| u64 `hash` JSON decoding loses precision | Parse with `int()`, never float; corpus tests assert uniqueness with exact int semantics. |
| Promotions collapse onto one index | Pinned in §5; only affects the mask, which the trainer computes from `legal_moves` (multiple promotions → same index, still all legal). |

## Success criteria

1. `trainer/` runs against the real corpus NDJSON (`data/corpus/train.ndjson`,
   `atomic-corpus/2`).
2. A weight file is produced in the §10 layout; `weights.read` round-trips
   it byte-exactly.
3. A deterministic sample fixture exists for the future Gate 3 Rust loader
   test.
4. `docs/plans/nn/report_external_trainer.md` records the training run, the
   validation loss, the emitted file's dims/size, and the exact input
   contract the trainer consumed (schema version, partial handling, pair
   semantics) so the Rust loader plan can rely on the file.

## Final task

Write `docs/plans/nn/report_external_trainer.md` covering:

- what was built (files, CLI, dependencies),
- the exact feature extraction (index layout + transform) as implemented,
- the label-pair generation (OR/AND, censoring, `partial` handling),
- the training run and its metrics,
- the emitted weight file: byte layout, dims, size, and the sample fixture,
- the input contract the trainer consumed (NDJSON schema + `hash`
  precision caveat),
- open items for Gate 3 (loader validation checks, promotion handling).