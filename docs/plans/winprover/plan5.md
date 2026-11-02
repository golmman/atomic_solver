# Winprover Plan 5: pivot disposition and head-to-head close-out (item 6)

Initiative: `winprover`, backlog **item 6** (head-to-head convergence), with
the report3 §7 candidate/verification pivot and item 5 (rule50-free key)
disposed of first. Drafted 2026-11-02. This is a **decision/close-out** plan,
not a new search mechanism: it records the read-only probes that close the
remaining search threads, decides the default flag set, and runs the final
canonical head-to-head. The initiative's success condition (geometric-mean
wall `< 1.0` with coverage at least provari's) is **not** met by the measured
state; the plan records the gap honestly and recommends the next direction
instead of stacking another small lever.

Prerequisite reading: [`initiative.md`](initiative.md), [`bench.md`](bench.md),
[`report3.md`](report3.md), [`report4.md`](report4.md),
[`plan4.md`](plan4.md), [`AGENTS.md`](../../../AGENTS.md), and the retired
[`provari/initiative.md`](../provari/initiative.md) plus
[`provari/report5.md`](../provari/report5.md).

## Why this plan exists

Plan 4's pre-registered gate failed: the opt-in attacker-only front-end, with
the first-form sibling replay, proves none of the three bench cells at a
10,000,000 child-eval cap. Report 4 therefore recommended the report3 §7
**candidate/verification-layer pivot** as the next plan session, with item 5
(rule50-free key + proof length) as the fallback. This plan session ran the
read-only probes for both, plus a front-end heuristic-parity probe. All three
are measured-out on the bench cells:

- the candidate/verification pivot has no viable candidate source at a
  product-affordable cap (the bounded general search's best-move chain is not
  a winning line, and the front-end cannot verify a fixed root move);
- the rule50-free key changes nothing on `core`/`easy-win` and only ~0.7 % of
  the censored `deep-win` work;
- the front-end with the shipped opt-in starting-number levers proves `core`
  only at ~30.9 M evals / ~15 s, roughly 2.2× the general solver's work and
  4× its wall, and does not prove `easy-win` at 50 M.

The remaining gap is architectural, not a missing small lever. The next
useful step is therefore item 6: decide the default flag set from the
measured evidence, run the final head-to-head, and close the search thread
with a recorded negative result and a concrete recommendation for a new
initiative.

## Plan-session probes (read-only, 2026-11-02)

All probes ran on a temporary copy of the tree at commit `5e8a1bb` + the
plan4 working tree. The copy was deleted after the probes; no probe code is
committed. They are **sizing evidence**, not product results. The host is the
same container as reports 3–4 (aarch64, 8 CPU).

### 1. Candidate/verification pivot

Three candidate sources were tried, all with the front-end's scratch TT and
the shipped opt stack for the candidate generator:

