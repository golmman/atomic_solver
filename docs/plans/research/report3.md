# Plan 3 Report: Subgame material-mass diagnostic (mid-search recognizer feasibility)

## Summary

Temporary instrumentation (plan1's pattern: plain `u64` counter arrays on
`Search`, incremented unconditionally, dumped to stderr once at the end of
`solve_with_progress`) classified every `dfpn` frame by the men count of the
frame's own position, accumulated each frame's descendant child-eval delta
into its bucket plus the threshold-cut slice, and counted frames matching the
plan13 detector shape widened to ≤5 men (pawnless, no castling) — the
invocation surface a mid-search region-closure recognizer would act on. Three
cases were measured in first-outcome mode, the instrumentation was reverted,
and the quick suite was verified bit-identical before and after.

**Result: NO-GO.** The searched tree of material-rich roots does not
simplify. Across all three cases **not a single `dfpn` frame below 6 men was
ever entered** — buckets 0–1 (≤5 men) and the harvestable counter are exactly
zero everywhere — and 99.93–99.97% of frame-eval mass sits at ≥9 men. H1
(simplification) is rejected; H2 (material-rich) is confirmed, and more
strongly than its ≥90% prediction. The mid-search recognizer direction is
closed for the outlier family with data.

## Raw dumps

Verbatim stderr dumps (search output elided in the tables below; full files
under `measurements/plan3/`):

### m22_white (`4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`, timeout 30)

```
  child_evals         = 14156269
  men_buckets         = [1-3, 4-5, 6-8, 9-12, 13-16, 17+]
  frame_bucket_counts = [0, 0, 532, 50618, 481235, 325732]
  frame_bucket_evals  = [0, 0, 69201, 2840730, 52429293, 77687320]
  frame_bucket_cuts   = [0, 0, 38721, 2371005, 50214239, 54200104]
  frame_harv_counts   = 0
  frame_harv_evals    = 0
```

Run: 3.0 s, outcome win length 95, `pv_status: first-outcome`.

### Stress case = m21_white (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`, timeout 300)

```
  child_evals         = 249480478
  men_buckets         = [1-3, 4-5, 6-8, 9-12, 13-16, 17+]
  frame_bucket_counts = [0, 0, 3113, 169454, 3539905, 10194995]
  frame_bucket_evals  = [0, 0, 1597438, 24898387, 253801013, 1874942230]
  frame_bucket_cuts   = [0, 0, 481518, 20537963, 245154414, 1485680828]
  frame_harv_counts   = 0
  frame_harv_evals    = 0
```

Run: 53.7 s (well under the 300 s bound, so no timeout cut contaminates the
splits), outcome win length 477, `pv_status: first-outcome`.

### dec13 (rich-middlegame control, castling rights KQ; timeout 30)

`r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14`

```
  child_evals         = 3822602
  men_buckets         = [1-3, 4-5, 6-8, 9-12, 13-16, 17+]
  frame_bucket_counts = [0, 0, 0, 0, 356, 164524]
  frame_bucket_evals  = [0, 0, 0, 0, 9873, 32397615]
  frame_bucket_cuts   = [0, 0, 0, 0, 9873, 25667353]
  frame_harv_counts   = 0
  frame_harv_evals    = 0
```

Run: 0.87 s, outcome win length 17, `pv_status: first-outcome`.

Derived per-bucket and cumulative-≤k share tables (counts and evals) are in
`measurements/plan3/derived_tables.md`.

## Sanity invariants

All plan-specified invariants hold bit-for-bit against plan1's frame-level
accounting (see the table in `derived_tables.md`):

- Σ per-bucket frame counts = plan1 `frame_total`: 858,117 (m22),
  13,907,467 (stress).
- Σ per-bucket frame evals = plan1's solved+cut+budget frame-eval total:
  133,026,544 (m22), 2,155,239,068 (stress).
- Σ per-bucket cut evals = plan1 `frame_cut_evals`: 106,824,069 (m22),
  1,751,854,723 (stress) — confirming the instrumentation reproduces plan1's
  exit-site classification exactly.
- First-outcome `child_evals` match the post-plan9 baselines:
  14,156,269 (m22), 249,480,478 (stress).

## Hypothesis verdict

**H1 (simplification) — rejected.** The gate requires ≥10% of `frame_evals`
in ≤5-men subtrees (or ≥10% harvestable). Measured: 0.000% and 0 on every
case. The search never even *creates* a ≤5-men frame: buckets 0–1 have zero
frame entries on all three cases, so a mid-search recognizer spike would have
zero invocation sites regardless of its budget or soundness contract.

