# Winprover Plan 1: fair hand-guide baseline and corpus re-derivation

Initiative: `winprover`, backlog **item 1**. Drafted 2026-10-30. This is a
**measurement-only** plan: no product code, no search-behaviour change, no
proof-format change.

Prerequisite reading: [`initiative.md`](initiative.md), [`bench.md`](bench.md),
[`AGENTS.md`](../../../AGENTS.md), and the retired baseline
[`provari/plan1.md`](../provari/plan1.md) /
[`provari/report1.md`](../provari/report1.md). The independent checker is
item 2 and is **not** part of this plan; the oracle here is provari's `prove`
command only.

## Why this plan exists

The initiative's success condition is a head-to-head win against provari
`--guide hand` on a shared-win set. The only committed baseline
([`bench-v0`](bench.md#bench-v0--2026-10-30-pre-plan-baseline-no-initiative-plan-executed-yet))
was measured on a three-cell set whose selection rule is "provari hand wins
inside the canonical budget", but that rule was never verified over the whole
frozen corpus:

- `provari` report 1 reports **4/20** `sample20` wins under the bundled model;
  the hand guide is expected to win only **~2/20** (plus `core`), because two
  of those four wins (`b9a7ed7eb368`, `dd3f89766a40`) are model-only. The
  full-corpus hand coverage has never been measured in one session.
- The bench set's three cells were measured under the hand guide, but the set
  was inherited from the retired `provari` initiative's work, not re-derived
  from a whole-corpus hand census. Item 1 must confirm the selection rule
  over all 21 cells before any later plan prices a lever against it.
- The retired baseline also never recorded the two provari columns side by
  side. The initiative requires that: hand is the primary target, model is a
  recorded stretch column that is never mixed into the target.

This plan therefore produces the fair reference column, the re-derived
shared-win set and coverage, and a side-by-side record of both provari
columns. It does **not** append a ledger row (it does not touch the win path);
it may add cells to the bench set by the existing selection rule.

## Scope and non-goals

In scope:

- rebuild the pinned provari oracle from source in a scratch directory;
- measure provari `--guide hand` over `core + sample20` at the canonical
  budget;
- measure provari `--guide model` over the same cells **if** the bundled
  artifacts can be fetched, and record the column as `INCONCLUSIVE` otherwise;
- measure our solver (default and the shipped opt-in stack) over the same
  cells, as a coverage census;
- derive the shared-win set, coverage per column, and wall ratios;
- update the bench set / reference note in `bench.md` if the hand census
  changes it;
- write `report1.md`.

Out of scope (do not do these here):

- no `src/`, `examples/`, `tests/`, or `Cargo.*` changes;
- no proof-tree export, no `provari check`, no independent checker (item 2);
- no new search lever, no default-flag decision (item 6);
- no corpus edits: `sample20` is frozen; do not re-pick cases after seeing
  results;
- no tablebases, no NNUE training, no model artifact committed;
- no ledger row: this plan does not change the win path.

## Oracle setup (scratch only, never committed)

Pinned oracle: `aiorla/provari` commit
`48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`), GPL-3.0-or-later.
Do not copy code, do not add a dependency, do not vendor the binary or the
network/model files.

```sh
rm -rf /tmp/winprover-oracle
git clone https://github.com/aiorla/provari /tmp/winprover-oracle
git -C /tmp/winprover-oracle checkout 48686bc6c786d6ea70568f0b2ad4002cf0520206
cargo build --release --manifest-path /tmp/winprover-oracle/Cargo.toml
/tmp/winprover-oracle/target/release/provari --version
```

The hand column needs only this non-bundled build. For the model column,
attempt the bundled build in the same scratch clone:

```sh
cd /tmp/winprover-oracle
./data/fetch.sh
cargo build --release --features bundled
./target/release/provari --version
sha256sum network.nnue model.json
```

- If the clone or the non-bundled build fails (network/toolchain/glibc),
  record the blocker and stop as **`INCONCLUSIVE`**. Do not substitute an
  untrusted binary; the hand column is the primary deliverable and cannot be
  faked.
- If `data/fetch.sh` or the bundled build fails, the hand column still
  proceeds with the non-bundled binary; record the model column as
  **`INCONCLUSIVE`** with the exact blocker. Do not silently compare against
  a different build.
- If the bundled build succeeds, use that one binary for both guides (the
  hand guide does not read the model) and record the feature set and artifact
  digests in the provenance README. Never commit the artifacts.

## Corpus and protocol

Corpus: [`provari/measurements/plan1/corpus.json`](../provari/measurements/plan1/corpus.json),
sha256 `9cd0169a30bb7d5b162b6a1ec119fe194b776c8a24a46cdf1582c07062847d0e`.
It contains `core` plus exactly 20 `sample20` rows. Verify both the digest and
the row count before measuring; if either differs, stop and record the
deviation.

Canonical protocol (from [`bench.md`](bench.md#canonical-measurement-protocol)):

| Item | Value |
| --- | --- |
| Budget, both sides | 10 s, 192 MB label (`--seconds 10 --memory 192` vs `--timeout 10 --tt-size 192`) |
| Reference | `provari prove "<FEN>" --seconds 10 --memory 192 --guide hand` (primary) / `--guide model` (stretch) |
| Candidate | `atomic_solver --fen "<FEN>" --timeout 10 --tt-size 192 --first-outcome --outcome-only <stack flags>` |
| Stacks | `default` (no flags) and `opt` = `--move-class-pn --quiet-grouping --eval-pn or` |
| Repeats | 3 for `core` and every existing/new bench cell; 1 for the other `sample20` cells |
| Ordering | sequential, one process at a time, same host, same session |
| Tablebases | none |

The driver is
[`measurements/bench/run_bench.py`](measurements/bench/run_bench.py). It
already measures the reference and one or more candidate stacks in one
invocation and writes per-run rows plus the derived ledger. Two small,
backwards-compatible changes are part of this plan (measurement tooling, not
product code):

1. Add `--guide {hand,model,none}` (default `hand`) and use it in
   `provari_command`; record it in `provenance.provari.guide`.
2. Make the reference side label `provari-<guide>` (so `provari-hand` is
   unchanged for the default) and pass that label to `medians`/`ledger`.

Keep the default output path and the default behaviour unchanged: the
canonical `bench.md` command must still work verbatim. Do not overwrite
[`measurements/bench/results.json`](measurements/bench/results.json) (the
`bench-v0` artifact); write this plan's outputs under
`measurements/plan1/`.

**Driver usage note.** `run_bench.py` always includes the `default` stack;
passing `--stack 'default='` adds a duplicate. Use only
`--stack 'opt=--move-class-pn --quiet-grouping --eval-pn or'` for the opt
stack. (The `bench-v0` command in `bench.md` has the redundant
`--stack 'default='`; it is harmless but wasteful. Do not rewrite the
historical command.)

## Measurements to take

All outputs go under `docs/plans/winprover/measurements/plan1/`.

### 1. Hand column (primary)

Census over all 21 cells, 1 repeat:

```sh
python3 docs/plans/winprover/measurements/bench/run_bench.py \
  --provari /tmp/winprover-oracle/target/release/provari \
  --solver ./target/release/atomic_solver \
  --guide hand --repeats 1 \
  --stack 'opt=--move-class-pn --quiet-grouping --eval-pn or' \
  --provari-commit 48686bc6c786d6ea70568f0b2ad4002cf0520206 \
  --solver-commit "$(git rev-parse HEAD)" \
  --out docs/plans/winprover/measurements/plan1/results_hand_census.json
```

Bench-cell repeats (3) for `core` and the existing bench cells: same command
with `--cases core,1eb80b92f92c,db6a741c723f --repeats 3` and
`--out .../results_hand_bench.json`. If the census finds a new hand win, add
its cell id to this `--cases` list and re-run it (3 repeats) so the new cell
has a stable reference wall.

### 2. Model column (stretch, only if the bundled build succeeded)

Same two invocations with `--guide model`, writing
`results_model_census.json` and `results_model_bench.json`. If the bundled
build failed, write `model_column.json`:

```json
{"status": "INCONCLUSIVE", "reason": "<exact blocker>", "commit": "48686bc6..."}
```

and do not run the model column. The model column is recorded side by side
and is **never** used for the bench-set selection rule or the primary
verdict.

### 3. Candidate census (coverage only)

The same invocations already run both candidate stacks (the driver always
runs the reference plus every stack). The candidate rows are a **coverage
census**, not a ledger row: they tell us which cells are shared wins and
whether our coverage matches provari's. They do not change the win path and
therefore do not append to the ledger.

## Derivation

Add `measurements/plan1/derive.py` (or an equivalent documented script) that
reads the results files and writes `census.json` and `census.md`. It must:

- merge the 1-repeat census with the 3-repeat bench file, preferring the
  3-repeat row for `core` and every bench cell;
- list every cell with: provari hand win/wall/nodes/mate, provari model
  win/wall/nodes/mate (or `INCONCLUSIVE`), solver default win/wall/evals,
  solver opt win/wall/evals;
- compute **coverage** per column over the 21 cells (wins / 21), and the
  `sample20`-only coverage (wins / 20) separately;
- compute the **shared-win set** for `hand × default`, `hand × opt`, and
  (if available) `model × opt`, with the wall ratio
  `solver_wall / provari_wall` for each shared cell;
- compute the geometric mean of the wall ratios over the shared cells, with
  the cell count stated (a one-cell geomean is just that cell's factor);
- list **new hand wins** not already in the bench set, and the resulting
  bench set under the fixed selection rule;
- never drop a censored row; a censored cell is reported as `censored` with
  its bound form where meaningful.

The script must set `sys.dont_write_bytecode = True` and must not write raw
transcripts.

## Bench-set update

Apply the fixed selection rule from `bench.md`: the bench set is every cell
of `core + sample20` where provari **hand** wins inside the canonical budget.

- If the census confirms the existing three cells (`core`, `easy-win`
  `1eb80b92f92c`, `deep-win` `db6a741c723f`), append a dated note to
  `bench.md` recording the re-derived hand reference values and coverage.
  Do not rewrite the `bench-v0` row.
- If a new hand win appears, add it to the bench set with a short name, cell
  id, FEN, reference values, and a one-line "why this cell"; record the
  addition as a dated note. Earlier rows are not rewritten.
- If a cell previously in the set is **not** a hand win (contradicting the
  selection rule), record the discrepancy explicitly and remove it from the
  set only with the measured evidence; do not silently keep it.
- Append **no ledger row**. The ledger's "Rows to come" section already says
  item 1 is reference-only; keep that true.

## Tasks

1. **Read and confirm.** Re-read this plan, `initiative.md`, `bench.md`, and
   the retired `provari/plan1.md`/`report1.md`. Confirm the pinned commit, the
   GPL/no-import boundary, and the corpus digest.
2. **Build the oracle** per the setup section. Record `provari --version`,
   commit, feature set, and (if fetched) the `sha256` of `network.nnue` and
   `model.json` in the provenance README. Do not commit the clone, binary, or
   artifacts.
3. **Extend the driver** with `--guide` and the dynamic reference side label.
   Verify `--dry-run` prints `--guide hand` by default and `--guide model`
   when asked; verify `--help` shows the flag. Do not change the default
   output path or the default command shape.
4. **Verify the corpus.** Check the sha256, the 20 `sample20` rows, and the
   byte-identical `core` FEN. Record the check in the provenance README.
5. **Run the hand census and bench repeats** (section 1). Keep every row,
   including censored ones.
6. **Run the model column** if the bundled build succeeded (section 2);
   otherwise write `model_column.json` with the blocker.
7. **Derive the census** with `derive.py`; write `census.json` and
   `census.md`.
8. **Update `bench.md`** per the bench-set update section (dated note, no
   ledger row).
9. **Write `report1.md`** (final task). Include: provenance and exact
   commands; the per-cell table; coverage per column (hand, model, default,
   opt); the shared-win sets and wall ratios; the re-derived bench set; the
   comparison of the hand and model columns; deviations and blockers; and a
   recommendation for plan2 (expected: item 2, the differential checker
   oracle, unless the census changes the ranking).
10. **Clean up.** No `__pycache__`, no raw logs, no provari artifacts, no TT
    snapshots. Confirm with `git status --short` and `git add --dry-run` that
    only `docs/plans/winprover/` files would be added.

## Acceptance

- The corpus digest matches and `sample20` has exactly 20 rows.
- The hand census covers all 21 cells; every row is present, including
  censored rows.
- Coverage and the shared-win set are computed from the rows, not from
  memory; the `sample20`-only and `core + sample20` counts are both stated.
- The model column is either measured over the same cells or recorded as
  `INCONCLUSIVE` with the exact blocker; it is never mixed into the primary
  column.
- `bench.md` has a dated reference note; the `bench-v0` row is intact; no
  ledger row was appended.
- `report1.md` is written and names the recommended plan2.
- No `src/`, `examples/`, `tests/`, or `Cargo.*` changes. If any were made by
  accident, revert them and run `make test`.
- `git status --short` shows only `docs/plans/winprover/` files; no provari
  binary, network/model, raw log, or `__pycache__` is staged.

## Validation

This plan changes no Rust code, so `make test` is not required. Run:

```sh
python3 - <<'PY'
import ast
from pathlib import Path
for p in [
    "docs/plans/winprover/measurements/bench/run_bench.py",
    "docs/plans/winprover/measurements/plan1/derive.py",
]:
    path = Path(p)
    if path.exists():
        ast.parse(path.read_text())
PY
```

and the driver once in `--dry-run` mode for both guides. Confirm every
`results_*.json` and `census.json` parses with the standard library. Do not
use `py_compile` (it writes `__pycache__`).

## Risks and mitigations

- **Oracle clone/build fails.** Record `INCONCLUSIVE` and stop; do not
  substitute an untrusted binary. The hand column is the primary deliverable.
- **Model fetch fails.** Hand proceeds; model column is `INCONCLUSIVE` with
  the blocker. This is expected in a network-restricted container and is not
  a plan failure.
- **Host drift.** All factors are same-session ratios; record the host and do
  not compare a new factor against a `bench-v0` row from another host. The
  reference walls for `core` have already varied by ~20 % across hosts.
- **Long runtime.** The full census is ~21 cells × (1 reference + 2 stacks) ×
  10 s worst case, plus the 3-repeat bench cells. Run sequentially, one
  process at a time; do not parallelise (it would invalidate the wall
  comparison).
- **Censored cells.** Keep them; a censored cell is a result, not a missing
  row. Never convert a timeout into an omitted row or a win.
- **Driver duplicate default stack.** Use `--stack 'opt=...'` only; the
  driver already includes `default`.
- **Corpus drift.** If the digest or row count differs, stop and record the
  deviation; do not silently re-pick cases.

## Report requirements

`report1.md` is the final task. It must include:

- the provenance table (date, host, provari commit/version/features, model
  artifact digests or `INCONCLUSIVE`, solver commit, corpus digest, budgets);
- the exact commands used;
- the per-cell table for all 21 cells, with both provari columns and both
  solver stacks;
- coverage per column, stated for `sample20` and for `core + sample20`;
- the shared-win sets and wall ratios, with the cell count and the
  geometric mean (or the single factor);
- the re-derived bench set and any additions/removals, with the evidence;
- a short comparison of the hand and model columns (the model column is
  context, not the target);
- deviations from this plan and any environment blockers;
- the recommended plan2, with the evidence that supports it;
- explicit "not measured" items (tablebases, NNUE training, full `high200`,
  independent checker).
