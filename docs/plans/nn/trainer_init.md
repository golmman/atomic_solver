# Trainer init: external trainer setup (Gate 2)

## Status

This is the setup/handoff document for the external (Python/PyTorch) trainer of
the move-ordering network. It is read **here, in this repo**, by whoever
prepares the execution environment. **This repo is not the execution target**:
nothing here is installed, configured, mounted, or run — there is no `uv.toml`,
no `trainer/` project, and no venv in this repo. The handoff below is a
self-contained recipe for the environment where the trainer actually runs.

The trainer and its development run in a Docker container that **rw-mounts the
trainer's own repository** (a separate checkout that holds the contract files
and the corpus). The container is disposable and resumable: all durable state —
code, the venv, the dependency cache, the lock file — lives in the mounted
trainer repository, so the container can be stopped, restarted, or recreated at
any time without losing work.

The trainer is decoupled from the Rust toolchain: the container never builds or
runs Rust. It produces the `docs/spec/nn.md` §10 weight file, a
training-summary JSON, and a deterministic sample fixture (plan:
`docs/plans/nn/plan_external_trainer.md`).

Everything below is pointers plus the gotchas that must survive the handoff;
the authoritative details live in the listed files, not here.

## What the trainer needs

### Inputs (only one data file)

- `data/corpus/train.ndjson` — the corpus, `atomic-corpus/2`, ~20 MB: 26,475
  rows (18,991 win / 7,484 loss), 59 cases, generated with `--suite quick
--timeout 20 --tt-size 64 --pt-size 256`. The NDJSON is the **only required
  input**.
  - The file is git-ignored in this repo (`/data/` in `.gitignore`), so it
    cannot be cloned; copy it into the trainer repository (or mount it at run
    time). Regeneration happens on the Rust side (`make nn_corpus`) and is out
    of scope for the container.
  - The first line is the `_meta` object (`{"_meta": "atomic-corpus/2",
"rows": …, "cases": …}`). The trainer should assert the version it was built
    for; the meta values are authoritative over anything in this file.

### Files to provision into the trainer repository

Because the execution target is the trainer's own repository (not this one),
whoever prepares that repository copies the **normative contract** over from
here. Nothing else in this repo is required:

| File                                     | Why                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/spec/nn.md`                        | Network contract: §2 feature index layout, the FEN convention (standard `k`/`K` notation only since atomic-movegen 2.1.0), and the other-side transform; §3 layer/weight shapes; §5 output mask (`policy_size` = 4096); §6 loss and OR/AND label semantics; §10 weight-file byte layout. The trainer must implement these **identically** to the future Rust loader. |
| `docs/plans/nn/plan_external_trainer.md` | The Gate 2 implementation plan and the input contract: `trainer/` package layout (`features.py`, `corpus.py`, `model.py`, `loss.py`, `weights.py`, `train.py` + tests), NDJSON parsing caveats, label-pair rules, pinned decisions, CLI, success criteria. Self-contained: nothing below is required to implement it.                                                |
| `data/corpus/train.ndjson`               | The corpus; see "Inputs" above.                                                                                                                                                                                                                                                                                                                                      |

Recommended context (not required to implement):

| File                       | Why                                                                                                              |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `docs/plans/nn/report2.md` | The full NDJSON schema table and the "Input contract for the Gate 2 trainer" (the plan points to it for detail). |
| `docs/plans/nn/report4.md` | Why `work` is the AND label (the `subtree_size` proxy was measured and rejected).                                |
| `docs/plans/nn/concept.md` | The _why_ of the whole pipeline and the honest risk assessment.                                                  |

Not needed in the container: the Rust sources, `Cargo.toml`, examples,
fixtures, the `.bin` dumps (they need the Rust `corpus_gen` binary to replay),
the Rust toolchain, or this `trainer_init.md` itself — it is a host-side setup
guide, read here in the repo by whoever builds the image.

### Suggested working-copy layout

```
trainer-repo/              # the trainer's own repo, rw-mounted into the container
  docs/                    # the plan + spec (+ optional reports), same layout
  data/corpus/train.ndjson
  pyproject.toml           # uv project; committed (created by the plan's bootstrap)
  uv.lock                  # pinned resolution; committed
  uv.toml                  # uv config (cache-dir, CPU torch index); committed
  trainer/                 # the python package per the plan
  .venv/                   # created by `uv sync`, git-ignored
  .uv-cache/               # created by uv, git-ignored