| Probe | Cell | Cap | Result |
| --- | --- | ---: | --- |
| Candidate line from a bounded general search | `core` | 1,000,000 | 8-move best-move chain; not a winning line |
| Hard-guide front-end (attacker follows the candidate, defender tries all replies) | `core` | 10,000,000 | not proved; 10,000,017 evals, 2,126,387 frames, 4.256 s |
| Root-move verification (front-end proves a fixed root move's child is a loss) | `core` | 1,000,000 per move | 0/41 root moves proved; 41,000,908 evals, 15.382 s |
| Root-move verification | `easy-win` | 1,000,000 per move | 0/38 root moves proved; 38,000,943 evals, 13.027 s |

Reading: the available candidate sources do not produce a line the front-end
can verify at a cap that fits the product budget. The candidate line from a
bounded general search is a best-move chain, not a proof; the hard guide
therefore fails. Root-move verification is the strongest form (the root move
is fixed, so the front-end only has to prove one child), and it still fails
on every root move at 1 M evals. The pivot is **measured-out** for the
current candidate sources. A learned or hand-evaluation candidate generator
is out of scope (the initiative's non-goals exclude a model dependency), and
a new candidate generator would be a new mechanism, not a pivot of the
existing front-end.

### 2. Rule50-free key prototype (item 5, key half)

The prototype made `zobrist::rule50_key` return `0`, so the TT key is
board-only. This is the **unsound upper bound** of the rule50-free key lever
(it reuses proofs across clocks without the proof-length/holding-clock
check); a sound implementation cannot do better on the same cells.

| Cell | Baseline evals | Rule50-free evals | Outcome |
| --- | ---: | ---: | --- |
| `core` | 13,845,432 | 13,845,432 | win, identical |
| `easy-win` | 2,697,438 | 2,697,438 | win, identical |
| `deep-win` | 49,089,691 | 48,744,931 | draw (censored), ~0.7 % fewer evals |

Reading: the rule50 key is not causing a measurable TT-miss cost on the two
shared cells, and the censored cell gains only ~0.7 % of its work without
changing coverage. The key half of item 5 is **measured-out** on the bench
set. The proof-length/holding-clock half is not separately measured here; it
is a soundness-sensitive mechanism whose only measured motivation was the
key, so it is closed with the key rather than implemented speculatively.

### 3. Front-end heuristic parity prototype

The front-end was given the shipped opt-in starting-number levers
(`move_class_pn` + `eval_pn = Or`) in the temporary copy. This is the
"front-end heuristic parity" idea that would otherwise be the next search
plan.

| Cell | Cap | `off` | `mc + ep` |
| --- | ---: | --- | --- |
| `core` | 10,000,000 | not proved, 10,000,034 evals | not proved, 10,000,005 evals |
| `easy-win` | 10,000,000 | not proved, 10,000,032 evals | not proved, 10,000,027 evals |
| `core` | 50,000,000 | not proved, 50,000,024 evals | **proved**, 30,851,728 evals, 15.042 s, depth 27 |
| `easy-win` | 50,000,000 | not proved, 50,000,020 evals | not proved, 50,000,027 evals |

Reading: the levers change the front-end's search shape but do not make it
competitive. `core` is proved only at ~30.9 M evals / ~15 s, versus the
general solver's 13.8 M evals / ~3.8 s on the same cell (2.2× the work, 4×
the wall); `easy-win` is not proved at 50 M. The front-end's per-eval cost and search efficiency, not its
starting numbers, are the bottleneck. The front-end thread is **closed**:
the code stays in the tree (opt-in, default off, all gates green), but no
further front-end lever is planned.

### Probe limitations

- The candidate/verification probe used a temporary hard-guide hack and a
  root-move loop, not a faithful candidate/verification layer. The negative
  result is about the **available candidate sources** and the front-end's
  verification power, not a proof that no candidate/verification design can
  ever work.
- The rule50-free prototype is deliberately unsound; it is an upper bound on
  the lever's benefit, not a candidate implementation.
- The front-end heuristic prototype is a temporary port, not a faithful
  integration; it is sizing evidence for the decision to close the thread.

## Scope and non-goals

In scope:

- record the three probe results above in the plan and the report;
- decide the default flag set (item 6) from the measured evidence, with a
  pre-registered decision rule (below);
- if the decision is to promote the opt stack, implement the default change
  and its CLI/help/golden/test updates; otherwise keep the shipped defaults
  and record why;
- run the final canonical head-to-head on the three bench cells with the
  chosen default and the best measured stack, and append one ledger row;
- update `initiative.md` and `docs/plans/README.md` to close the search
  thread and record the measured gap;
- write `report5.md` with the probe tables, the decision, the head-to-head,
  and a concrete next-direction recommendation.

Out of scope (do not do these here):

- no new search mechanism, no front-end change, no candidate/verification
  implementation, no rule50-free key or proof-length implementation;
- no second-form sibling replay (`replay_threat`), no static-tactics or
  defender-side closures, no new heuristics;
- no proof-tree/proofdb/format change, no tablebases, no NNUE/model;
- no new corpus, no change to the bench set or the canonical protocol;
- no parallel/portfolio work, no unrelated micro-optimization.

## Design

### Default-flag decision (item 6)

The candidate default is the shipped opt-in stack:

```text
move_class_pn = true
quiet_grouping = true
eval_pn = Or
```

The decision rule is pre-registered:

1. Measure the **quick suite** (59 cases) with the old default and with the
   opt stack, at the suite's canonical settings (`--first-outcome`,
   `--timeout 3`, one run per case, release build). Compare outcomes
   case-by-case.
2. If the opt stack has **no outcome regression** on the quick suite, promote
   it to the default: change `CliOptions`/`Search` defaults, add
   `--no-move-class-pn` and `--no-quiet-grouping` (and keep `--eval-pn off`)
   so the old behavior remains reachable, update `print_help()` and
   `tests/fixtures/cli_help_golden.txt`, and re-baseline the affected
   trajectory/golden tests with a recorded justification.
