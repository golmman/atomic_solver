# report13 — plan13 (item #19 first arms: ε = 0.375 / 0.5 re-score under the pinned gate)

Date: 2026-10-11. Initiative: `research`. Evidence:
[`measurements/plan13/`](measurements/plan13/) (396 runs),
[`gate_methodology.md`](gate_methodology.md) (pinned v1.0, first real-arm
application).

## Deliverables

1. **plan13 executed as pre-registered.** No `src/` changes (product
   binary reused, `git status` clean apart from `docs/`); each arm differs
   from the shipped baseline by the single `--epsilon` option (D2).
   22 cases (frozen plan12 corpus) × 6 salts × 3 arms = **396 runs**,
   7228 s CPU-serial (~120 min, close to the 2.5 h envelope), max single
   run 458 s, ~50 min wall at 3-way concurrency. The baseline arm was run
   fresh, never copied from plan12.
2. **All D3 soundness invariants pass — no HALT.**
   - (a) *Salt-0 identity*: the fresh baseline reproduces the
     plan11/plan12 recorded values exactly (stress 249,480,478; m22
     14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20
     censored at 2 B).
   - (b) *Per-salt identity*: the fresh baseline arm equals plan12's
     recorded per-salt value (evals **and** censoring status) on all
     22 × 6 cells — a 132-cell cross-session bit-identity, the strongest
     identity check yet.
   - (c) *Outcomes*: zero decisive-outcome conflicts within any arm
     across salts; every uncensored run of every arm matches its fixture
     expectation (ε changes work, never the proven value).
   - (d) No censored run enters any outcome set (budget-censored draw =
     no-result sentinel, enforced structurally in `parse.py`/`verdicts.py`).
3. **D4 verdict machinery applied verbatim** (`verdicts.py` →
   `state/verdicts.json`): per-salt paired ratios over both-uncensored
   cells, censoring patterns, baseline basin counts (from plan12's
   `calibration.json`), under-budget rule, low-diversity clause, and the
   pinned W_LO/W_HI + ≥ 75 % direction-agreement rule.
4. **Both arms: lever verdict = reject.** The ε re-score of item #19
   resolves negatively for global ε = 0.375 / 0.5 as shipped-default
   replacements; per D6 there is **no hand-off** (reject arms are recorded
   with their tables below). plan4's single-draw readings are superseded
   at gate grade.
5. **First real-arm calibration of the pinned comparison rule recorded**
   (dispersion summary below). No methodology revision is proposed; the
   standing rule (any W_LO/W_HI change needs stated data) is respected —
   this report only adds data.
6. `docs/plans/README.md` research row **not** refreshed: per plan13
   Phase 3 that happens only on an adopt hand-off or a methodology
   revision, neither of which resulted. The row's "plan13 next" text is
   now historically stale; the next session touching the #19 thread
   should fold the refresh into its own index update.

## Lever verdicts and per-case tables

Basin counts are the **baseline's** (plan12); "cens" marks a
budget-censored cell at the case cap. Ratios are per-salt
candidate/baseline over both-uncensored cells.

### Arm e375 (ε = 0.375) — **reject**

| case | verdict | basins | censoring (base vs cand) | paired ratios |
| --- | --- | --- | --- | --- |
| stress | **win** | 3 | base cens s2/s3/s4; cand cens s5 | 0.150 (s1), 0.913 (s0) |
| m20_white | unresolved (under-budget) | 4 | both cens s0/s1; cand cens s3/s5 | 0.298 (s2), 0.537 (s4) |
| m22_white | **censored-loss** | 6 | cand cens s3 | 0.67–1.41 (5 pairs) |
| m23_white | **censored-loss** | 5 | cand cens s0/s5 | 0.526–0.721 (4 pairs) |
| dec01 | **regression** | 6 | none | 1.32, 9.78, 22.2, 41.1, 47.4, **95.3** |
| dec03 | **regression** | 1 | none | 1.189 ×6 (exact) |
| dec05 | **regression** | 1 | none | 5.91 ×6 |
| dec12 | **regression** | 1 | none | 1.117 ×6 |
| dec14 | **regression** | 3 | none | 1.69–1.77 |
| dec02/04/06/08/09/10/11/17/18 | unclear | 1–4 | none | 0.93–1.27 |
| dec07/15/16 | win (not hard class) | 1 | none | 0.67–0.85 |
| dec13 | win (not hard class) | 2 | none | 0.78–1.03 |

