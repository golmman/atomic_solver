# Trainer init: external trainer setup (Gate 2)

## Status

This is the setup/handoff document for the external (Python/PyTorch) trainer of
the move-ordering network. The trainer and its development run in a Docker
container, fully decoupled from the Rust repository: the container receives a
handful of contract documents and the NDJSON corpus, and produces the
`docs/spec/nn.md` §10 weight file, a training-summary JSON, and a deterministic
sample fixture (plan: `docs/plans/nn/plan_external_trainer.md`).

Everything below is pointers plus the gotchas that must survive the handoff;
the authoritative details live in the listed files, not here.

## What the trainer needs

### Inputs (only one data file)

- `data/corpus/train.ndjson` — the corpus, `atomic-corpus/2`, ~20 MB: 26,475
  rows (18,991 win / 7,484 loss), 59 cases, generated with `--suite quick
--timeout 20 --tt-size 64 --pt-size 256`. The NDJSON is the **only required
  input**.
  - The file is git-ignored (`/data/` in `.gitignore`), so it cannot be
    cloned; copy it or mount the directory. Regeneration happens on the Rust
    side (`make nn_corpus`) and is out of scope for the container.
  - The first line is the `_meta` object (`{"_meta": "atomic-corpus/2",
"rows": …, "cases": …}`). The trainer should assert the version it was
    built for; the meta values are authoritative over anything in this file.

### Files to copy over (specs, plans, docs)

The container needs only the **normative contract** — the plan and the spec —
plus the corpus. Everything else in the repo is rationale or history; copy the
reports only if the trainer developer wants the full background.

Required — the trainer contract:

| File                                     | Why                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/spec/nn.md`                        | Network contract: §2 feature index layout, the FEN convention (standard `k`/`K` notation only since atomic-movegen 2.1.0), and the other-side transform; §3 layer/weight shapes; §5 output mask (`policy_size` = 4096); §6 loss and OR/AND label semantics; §10 weight-file byte layout. The trainer must implement these **identically** to the future Rust loader. |
| `docs/plans/nn/plan_external_trainer.md` | The Gate 2 implementation plan and the input contract: `trainer/` package layout (`features.py`, `corpus.py`, `model.py`, `loss.py`, `weights.py`, `train.py` + tests), NDJSON parsing caveats, label-pair rules, pinned decisions, CLI, success criteria. Self-contained: nothing below is required to implement it.                                                |

Data:

| Path                       | Why                             |
| -------------------------- | ------------------------------- |
| `data/corpus/train.ndjson` | The corpus; see "Inputs" above. |

Recommended context (not required to implement):

| File                       | Why                                                                                                              |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `docs/plans/nn/report2.md` | The full NDJSON schema table and the "Input contract for the Gate 2 trainer" (the plan points to it for detail). |
| `docs/plans/nn/report4.md` | Why `work` is the AND label (the `subtree_size` proxy was measured and rejected).                                |
| `docs/plans/nn/concept.md` | The _why_ of the whole pipeline and the honest risk assessment.                                                  |

Not needed in the container: the Rust sources, `Cargo.toml`, examples,
fixtures, the `.bin` dumps (they need the Rust `corpus_gen` binary to
replay), the Rust toolchain, or this `trainer_init.md` itself — it is a
host-side setup guide, read here in the repo by whoever builds the image.

### Suggested working-copy layout

```
trainer/
  docs/                  # the plan + spec (+ optional reports), same layout
  data/corpus/train.ndjson
```

## Docker container prerequisites

### What to preinstall

- **Base image:** `python:3.12-slim` is enough. The net is tiny (~242k
  weights) and the plan explicitly runs the v1 training on CPU; a
  `pytorch/pytorch` GPU image only if a GPU is available.
- **Python packages** (pinned in a `requirements.txt` so the sample fixture
  and the weight file are byte-stable across runs):
  - `torch>=2.0` (plan decision: Python 3.12 + PyTorch ≥ 2.0),
  - `numpy`,
  - `pytest` (trainer test suite),
  - nothing else — the plan pins "No other dependencies beyond numpy/torch".
- **System tools:** `git` (to keep the copied bundle in sync with the repo).

Example:

```dockerfile
FROM python:3.12-slim
RUN apt-get update && apt-get install -y --no-install-recommends git \
    && rm -rf /var/lib/apt/lists/*
COPY requirements.txt /trainer/requirements.txt
RUN pip install -r /trainer/requirements.txt
WORKDIR /trainer
```

### Explicitly out of scope for the image

- The Rust toolchain and the `atomic_solver` build (the trainer must not
  depend on Rust).
- The corpus itself: mount/copy it at run time rather than baking it into the
  image (it is a large, generated, git-ignored artifact).

### Runtime mounts

- Read-only: `<repo>/data/corpus/train.ndjson` → `/trainer/data/corpus/`.
- Writable: `<repo>/data/corpus/` for `weights.v1.bin` and `<out>.json`
  (git-ignored), and `<repo>/trainer/fixtures/` for the committed sample
  fixture.

## Gotchas worth a sanity check (all pinned in the bundled docs)

This is a host-side checklist of the facts a developer is most likely to miss
even though the bundled plan/spec state them. Each points at its normative
home; if a fact is missing there, the bundle is stale.

- **Corpus FENs use standard `k`/`K` notation** (atomic-movegen ≥ 2.1.0;
  verified on `atomic-corpus/2` — all rows, zero `c`/`C`). Corpora generated
  with the pre-2.1.0 `c`/`C` spelling are obsolete and rejected by the crate;
  regenerate with `make nn_corpus`. `docs/spec/nn.md` §2 pins the convention.
- `hash` is a decimal u64 JSON number; parse with Python's exact `int`
  semantics, never `float` (plan Background).
- `children[].work` is the AND label: rank children by `work` **descending**
  (cheapest first); `work >= 1` for every expanded child; zero/absent `work`
  means censored — excluded from loss pairs, never "cheap" (nn.md §6, plan §2).
- `partial: true` rows stem from timed-out/synthesized cases; dropped by
  default (`--keep-partial` re-enables; plan §2).
- Other-side transform is pinned in nn.md §2: swap colors + mirror the file
  (`f → 7 − f`), rank unchanged; both views share `W_1`.
- `static_scores` object keys are JSON-sorted; align via `legal_moves[i]`,
  never key order (plan Background).
- NDJSON is compact, one object per line after the `_meta` line; a JSON
  object's internal field order must not be assumed (plan Background).

## Outputs / hand-back

The trainer writes (per `docs/plans/nn/plan_external_trainer.md`):

- the §10 weight file — 16-byte header + six float32 tensors, 967,312 bytes
  total,
- `<out>.json` training summary (epochs, final loss, row counts, seed),
- `trainer/fixtures/weights.v1.bin` — deterministic sample fixture for the
  future Rust loader test (fixed seed, byte-stable across runs).

Gate 3 (Rust loader) will consume the weight file; `report_external_trainer.md`
must record the exact input contract the container consumed (corpus version,
partial handling, pair semantics) alongside the run metrics.
