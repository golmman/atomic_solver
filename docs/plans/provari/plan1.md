# Provari Plan 1: reproducible head-to-head baseline and target corpus

Initiative: `provari`, backlog item 1. Drafted 2026-10-29. This is a
**measurement-only** plan: no product code, no search behavior change.

Prerequisite reading: [`initiative.md`](initiative.md), `AGENTS.md`, and the
oracle isolation conventions in
[`interop/plan1.md`](../interop/plan1.md). The `interop` initiative owns the
independent checker; this plan only needs provari's `prove` command and its
published `bench/high200.jsonl`.

## Why this plan exists

The initiative was opened from an ad hoc `/tmp` comparison: provari proves the
motivating FEN in ~0.33 s with its bundled model, while our solver needs
~39.6 s to find its first win. Before adopting a single mechanism we need:

1. a committed definition of the **target corpus**,
2. a reproducible driver and provenance record,
3. a baseline that separates provari's model/hand/no-eval variants, and
4. a shared-win set on which “faster than provari” can be measured.

Without this, later plans will argue from single-position anecdotes and
different budgets.

## Plan-session probe (2026-10-29, `/tmp`, not committed)

A local provari 0.1.0 source build at commit
`48686bc6c786d6ea70568f0b2ad4002cf0520206`, built with
`--features bundled` after `data/fetch.sh`, produced:

| Run on the core FEN | Wall | Nodes | First win |
| --- | ---: | ---: | ---: |
| bundled model (default) | 0.33 s | 169,485 | mate 13 |
| `--guide hand` | 0.49 s | 342,289 | mate 17 |
| `--guide none` | 0.91 s | 670,136 | mate 16 |

The shipped `atomic_solver --fen <core> --timeout 60 --first-outcome`
returned a win in 39.6 s (11,773,039 nodes, 305,744,115 child evals, first line
41 plies). No tablebases were used.

These numbers are the expected starting point, not a gate. If plan execution
cannot reproduce the order of magnitude, record the deviation before drafting
plan2.

## Deliverables

Under `docs/plans/provari/measurements/plan1/`:

- `corpus.json` — the committed corpus definition:
  - `core`: the motivating FEN, labelled `core`;
  - `sample20`: every 10th entry of provari's `bench/high200.jsonl`
    (line indices 1, 11, 21, …, 191, 1-based), copied as `{id, fen}` pairs;
  - the selection rule and source URL/commit in a `source` field.
- `run_baseline.py` — the driver (or an equivalent shell script, but Python is
  preferred for JSON handling). It must be deterministic given the corpus and
  commands, and must not write `__pycache__` (`sys.dont_write_bytecode =
  True`).
- `results.json` — parsed per-case rows (schema below), with the exact
  commands, versions, host, and budgets.
- `README.md` — provenance table: date, host, provari commit/tag, build
  features, network/model digests if fetched, `atomic_solver` commit,
  commands, and the `.gitignore` note for raw logs.
- `report1.md` — baseline report (final task; created by this plan).

Do **not** commit: the provari clone, binary, `network.nnue`, `model.json`,
raw stdout/stderr transcripts, or `__pycache__`.

## Oracle setup

Use the pinned commit from `interop` (confirm it is still the intended
comparison point; if a newer release is used, record why):

```sh
rm -rf /tmp/provari-oracle
git clone https://github.com/aiorla/provari /tmp/provari-oracle
git -C /tmp/provari-oracle checkout 48686bc6c786d6ea70568f0b2ad4002cf0520206
cd /tmp/provari-oracle
./data/fetch.sh                    # network.nnue + model.json, digest-checked
cargo build --release --features bundled
./target/release/provari --version
```

- License boundary: GPL-3.0-or-later. Do not copy code, do not add it to
  `Cargo.toml`, do not vendor the binary/network/model.
- If `data/fetch.sh` or the bundled build fails, rebuild without
  `--features bundled`, use `--guide hand` for the baseline, and record the
  blocker. Do not silently compare against a different build.
- No tablebases: do not pass `--tablebases`. The comparison is
  tablebase-free.

## Corpus