Reject drivers: 5 regressions + 2 censored-losses. The stress **win** is
real but fragile: it rests on 2 paired salts (median 0.53), and the
arm's salt-5 trajectory *fails entirely* at 2.5 B where the baseline
solves in 252 M. The m23 rows are the sharpest illustration of the
censoring asymmetry (see below): e375 *improves* m23 on all four
uncensored salts (0.53–0.72×) yet its two censored cells force a
censored-loss.

### Arm e5 (ε = 0.5) — **reject**

| case | verdict | basins | censoring (base vs cand) | paired ratios |
| --- | --- | --- | --- | --- |
| stress | **censored-win** | 3 | base cens s2/s3/s4, all solved by cand | 0.362 (s1), 0.627 (s0), 1.472 (s5) |
| m20_white | unresolved (under-budget) | 4 | base cens s0 (cand **solves s0 at 389 M**); cand cens s1/s2/s3/s4/s5 | none |
| m22_white | unclear | 6 | none | 0.594–2.077 |
| m23_white | **censored-loss** | 5 | cand cens s4 | 0.335–0.941 (5 pairs) |
| dec01 | **regression** | 6 | none | 0.96–1.40 |
| dec02 | **regression** | 1 | none | 1.277 ×6 |
| dec03 | **regression** | 1 | none | 1.400 ×6 |
| dec05 | **regression** | 1 | none | 1.999 ×6 |
| dec11 | **regression** | 1 | none | 2.939 ×6 |
| dec12 | **regression** | 1 | none | 1.733 ×6 |
| dec13 | **regression** | 2 | none | 1.14–1.52 |
| dec15 | **regression** | 1 | none | 1.385–1.400 |
| dec04/06/08/09/10/18 | unclear | 1–4 | none | 0.96–1.78 |
| dec07/14/16/17 | win (not hard class) | 1–3 | none | 0.71–0.89 |

Reject drivers: 8 regressions + 1 censored-loss. The stress
**censored-win** is the strongest single result of the plan (solves all
three baseline-censored trajectories, 0.36–0.63× on two of three
both-uncensored salts), and e5 even *solves m20 salt 0* (389 M) where the
baseline is censored — but m20 is under-budget for e5 (cand censored on
5 of 6 salts) and therefore contributes nothing, and the broad control
regressions veto the arm regardless.

### Contrast with plan4's single-draw readings (superseded)

| plan4 claim (single draw) | plan13 verdict (6 paired salts) |
| --- | --- |
| ε = 0.375 "Pareto-improves all four cases" | **false** — dec01 explodes (up to 95×), m22/m23 censored-loss |
| ε = 0.375 dec10 −14.5 % | unclear (0.76–1.25, direction-mixed) |
| ε = 0.5 "wins stress −37 %" | **confirmed, stronger**: censored-win |
| ε = 0.5 "regresses m22 +12.7 %" | unclear (0.59–2.08, direction-mixed) |
| ε = 0.5 "regresses dec13 +52 %" | confirmed: regression 1.14–1.52 |

## Paired-ratio dispersion — first methodology calibration data point

Over all both-uncensored salt pairs per arm (the pinned rule's first
observation on real lever arms):

| arm | n | min | p25 | median | p75 | p90 | max | frac outside [0.9, 1.1] |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| e375 | 121 | 0.150 | 0.850 | 1.000 | 1.117 | 1.735 | **95.29** | 56 % |
| e5 | 122 | 0.335 | 0.886 | 1.025 | 1.385 | 1.779 | 2.939 | 70 % |