3. If the opt stack has **any outcome regression**, keep the shipped defaults
   (all experimental levers off) and record the regression and the decision
   in the report. The opt stack remains the best measured configuration and
   stays opt-in.

The decision is about the **default flag set**, not about the initiative's
success condition. Even if the opt stack is promoted, the head-to-head still
fails the `< 1.0` goal; the report must say so.

### Head-to-head measurement

Use the canonical protocol from [`bench.md`](bench.md):

- 10 s / 192 MB label on both sides, three repeats per cell per side, one
  process at a time, same host and session;
- provari `--guide hand` is the 1.000 reference;
- cells: `core`, `easy-win` (`1eb80b92f92c`), `deep-win` (`db6a741c723f`);
- stacks: the chosen default, the opt stack (same as default if promoted),
  and optionally the `opt+win` front-end control (recorded, not a candidate);
- record per-cell factor, canonical geometric mean, coverage, and
  deterministic `child_evals`; append **one** ledger row.

The driver is the committed
[`measurements/bench/run_bench.py`](measurements/bench/run_bench.py). If the
default change makes the driver's `default` label equal to `opt`, keep both
labels in `results.json` and state the equivalence in the report; do not
rewrite earlier rows.

### Close-out

Update [`initiative.md`](initiative.md):

- item 4: first form done, second form deferred/closed, front-end thread
  closed (kept opt-in);
- item 5: closed as measured-out (key half) with the proof-length half
  closed with it;
- item 6: done with the measured head-to-head and the default decision;
- status: **closed** (or **dormant** if the owner wants to keep the
  initiative open for a future new mechanism) with a dated history entry;
- a "next direction" note: a from-scratch provari-parity attacker-only prover
  (full opt stack, optimized child evaluation, provari-style `Mover`/effect
  precomputation) is the only measured path to the `< 1.0` goal; it is a new
  initiative, not a winprover plan.

Update the `docs/plans/README.md` winprover row to match.

## Tests

If the default is promoted:

- CLI parsing tests for `--no-move-class-pn` and `--no-quiet-grouping`, and
  for `--eval-pn off` restoring the old behavior;
- `tests/fixtures/cli_help_golden.txt` updated;
- a default-equivalence test: the new default produces the same outcome,
  PV, nodes, and child evals as the explicit opt stack on a fixture;
- the quick-suite comparison is the regression gate; any outcome change is a
  veto for promotion;
- the repetition soundness gate
  (`tests/test_repetition.rs -- --include-ignored`) and the existing anchors
  (`dec10`, `m22`, quick suite) must be green; re-baseline only with a
  recorded justification.

If the default is not promoted, no product-code test changes are needed; the
report records the quick-suite regression that blocked promotion.

## Measurement and gate

**Gate 1 — default decision.** The opt stack is promoted only if the quick
suite has no outcome regression. A regression keeps the shipped defaults and
is recorded; it is not worked around.

**Gate 2 — head-to-head.** The final canonical measurement is recorded
honestly: per-cell factors or `censored` + bound, geometric mean, coverage,
and deterministic evals. The initiative's success condition is not met; the
report states the gap and does not add a new lever to chase it.

**Gate 3 — correctness.** `make test` and `make test-full` green; `cargo fmt
--check`, `cargo clippy --all-targets -- -D warnings`, `cargo doc --no-deps`,
and `cargo build --release` clean. If the default is promoted, the quick
suite, repetition soundness, and anchors are green.

## Tasks

1. **Read and confirm.** Re-read this plan, `initiative.md`, `bench.md`,
   `report3.md`, `report4.md`, `plan4.md`, and `AGENTS.md`. Confirm the
   close-out boundary: no new search mechanism, no front-end change, no
   rule50-free key implementation.
2. **Run the quick-suite comparison.** Measure the old default and the opt
   stack on the 59-case quick suite at the suite's canonical settings.
   Record the per-case outcomes and the regression list.
3. **Apply the default decision.** If there is no outcome regression, promote
   the opt stack (CLI/`Search` defaults, `--no-...` flags, help, golden
   fixture, tests). If there is a regression, keep the shipped defaults and
   record the decision.
4. **Run the canonical head-to-head.** Use the committed bench driver on the
   three cells with the chosen default and the opt stack; append one ledger
   row to `bench.md`.
5. **Write the measurement record.** Add
   `measurements/plan5/{README.md,run_plan5.py,results.json,results.csv}`
   (provenance, driver, parsed results; no raw transcripts, no
   `__pycache__`, no TT snapshots).