Fixed selection rule (do not substitute a “representative” subset after seeing
results):

- **core:** `r1b1kbnr/ppp3pp/n4q2/3pppN1/1P1PP3/8/P1P2PPP/RNBQKB1R w KQkq - 0 1`
- **sample20:** provari's `bench/high200.jsonl`, every 10th line starting at
  line 1 (20 rows). The file is pinned by the provari commit above.

Write the exact ids/fens into `corpus.json` during execution. A later plan may
add a corpus, but it must use a recorded selection rule; it must not edit
`sample20` in place.

## Measurement protocol

Run sequentially on an otherwise idle host, release builds, one thread.

### Provari

Primary configuration (bundled model):

```sh
/tmp/provari-oracle/target/release/provari prove "<FEN>" \
  --seconds 10 --memory 192 --guide model
```

Control on the core case only:

```sh
/tmp/provari-oracle/target/release/provari prove "<core FEN>" \
  --seconds 10 --memory 192 --guide hand
```

Do not pass `--shorten`; provari's default first-win behavior is the
comparison target. If the bundled model is unavailable, use `--guide hand`
for all provari rows and label that in `results.json`.

### Our solver

```sh
./target/release/atomic_solver \
  --fen "<FEN>" --timeout 10 --first-outcome --outcome-only --tt-size 192
```

`--first-outcome` is deliberate: it matches provari's first-win comparison and
excludes our optional PV-refinement tail. `--outcome-only` keeps stdout clean;
stderr chunk lines may be captured to a temporary file but are not committed.
Use the default preflight setting (product behavior).

### Repeats

- Core case: 3 runs per solver/configuration; report each run and the median.
- `sample20`: 1 run per solver; report every case, including timeouts and
  censored results. Do not drop non-wins from the table.
- For provari, `first_mate_ms` is the primary wall number; `ms` is the whole
  command and may include proof bookkeeping.
- For our solver, parse `outcome:`, `pv:`, `evals:`, and `pre_exit:` from
  output; wall time is measured by the driver, not read from a log line.

### Fairness notes

- Both sides get 10 s and 192 MB. Our `--tt-size 192` is a reservation label;
  provari's `--memory 192` is split between table and proofs. Record both
  labels, not just the numeric value.
- Provari's node is an expansion (plus simulation steps); our `nodes` counts
  DF-PN entries and our `child_evals` counts child evaluations. Store them in
  separate fields and never collapse them into one “nodes” comparison.
- If a case is terminal or trivially solved by preflight, record the outcome;
  do not exclude it.

## Results schema

`results.json` is a JSON object with `provenance` and `runs`:

```json
{
  "provenance": {
    "date": "2026-10-29",
    "host": "...",
    "provari": {"commit": "...", "version": "...", "features": ["bundled"], "guide": "model"},
    "solver": {"commit": "...", "command": "..."},
    "budgets": {"seconds": 10, "memory_mb": 192}
  },
  "runs": [
    {
      "case": "core",
      "solver": "provari",
      "config": "bundled-model",
      "wall_s": 0.329,
      "outcome": "win",
      "first_win_plies": 26,
      "provari_nodes": 169485,
      "exit": null,
      "command": "..."
    },
    {
      "case": "core",
      "solver": "atomic_solver",
      "config": "first-outcome",
      "wall_s": 39.6,
      "outcome": "win",
      "first_win_plies": 41,
      "child_evals": 305744115,
      "solver_nodes": 11773039,
      "exit": "Complete",
      "command": "..."
    }
  ]
}
```

A censored run keeps its row with `outcome: "draw"` (or `"none"`) and the
reason/exit, plus `censored: true`. Never convert a timeout into an omitted
row.

## Tasks

1. **Read and confirm.** Re-read this plan, `initiative.md`, and
   `interop/plan1.md`; confirm the pinned commit and the GPL/no-import
   boundary.
2. **Create the corpus and driver.** Add
   `measurements/plan1/{corpus.json,run_baseline.py,README.md}`. The driver
   must accept `--provari`, `--solver`, `--corpus`, `--out`, `--seconds`,
   `--memory`, and `--repeat`; it must record commands and parse JSON
   robustly (provari may print a final JSON line only).
