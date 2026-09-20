# Plan 4 Report: Depth- and clock-scheduled ε POC (threshold-schedule heterogeneity)

## Summary

Phase 0 re-ran the global-ε sweep per-case and first-outcome (the 2026-09-11
sweep was suite-aggregate and timeout-noise-dominated) and **refuted the
"global ε is inert" claim on the gate object**: on the stress case, ε=0.5
gives 156,394,866 first-outcome child evals vs the 249,480,478 default
(−37.3%). However, ε=0.5 regresses the controls (m22 +12.7%, dec13 +52.4%),
so no single global ε clears the GO gate.

Phase 1 measured four env-gated schedule arms (path depth, remaining depth,
rule50 clock, linear-in-path-depth) with the default ε=0.125 as the base of
every unscheduled region. **No arm beats the default anywhere on the gate
object**: `d_leaf` and `r50` never (or microscopically) fire; `d_root` and
`linear` — the two arms that coarsen ε near the root — regress the stress
case (+28.4% / +11.0%) and are catastrophic on m22 (+351% / 30 s timeout).

**Gate decision: NO-GO for backlog #11 (scheduling the threshold by depth or
clock).** The churn mass's threshold response is *not regionally structured*:
the strong non-flatness the sweep exposed is per-position trajectory chaos,
not a schedulable gradient. A spin-off finding is recorded: the global
constant **ε=0.375 is Pareto-improving across all four cases** (stress −8.7%,
m22 −12.7%, dec13 +2.6%, dec10 −14.5%) — a PARTIAL-band candidate (3–10%,
below the 10% GO bar) handed to `conversion` as a sized note, separate from
the closed scheduling question.

Per the plan's NO-GO consequence, the next plan is literature target #5
(child-level early termination in DF-PN), which attacks the same churn mass
from the child-granularity side.

## Phase 0 — clean global ε sweep

First-outcome `child_evals` (delta vs each case's ε=0.125 baseline; full
table in `measurements/plan4/plan4_derived.csv`):

| ε | stress (249,480,478) | m22 (14,156,269) | dec13 (3,822,602) | dec10 (4,262,128) |
|---|---|---|---|---|
| 0 | 1,085,531,183 (+335.1%) | 20,553,440 (+45.2%) | 4,224,109 (+10.5%) | 4,594,131 (+7.8%) |
| 0.0625 | 1,083,780,750 (+334.4%) | 16,925,024 (+19.6%) | 4,655,699 (+21.8%) | 2,748,468 (−35.5%) |
| **0.125** | **249,480,478** | **14,156,269** | **3,822,602** | **4,262,128** |
| 0.25 | 347,233,661 (+39.2%) | 21,152,291 (+49.4%) | 3,545,815 (−7.2%) | 3,102,360 (−27.2%) |
| 0.375 | 227,834,433 (−8.7%) | 12,351,299 (−12.7%) | 3,921,011 (+2.6%) | 3,642,166 (−14.5%) |
| 0.5 | 156,394,866 (−37.3%) | 15,952,085 (+12.7%) | 5,827,389 (+52.4%) | 3,273,293 (−23.2%) |
| 1.0 | timeout (1.96 B evals) | 51,843,616 (+266.2%) | 7,947,572 (+107.9%) | 2,367,895 (−44.4%) |

Findings:

1. **The inert claim is dead.** The response is violently non-flat and
   non-monotone (0.25 is worse than 0.125 on stress while 0.375 and 0.5 are
   better; dec10's optimum is at the grid edge ε=1.0). The 2026-09-11
   suite-aggregate ~1.4% spread was an artifact of timeout-dominated totals.
2. **No global ε satisfies GO control parity.** ε=0.5 wins the gate object by
   −37.3% but regresses m22 (+12.7%) and dec13 (+52.4%); every other value
   either loses on stress or regresses a control.
3. **ε=0.375 Pareto-improves all four cases** (worst case dec13 at +2.6%,
   inside the +5% control bound). It is the only candidate with no control
   regression and a stress win, but at −8.7% it sits under the 10% GO bar.
4. m22's optimum is exactly the default (0.125); the m20–m23 family is
   already tuned at the default on this surface.

Outcome invariance holds for every finishing run (all `win`, matching the
baselines); the two timeouts (stress ε=1.0) return `draw` by construction.
PV length varies with ε — inherent for a trajectory knob under
`--first-outcome` (see `measurements/plan4/summary.md`).

## Phase 1 — schedule arms

Arms (base ε=0.125 in every unscheduled region; spike-off control =
bit-identical, verified per-case on the quick suite, aggregate 38,974,090):

| Arm | stress | m22 | dec13 | dec10 |
|---|---|---|---|---|
| d_root (ε=0.5 while path_depth<8) | 320,435,684 (+28.4%) | timeout (279 M evals) | 5,521,685 (+44.5%) | 3,290,784 (−22.8%) |
| d_leaf (ε=0.5 while max_depth<16) | 249,480,478 (0.0%) | 14,156,269 (0.0%) | 3,822,602 (0.0%) | 4,262,128 (0.0%) |
| r50 (ε=0.5 while rule50≥60) | 249,480,248 (−0.0001%) | 14,156,210 (−0.0004%) | 3,822,602 (0.0%) | 4,262,128 (0.0%) |
| linear (ε=0.125·(1+path_depth/16)) | 276,802,415 (+11.0%) | 63,863,508 (+351.1%) | 3,836,614 (+0.4%) | 6,129,011 (+43.8%) |

Findings:

1. **`d_leaf` and `r50` never effectively fire.** Exact bit-equality of
   `child_evals` with the spike-off control means not a single second-child
   threshold update was scheduled differently. The churn mass's threshold
   updates happen at frames with remaining depth ≥16 and rule50 <60 — plan1's
   threshold-cut mass lives deep in material-rich middlegames where neither
   the horizon nor the clock is near. The clock and horizon axes are simply
   empty at the churn.
2. **Near-root coarsening is harmful, not helpful.** The D_ROOT hypothesis
   ("absorb root fan-out churn coarsely") fails in both directions: it loses
   +28.4% on the gate object, times out m22, and gains only on dec10
   (−22.8%) — while the *global* ε=0.5 wins stress by −37.3%. Combined with
   finding 1, the beneficial coarsening mass on stress sits **neither at the
   root nor at the horizon nor on the clock** — it is distributed through the
   mid-tree in a way none of the four tested partitions captures.
3. **Schedule interactions are not additive.** m22 tolerates global ε=0.5
   (15.95 M evals) but *times out* when ε=0.5 is applied only near the root
   (d_root) and explodes +351% under linear ramping. The proof-number
   trajectory couples the regions; regional ε changes invalidate the
   thresholds every other region computed under the old schedule.

## Hypothesis verdicts

- **H1 (heterogeneous response) — rejected.** No scheduling arm beats even
  the default, let alone every global ε by ≥10% at control parity. The
  optimal ε is not partitionable by path depth, remaining depth, or rule50.
- **H2 (homogeneous response) — also rejected.** The response is not flat:
  global ε swings −37% to +335% on the gate object. The correct description
  is *trajectory chaos*: ε changes flip proof orderings chaotically and
  per-position, with no exploitable regional structure. Backlog #11's
  framing ("different regions want different ε") gets a data-backed no; the
  threshold surface is closed for constant *and* schedule knobs.

## Gate decision

Applying the gate table:

- **GO**: fails — no arm is ≥10% below baseline (best arm: 0.0%).
- **PARTIAL**: fails for arms (none below baseline); met only by the Phase 0
  global constant ε=0.375 (−8.7% stress, no control regression) if constants
  are admitted as candidates. Below the 10% bar.
- **NO-GO**: fires — no scheduling lever exists on this surface (the best
  arm matches, the firing arms regress), and the only strong global win
  (ε=0.5) carries control regressions.

**Decision: NO-GO on backlog #11 (scheduled ε), with a sized PARTIAL-band
spin-off**: ε=0.375 as a drop-in default-ε candidate, handed to `conversion`
(see Hand-offs). Per the plan's NO-GO consequence, the next research plan is
literature target #5 (child-level early termination in DF-PN).

## Hand-offs

- **`conversion` — sized note (spin-off, not #11):** global ε default change
  0.125 → 0.375. Evidence: Pareto improvement on all four measured cases
  (`measurements/plan4/plan4_derived.csv`); deterministic, soundness-neutral
  (outcomes unchanged; ε only moves the search trajectory). Sizing/open
  questions for a conversion plan: (a) effect in default mode *with* PV
  refinement rounds (`--first-outcome` was the measurement mode; the
  refine-cap rounds re-search under the new ε too); (b) the wider
  thorough-suite matrix under `benchmark --json`; (c) golden updates
  (`tests/m22_default_stdout_golden.txt`, trajectory goldens) and
  `make test-full`; (d) whether the non-monotonicity justifies a tiny
  per-case ε autotune instead of a fixed constant.
- **Next research plan:** literature target #5 (child-level early
  termination in DF-PN), elevated per the plan's NO-GO consequence.

## Deviations from the plan

- **Phase 0 child-eval capture**: the CLI prints no `child_evals`, so Phase 0
  runs used the same temporary env-gated stderr dump as plan1/plan3
  (`RESEARCH4_DUMP=1`, one line at the end of `solve_with_progress`). The
  dump was part of the single instrumentation pass; the plan's "no code
  changes in Phase 0" is replaced by the stronger spike-off bit-identity
  check (quick suite per-case identical to clean HEAD, 38,974,090 aggregate),
  which the plan required for Phase 1 anyway.
- **PV-length invariant**: plan4.md requires identical PV lengths across
  arms; this is unsatisfiable for any ε change that fires (ε moves the
  trajectory; `--first-outcome` keeps the first line found). Outcome +
  `pv_status` invariance was used as the soundness invariant instead; PV
  lengths are recorded per run in the raw dumps.
- **Phase 0 grid extension**: ε=0.375 and ε=1.0 were added beyond the
  specified grid {0, 0.0625, 0.125, 0.25, 0.5}, and the sweep was extended to
  dec13/dec10, because the gate decision hinged on control behavior of the
  global constants. dec10's grid-edge optimum (ε=1.0, −44.4%) is noted but
  not pursued (single-case, and ε=1.0 times out the stress case).
- **Revert mechanics**: the spike (one module, two call-site hooks, one
  temporary field, the dump) was removed and `src/` restored to HEAD;
  `git diff --exit-code src/` passes. The restore used `git checkout --` on
  the two hooked files (working-tree-only, no index/history writes) after
  first attempting edit-based reversion; noted for transparency against the
  read-only-git convention.
- **d_root m22 timeout**: plan4.md's "any divergence = spike bug" reading was
  investigated and resolved as a genuine timeout (search explosion under
  near-root coarsening), not a spike defect: spike-off bit-identity and the
  exact-equality behavior of the never-firing arms rule out instrumentation
  error, and the same arm finishes on the stress case's 300 s budget.

## Verification

- Spike-off bit-identity: quick suite per-case identical, aggregate
  38,974,090 (`measurements/plan4/quick_clean.json` vs
  `quick_spikeoff.json`).
- Baseline reproduction: Phase 0 ε=0.125 = 249,480,478 (stress, exact) and
  14,156,269 (m22, exact).
- Post-revert: `git diff --exit-code src/` clean; `cargo fmt --check` clean;
  `cargo clippy --release --all-targets` clean; `make test` all green
  (fast tier, ignored tests excluded).
- Post-revert stress run: win length 477, `pv_status: first-outcome`, and the
  8-chunk stderr `work_done` fingerprint identical to the instrumented
  spike-off run — Phase 0 numbers reproduce on clean HEAD
  (`measurements/plan4/postrevert_stress.*`).

## Next steps

1. `research` plan5: literature target #5 — child-level early termination in
   DF-PN (same churn mass, child granularity; per plan3/plan4 the frame- and
   threshold-granularity sides are now closed).
2. `conversion`: decide on the ε=0.375 spin-off note (default-mode + suite
   revalidation before any default change).
3. No further ε-constant or ε-schedule work under `research`: the threshold
   surface is response-flat to structure (H1) and chaotic (anti-H2); a
   revisit needs a new mechanism, not a new constant or schedule.