6. **Update the initiative and index.** Record the probe dispositions, the
   default decision, the head-to-head, and the next-direction note in
   `initiative.md`; update the `docs/plans/README.md` winprover row.
7. **Write `report5.md`** (final task). Include: provenance and exact
   commands; the three probe tables and their limitations; the quick-suite
   comparison and the default decision; the head-to-head table and ledger
   row; the gate results; the verdict (goal not met, search thread closed);
   the explicit "not measured" list; and the next-direction recommendation.
8. **Clean up.** No `__pycache__`, no raw logs, no TT snapshots, no
   temporary instrumented copy, no provari artifacts. Confirm with
   `git status --short` and `git add --dry-run` that only the intended files
   would be added.

## Acceptance

- The candidate/verification pivot, the rule50-free key, and the front-end
  heuristic-parity idea are recorded as measured-out with the probe data and
  their limitations.
- The default-flag decision is made by the pre-registered rule and is
  justified by the quick-suite comparison; if the opt stack is promoted, the
  old behavior remains reachable and the quick suite has no outcome
  regression.
- The final canonical head-to-head is recorded with per-cell factors,
  geometric mean, coverage, and deterministic evals; the initiative's
  success condition is reported honestly as not met.
- `initiative.md` and `docs/plans/README.md` are updated; the report
  recommends a concrete next direction (a from-scratch provari-parity prover)
  or closure.
- `make test` and `make test-full` are green; fmt, clippy, doc, and release
  build are clean.
- `git status --short` shows only the intended source, test, doc, and
  measurement files; no temporary instrumented copy, raw log, TT snapshot,
  or `__pycache__` is staged.

## Validation

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
make test
make test-full
cargo doc --no-deps
cargo build --release
python3 - <<'PY'
import ast
from pathlib import Path
for p in Path("docs/plans/winprover/measurements/plan5").glob("*.py"):
    ast.parse(p.read_text())
PY
```

Do not use `py_compile` (it writes `__pycache__`). Confirm every committed
JSON/CSV parses with the standard library.

## Risks and mitigations

- **The quick suite regresses under the opt stack.** That is a valid outcome:
  keep the shipped defaults, record the regression, and do not promote. The
  opt stack stays the best measured opt-in configuration.
- **The head-to-head is worse than the earlier rows.** Record it; the
  canonical protocol is same-session and same-host, so the row is valid on
  its own host. Do not compare it to an older row and call the difference a
  lever effect.
- **The probes are not faithful implementations.** State the limitations;
  the negative result is about the available candidate sources and the
  front-end's measured power, not a universal impossibility proof.
- **Scope creep.** Do not add a new search mechanism, a front-end change, a
  rule50-free key, or a new corpus to "rescue" the goal. The plan's value is
  the honest close-out and the next-direction recommendation.
- **The default change touches many tests.** If the promotion branch is
  taken, budget for the golden/trajectory re-baseline and justify every
  changed expectation; if the re-baseline is not clean, fall back to keeping
  the opt stack opt-in.

## Report requirements

`report5.md` is the final task. It must include:

- the provenance table (date, host, solver commit, budgets, corpus, oracle);
- the exact commands used;
- the three plan-session probe tables (candidate/verification, rule50-free
  key, front-end heuristic parity) and their limitations;
- the quick-suite comparison (old default vs opt stack) and the default
  decision, including any regression that blocked promotion;
- the canonical head-to-head table (per cell, factor or censored bound,
  evals, coverage) and the ledger row;
- the gate results (`make test`, `make test-full`, fmt, clippy, doc, release
  build);
- the verdict: the initiative's success condition is not met; the search
  thread is closed; the front-end stays opt-in; item 5 is measured-out;
- the explicit "not measured" list (second-form replay, proof-length/holding
  clock, a new candidate generator, a new corpus, tablebases, NNUE);
- the next-direction recommendation (a from-scratch provari-parity prover, or
  closure) with the measured evidence that motivates it.

## Deliverables

- `docs/plans/winprover/plan5.md` (this plan).
- `docs/plans/winprover/report5.md`.
- `docs/plans/winprover/measurements/plan5/` (provenance README, driver,
  parsed results).
- `docs/plans/winprover/bench.md` (one ledger row).
- `docs/plans/winprover/initiative.md` and `docs/plans/README.md` status
  updates.
- If the default is promoted: `src/cli.rs`, `src/main.rs`,
  `src/search/dfpn/{mod,config}.rs`, `tests/fixtures/cli_help_golden.txt`,
  and the affected tests.