Reading: on the 14 salt-invariant cases these ratios are *exact*
(deterministic, single draw), so the dispersion is real lever effect, not
noise — consistent with the pinned rule's degenerate-null premise (the
null sits at 1.0; thresholds guard direction-mixed effects, and the
≥ 75 % agreement clause did its job, e.g. m22 e375's median-drag case).
The methodology's W_LO/W_HI require no revision from this data. What the
data *does* expose is a structural property the rule already encodes but
that is now observed at scale:

- **Censoring cells dominate lever verdicts.** Both arms' rejections were
  driven by regressions, but the *hard-class picture* is distorted by
  budget-cap interactions: m23 is improved 0.33–0.94× on 10 of its 12
  across-arm paired salts, yet three censored cells (e375 s0/s5, e5 s4)
  force censored-losses; m20 is unresolved for both arms because the
  ε trajectories *redistribute* which salts exceed the 2 B cap. A future
  methodology revision candidate (needs its own stated data, per the
  standing rule) is distinguishing a censored-loss on an
  otherwise-improving case from one on an otherwise-losing case.
- The low-diversity clause was not load-bearing this time: no hard-class
  case has a baseline basin count < 3 (stress 3, m20 4, m22 6, m23 5).

## Problems encountered

- Trivial: the first `parse.py` run failed on a missing `state/` dir
  (created; no data impact).
- No HALT, no D3 violation, no rerun needed anywhere — the driver was
  fully resumable and never exercised beyond the initial pass.
- `verdicts.py` reads plan12's committed `state/calibration.json` for
  baseline basin counts and `parse.py` reads plan12's
  `state/summary.json` for the D3(b) reference values; both are committed
  artifacts, so the pipeline stays deterministic and self-contained.
- Interpretive caveat (not a machinery bug): e375's stress "win" rests on
  2 paired ratios with a third trajectory censored — the pinned win rule
  is satisfied, but the report table flags the censoring so the win is
  not over-read.

## Missing tests / caveats

- **m20 × ε arms is unresolved** at the pinned caps (e5 censored on 5/6
  salts). Resolving it would need a pre-registered cap extension (per the
  methodology, raise the cap or move the case to "unresolved" — we chose
  the latter). Given both arms are already rejected on other cases, a cap
  extension was not worth the budget this round.
- The **mechanism** of the dec01 e375 blow-up (5.7 M → up to 544 M evals,
  95× on salt 5) is not diagnosed; ε affects DF-PN+ threshold refinement,
  and dec01 is a deep decisive control. Out of scope here (no `src/`
  changes), but it is the single most informative lever trace from this
  plan if the ε surface is ever revisited.
- The methodological observation on censoring-dominated verdicts (above)
  is recorded, not acted on.
- `make test` not run: plan13 makes **no `src/` changes** (verified via
  `git status`; only `docs/` artifacts changed).

## Gate result

**Reject (both arms) — no hand-off.** The #19 ε re-score legs are closed
negatively at gate grade: global ε = 0.375 and ε = 0.5 are not adopted as
shipped-default replacements. The remaining #19 closures (clock-budget
solved-entry reuse, `lean` plan10 history/killer arms, TT-eviction arm V2)
are untouched, as are #18/#20/#21 and plan14 (#23).

SESSION COMPLETE
- plan13 executed: 396-run 3-arm rollout measured
  (measurements/plan13/, all D3 gates green incl. the 132-cell per-salt
  baseline identity), D4 verdicts computed (e375: reject; e5: reject),
  dispersion recorded as the pinned rule's first calibration data point,
  measurements/plan13/README.md + report13.md written, docs/plans/README.md
  row intentionally not refreshed (no adopt hand-off / methodology revision,
  per plan13 Phase 3)
Follow-up options:
  1. "Execute docs/plans/research/plan14.md" — the #23 tie-break channel
     arm under the pinned gate; it is independent of #19 and its plan is
     already drafted. Reason: keeps the reopened program moving on the
     next pre-registered lever while #19's remaining arms await batching
     preference.
  2. Alternative: draft + execute the next #19 re-score arm (dfpn
     clock-budget reuse, `lean` history/killer, or TT-eviction V2) —
     pick this if closing the §1 unjudged closures takes priority over
     #23; each needs its own one-discovery plan per the working agreement.