3. **Build the oracle** per the setup section; record `--version`, commit,
   feature set, and whether the bundled model was fetched. Record the
   `sha256` of `network.nnue`/`model.json` in the provenance README but do not
   commit the files.
4. **Run the core case** (3 repeats, bundled model + hand control + our
   solver). If the bundled model is unavailable, use hand everywhere and
   record why. Separately, rerun provari on the core with
   `--proofs /tmp/core.pvr` and run `provari check /tmp/core.pvr`; record
   `valid:true` or the failure (do not use this run's wall time). Our
   solver's `--first-outcome` line is informational in this plan; proof-tree
   validation belongs to the search/proof plans.
5. **Run `sample20`** with the primary configuration on both sides. Keep raw
   transcripts out of git; write only parsed rows.
6. **Refresh the corpus check.** Confirm `sample20` still has 20 rows and the
   core FEN is byte-identical to this plan.
7. **Write `report1.md`.** Include: the table, per-case shared wins, coverage
   at 10 s, the core gap reproduction, deviations, and a recommendation for
   plan2 (win-only front-end skeleton vs an incremental move-class spike).
   Record whether the ad hoc order of magnitude reproduced.
8. **Update the initiative index row only if the initiative's status changes**
   (it should stay Active; no README edit is required for a normal plan
   completion, per `AGENTS.md`).

## Acceptance

- `corpus.json` contains the core FEN and exactly 20 `sample20` rows selected
  by the stated rule.
- `run_baseline.py` runs end-to-end from the committed corpus and writes a
  valid `results.json`.
- The core case reproduces the expected order of magnitude: provari proves
  within ~1 s (bundled or hand), and our current `--first-outcome` solver
  needs tens of seconds or times out at 10 s. If it does not, the report
  records the deviation and plan2 is **not** drafted from the old story.
- Every `sample20` row is present, including censored rows; shared-win and
  coverage counts are computed from the rows, not from memory.
- `git status --short` shows only docs/plans/provari files; no provari binary,
  network/model, raw log, or `__pycache__` is staged.
- No `src/`, `examples/`, `tests/`, or `Cargo.*` changes. If any were made by
  accident, revert them and run `make test`.

## Validation

This plan changes no Rust code, so `make test` is not required. Run:

```sh
python3 - <<'PY'
import ast
from pathlib import Path
ast.parse(Path("docs/plans/provari/measurements/plan1/run_baseline.py").read_text())
PY
```

and the driver once in `--help`/dry-run mode. (Do not use `py_compile`;
it writes `__pycache__`.) If the driver is a shell script
instead, run `sh -n` and a dry run. Confirm `results.json` parses with the
standard library.

## Risks and mitigations

- **Provari network/model fetch fails.** Build without `bundled`, use
  `--guide hand`, label the guide in every row, and note the missing model in
  the report. Do not substitute an untrusted binary.
- **Wall-clock noise.** Three core repeats; report all values and the median.
  Sample rows are single-run by design and are used for coverage, not precise
  ratios.
- **High200 rows are too easy or too hard.** The sample is fixed by rule; do
  not re-pick after seeing results. If it yields few shared wins, that is a
  finding for plan2/plan7, not a reason to edit `sample20`.
- **Memory mismatch.** Record both `--memory 192` and `--tt-size 192`; note
  that provari splits memory and our label is a reservation. If a later plan
  needs exact parity, it must define the split and measure it.
- **GPL contamination.** Keep the oracle in `/tmp`; commit only the driver,
  corpus, parsed results, and provenance. Never copy code from
  `/tmp/provari-oracle`.

## Report requirements

`report1.md` is the final task. It must include:

- the provenance table and exact commands;
- the core-case repeat table (all runs, medians);
- the `sample20` results, with shared-win set and coverage at 10 s;
- a short diagnosis of whether the gap is latency, coverage, or both;
- deviations from this plan and any environment blockers;
- the recommended plan2: either the win-only front-end skeleton or a smaller
  move-class initialization spike, with the evidence that supports it;
- explicit “not measured” items (tablebases, NNUE training, full high200).