```

### Repo-local uv state (durable, in the mount)

All of the following is **created in the trainer repository by the plan's
bootstrap** (`docs/plans/nn/plan_external_trainer.md`, Decisions "Project
scaffolding" + Implementation steps) and is **not** present in this repo:

- `pyproject.toml` (committed) — project metadata + dependencies only; it
  deliberately has **no `[tool.uv]` table**.
- `uv.toml` (repo root, committed) — the single place for uv configuration:
  `cache-dir = ".uv-cache"` (so container and non-container runs share one
  repo-local cache) plus the CPU-only torch index.
- `uv.lock` (committed) — the pinned, hash-locked resolution; the lock file is
  what makes the weight fixture byte-stable across runs (the same promise a
  pinned `requirements.txt` used to make).
- `.venv/` (git-ignored) — the virtual environment created by `uv sync`; lives
  in the mount and survives container restarts.
- `.uv-cache/` (git-ignored) — uv's wheel/metadata cache, repo-local for the
  same reason. Also add `.venv/`, `.uv-cache/`, `__pycache__/`, `*.pyc` to the
  trainer repo's `.gitignore`.

## Docker container

### Image

- **Base image:** `python:3.12-slim` is enough. The net is tiny (~242k
  weights) and the plan explicitly runs the v1 training on CPU; a
  `pytorch/pytorch` GPU image only if a GPU is available.
- **Preinstall:** `uv` and `git` only. In particular **no**
  `torch`/`numpy`/`pytest` baked into the image: they are installed into the
  mounted trainer repo on first boot (`uv sync`), where they stay. The image
  stays small, stays offline-capable at runtime, and has nothing to drift from
  the lock file.
- The Rust toolchain is never installed.

Example Dockerfile:

```dockerfile
FROM python:3.12-slim
COPY --from=ghcr.io/astral-sh/uv:latest /uv /usr/local/bin/uv
RUN apt-get update && apt-get install -y --no-install-recommends git \
    && rm -rf /var/lib/apt/lists/*
```

### Launching the container

Run as a non-root user matching the host UID so files written into the mount
are not root-owned on the host:

```sh
docker run -it --rm --user "$(id -u):$(id -g)" \
    -v "$PWD:/repo" -w /repo <image> bash
```

### Bootstrapping the trainer repository

Creating the project (`pyproject.toml`, `uv.lock`, `uv.toml`, `.gitignore`
rules) is **defined in `docs/plans/nn/plan_external_trainer.md`** —
Decisions "Project scaffolding" (exact `uv.toml` contents, the CPU-torch
index, the no-`[tool.uv]` rule) and Implementation step 1. Run those steps
first; this handoff covers the container, not the project files.

Once the source is set up, `uv sync` creates `.venv/` (inside the mount) and
installs into it. The first run downloads wheels into the repo-local cache;
afterwards the container is network-independent:

```sh
uv sync --locked --offline
uv run pytest
uv run python -m trainer.train --corpus data/corpus/train.ndjson --out data/corpus/weights.v1.bin
```

### Cache location and the CWD rule

`uv.toml` at the repo root sets `cache-dir = ".uv-cache"`. **uv resolves
relative paths against the current working directory of the invocation, not
against the config file's directory** (verified against uv 0.12). Therefore:

- run all uv commands from the **same directory** — the repo root is the
  canonical one (it holds `pyproject.toml`), so the `bash` shell in the
  `docker run` example above already complies;
- the cache then lives at `<repo>/.uv-cache`. Both the cache and `.venv` are
  inside the mount and therefore durable; mixing invocation directories
  fragments the cache and breaks the `--offline` shortcut, so don't;
- `uv cache clean` / `uv cache prune` operate on the cache at the *current*
  invocation directory; run them from the repo root.

If you ever want an absolute, CWD-independent override, the `UV_CACHE_DIR`
environment variable takes precedence over the config file.

### Container lifecycle

- `docker pause`/`unpause` and `docker stop`/`start` (same container):
  everything persists, including the container's writable layer.
- `docker rm` / `docker compose down -v` / CI autoremove: the container layer
  is gone forever — but this costs nothing, because the durable state is in
  the mount. Packages installed into the container's own filesystem would die
  here; do not install into the system Python. After losing the container,
  `uv sync --locked` rebuilds `.venv` from the lock file, and the warm
  `.uv-cache` avoids re-downloads.

### Runtime mounts

One read-write mount of the trainer repository root (e.g. `/repo`). The corpus
is either copied into the repo (see "Files to provision") or mounted read-only
on top of `data/corpus/` at run time:

- Read: `data/corpus/train.ndjson`.
- Write: `data/corpus/weights.v1.bin` and `<out>.json` (git-ignored), and
  `trainer/fixtures/weights.v1.bin` (committed sample fixture).

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
- **uv config precedence (verified):** a same-directory `uv.toml` shadows a
  `[tool.uv]` table in `pyproject.toml`, and `[tool.uv.sources]` is **not
  allowed** in `uv.toml`. The plan therefore keeps `pyproject.toml` free of
  `[tool.uv]` and puts `cache-dir` + the CPU-torch index in the repo-root
  `uv.toml`. Keep that split exactly; do not "helpfully" move config around.
- **CPU-only torch:** the plan pins the CPU index via index priority in
  `uv.toml` (`[[index]] url = "https://download.pytorch.org/whl/cpu"`),
  verified to resolve `2.13.0+cpu`; this avoids the default multi-GB
  CUDA-bundled PyPI wheel on a CPU-only run. A GPU machine changes the URL to
  the `cu126` index. (uv's `torch-backend` setting affects only `uv pip`
  commands, not `uv sync`.)
- **Aggressive host cleans:** `git clean -fdx` from the host removes `.venv/`
  and `.uv-cache/`; harmless because they are recreatable from `uv.lock`
  (re-download needed afterwards).

## Outputs / hand-back

The trainer writes (per `docs/plans/nn/plan_external_trainer.md`):

- the §10 weight file — 16-byte header + six float32 tensors, 967,312 bytes
  total,
- `<out>.json` training summary (epochs, final loss, row counts, seed),
- `trainer/fixtures/weights.v1.bin` — deterministic sample fixture for the
  future Rust loader test (fixed seed, byte-stable across runs; the `uv.lock`
  pin is what keeps the environment byte-stable).

Gate 3 (Rust loader) will consume the weight file; `report_external_trainer.md`
must record the exact input contract the container consumed (corpus version,
partial handling, pair semantics) alongside the run metrics.