**H2 (material-rich) — confirmed, beyond its own prediction.** H2 predicted
≥90% of eval mass at ≥9 men; measured 99.926% (stress), 99.948% (m22),
99.970% (dec13). The dominant bucket is 17+ men (58.4% m22, 87.0% stress,
99.97% dec13); even the 6–8-men bucket — the PARTIAL gate's candidate width —
holds only 0.074% (stress) / 0.052% (m22) of eval mass.

The plan's counter-evidence concern is resolved: the principal line's tempo
conversion (long capture-free stretches) is not an outlier — the *searched
tree* mirrors it. Although atomic blasts make men count monotone non-increasing
down any path, captures are evidently rare in the searched mass: the
threshold-cut churn (79–81% of frame evals, consistent with plan1) lives
almost entirely in 13–17+ men frames where the position is still material-rich
and a region closure of any width is infeasible.

## Gate decision

| Gate | Criterion (stress, eval-weighted) | Measured | Verdict |
|---|---|---|---|
| GO | ≥10% of `frame_evals` in buckets 0–1, or harvestable ≥10% | 0.000% / 0 | **not met** |
| PARTIAL | dominant small-material mass in 6–8 men | 0.074% (not dominant) | **not met** |
| NO-GO | ≥90% in buckets 3–5 and harvestable negligible | 99.926%, 0 | **met** |

**Decision: NO-GO.** Backlog #3 is closed with data: the recognizer direction
(region-closure fixpoint widened into mid-search) is dead for the outlier
family — not merely unwired, but with no invocation sites and no material
density to act on. m22 (the regression control) and dec13 agree with the gate
object (stress).

## Hand-off record

Per the plan's NO-GO consequence, the next plan falls back to **#11
(depth-scheduled ε POC)**, with the documented caveat: global ε ∈
{0, 0.125, 0.5} was measured inert on the stress case (2026-09-11,
pre-plan9), but plan11 showed threshold-arithmetic changes are high-leverage
(−37.7% and +110% arms). The untested variant is *scheduling* ε (or the
refinement cap) by depth or rule50 clock rather than globally; the sizing
question stays 1 session, flag-gated, likely owner `conversion`. No reopening
of `dfpn` is warranted by this plan: trigger (a) (a measured search-semantics
diagnostic for the class) was the GO-path hand-off and did not fire. The
PARTIAL-gate consequence (literature target #7, mating-net recognizers, rising
on the backlog) is likewise not triggered, since the 6–8-men width is empty;
#7 remains open at its existing priority.

## Drift check and revert verification

- Quick-suite drift: `benchmark --suite quick --json --first-outcome`
  aggregate `child_evals` 38,974,090 instrumented vs 38,974,090 clean;
  **0/59 per-case mismatches** (clean baseline captured before
  instrumentation; the instrumentation does not perturb the trajectory —
  plan1's precedent reproduced).
- Revert: all counter fields, increments, the bucket classifier, and the
  dump call removed from `src/`.
- `git diff --exit-code src/` — clean.
- `cargo fmt --check` — clean. `cargo clippy --release --all-targets` — no
  warnings. `make test` (fast gate) — all tests pass.

## Problems encountered

- **Solved-frame accounting subtlety.** Solved frames exit through the main
  post-loop exit (not an early return), so the initial cut-slice condition
  (`!frame_budget_cut`) wrongly absorbed solved frames' eval deltas. The
  plan1 cross-check caught it immediately (Σ cuts came out 121.1M vs plan1's
  106,824,069 on m22 — the excess was exactly plan1's `frame_solved_evals`,
  14.3M); fixed by conditioning the cut slice on `outcome_to_store.is_none()`
  as well, after which all invariants matched exactly. Lesson: the plan1
  invariants are load-bearing — compute them mid-session, not post-hoc.
- `board.occupied().count()` returns `u32` (bitboard popcount), not `usize`;
  the bucket classifier needed the matching signature. Cosmetic only.
- The plan's bucket map starts at 1 man; 2-men frames (K vs K) are reachable
  via the solver's `occupied == 2` draw rule, so bucket b0 was defined as
  0–3 men. No such frames occurred in the measured cases.

## Files

- `measurements/plan3/plan3_m22_instrumented.txt` — raw stderr, m22_white
- `measurements/plan3/plan3_stress_instrumented.txt` — raw stderr, stress case
- `measurements/plan3/plan3_dec13_instrumented.txt` — raw stderr, dec13 control
- `measurements/plan3/derived_tables.md` — per-bucket + cumulative share tables
- `/tmp/opencode/plan3_quick_clean.json`, `/tmp/opencode/plan3_quick_instrumented.json` — drift-check JSONs (session-local, aggregate recorded above)
