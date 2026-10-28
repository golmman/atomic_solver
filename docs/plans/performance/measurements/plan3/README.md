# plan3 measurement provenance (TT pre-probe before make/unmake)

Plan: [`../../plan3.md`](../../plan3.md). Report:
[`../../report3.md`](../../report3.md).

## plan_session/ — the plan session's read-only probe (2026-10-28)

Calibration record for plan3's pre-registered bars. No product code was
modified: all probe builds live under `/tmp` (`git archive HEAD` copies) and
nothing from them landed.

| file | content |
|---|---|
| `ab_final.py` | interleaved ABAB A/B between the pristine-`HEAD` baseline binary (`/tmp/plan3_base`) and the `/tmp` prototype (`/tmp/plan3_proto`): per-cell median/min wall, `child_evals`, `ru_maxrss`, `ru_minflt`, stdout capture; asserts nothing, records everything |
| `ab_final.json` | the run behind the plan's prototype table (reps: 15 for `mate1`/`easy_kq`, 11 for `dec13`/`dec10`/`m22`/`m22_b`/`stress_b`, 7 for `rem12`); **every cell is `child_evals`- and stdout-identical** across the two arms |
| `quick_base.json`, `quick_proto.json` | `benchmark --suite quick --first-outcome --timeout 3 --json` for both arms: 59/59 eval-identical, 42,109,768 total child evals in each |
| `env.json` | host, toolchain, flags, the further probe procedures |

Not committed (per AGENTS.md's measurement layout): raw solver transcripts, and
the census instrumented build (its patch is described in the plan; reproducing
it means adding per-branch counters at the head of `Search::evaluate_child` and
printing them behind an env var — the counter definitions are quoted verbatim in
plan3.md's census table header).

Two probe procedures the plan session ran as one-liners (the execution session
scripted the census as `census.py`):

- **census**: the instrumented `/tmp` build, one run per corpus cell, with
  per-branch counters (solved-entry skip / live-unsolved skip / path repetition
  / best-move guard / rule50-expired); the numbers in plan3.md reproduce report
  §7.3's counts exactly on the overlapping cells (`dec10`, `m22_b`).
- **perf**: `perf record -e cpu-clock -g` over 15 `dec10` runs of the baseline
  binary, `perf report --stdio --no-children` for leaf attribution (this
  container has no hardware PMU; `cpu-clock` sampling is the ceiling).


## state/ — the execution session (2026-10-28, resumed after a crash)

The final Phase-0/census/after record and the authoritative interleaved A/B.
Host and methodology are in [`state/env.json`](state/env.json).

| file | content |
|---|---|
| `state/baseline.json`, `state/after.json` | the single-arm corpus for the pristine-`HEAD` baseline and the final working tree (5 runs/case): median wall, `child_evals`, RSS, minor faults, outcome/PV/`pv_status`, stdout |
| `state/census.json` | the census re-run on a copy of the final tree (`census.py` patch, counters only): per cell solved-entry skip, live-unsolved skip, path-repetition fallback, best-move guard, rule50-expired, skip rate |
| `state/ab_wall_final.json` | authoritative A/B, 11–21 reps/case, counterbalanced order; per-run `child_evals`/stdout identity is a hard driver abort |
| `state/ab_wall_short_cb.json` | the two sub-2 ms latency-floor cells at 151 reps with both binaries on the same filesystem (process start from `/tmp` vs the workspace overlay is a ~1 ms confound there) |
| `state/quick_after.json` | `benchmark --suite quick --first-outcome --timeout 3 --json`: 59/59 eval-identical to `measurements/plan2/state/quick_after.json`, 42,109,768 total child evals |

`ab_wall.py` alternates the arm order per repetition (base→after, then
after→base, …). A fixed base-then-after order made the after arm the second run
of every pair, which biased the ratio against it on the dec10 borderline; the
counterbalanced order removes that. The driver docstring also records the
same-filesystem requirement for the latency-floor cells.

Not committed: the raw A/B transcripts and the `/tmp` baseline/census builds.
The exploratory high-rep reruns that led to the `inline(always)` fallback fix
(pre-inline dec10 ~0.963 vs post-inline ~0.946) are not retained as data files;
the finding is recorded in `report3.md`.

## Handoff to the execution session

- Phase 0 re-measures the baseline **in-session**: absolute walls on this
  container have drifted since `report2.md` (`dec10` 0.65 → 0.35 s, `m22`
  2.19 → 1.47 s). The bars are ratios, never remembered absolutes.
- The prototype's first version lost the entire deep-cell gain by probing the
  TT twice per child (pre-pass + fallback). The single-probe reuse (plan3 §D-f)
  is part of the design, not an optimization to be dropped.
