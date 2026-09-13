# Report 4: Clock-pressure ordering — AND-side shuffle preference (plan4)

**Outcome: NO-GO.** The Phase 0 counter-only spike killed the lever on all
three go-bar criteria at once, on both the stress case and the m22 control,
in both first-outcome and default mode. No ordering term was implemented;
all spike code was reverted — the working tree is byte-identical to the
pre-plan4 state (commit `5b7a1b3`), and the fast gate is green. Backlog #3
is closed for the AND side, and the same measurement closes the OR-side
half as well (see *Backlog re-ranking* below).

## What was run

Phase 0 spike per the plan: temporary, env-gated (`PLAN4_SPIKE=1`)
counter instrumentation in a `spike_plan4.rs` module under `search/dfpn`,
with four hook sites (frame entry, post-`sort_moves` expansion, frame
exit, `evaluate_child` attribution), fully marked `// PLAN4 SPIKE` and
deleted after measuring. Definitions:

- *Expansion*: a `dfpn` frame that reaches `sort_moves` (frames exiting
  early via terminal/depth-0/path-repetition/TT/resolution-cache checks
  are never counted — those are exactly the exits no reorder can touch).
  `frames_or + frames_and ≈ nodes` on every run, so early-exit frames are
  a negligible fraction and the sampling is essentially complete.
- *Shuffle*: `!is_capture && moving piece ≠ Pawn` (the plan's
  classification; two integer compares, no board probes).
- *Rank*: the position of the frame's selected child
  (`store_best_move`) in the frame's final sorted order (TT promotion
  included) — the order the search actually tries children in. Recorded
  only for *proven* frames (`outcome_to_store` is `Some`), where a
  selected child is well-defined.
- *Direct evals*: `evaluate_child` calls attributed per frame depth
  (bucketed per depth so nested descendant frames never double-count into
  an ancestor), summed over high-clock AND frames.

Spike non-perturbation was verified: every instrumented run reproduced the
inherited baseline node/child-eval counts exactly (m22 858,117 /
14,156,269; stress FO 13,907,467 / 249,480,478; stress default 19,943,731
/ 338,094,183). Same-host pre-change baselines were captured first via the
`benchmark` example and matched the inherited post-plan9 numbers
(first-outcome 249,480,478 evals / 13,907,467 nodes / 54.2 s; default
338,094,183 / 19,943,731 / 72.0 s; m22 3.0 s).

## Measurements

### Stress case first-outcome (T = 60)

```text
nodes=13907467 child_evals=249480478 frames_or=2845274 frames_and=11054603
and_clock_hist(×10)=[10875873, 169644, 3693, 1636, 1336, 871, 453, 339, 294, 464, 0]
or_clock_hist(×10) =[2819549,  22592,  1115,  634,  509,  309, 188, 160, 103, 115, 0] or_high=566
and_high frames=1550 (0.01% of nodes) direct_evals=4029 (0.00% of child_evals)
composition zero=24 low(1-49%)=60 high(50-99%)=60 all=1406 moves=3451 shuffles=3192 (mean 0.925)
proven_high=153 (draw=0 loss=151 shuffle=150)
rank_all[0]=152 [1]=0 [2-4]=1 [5+]=0 nf=0
rank_shuffle[0]=150 [1]=0 [2-4]=0 [5+]=0 nf=0
```

### Stress case default mode (T = 60)

```text
nodes=19943731 child_evals=338094183 frames_or=3756157 frames_and=15870349
and_clock_hist(×10)=[15392154, 422076, 14370, 6715, 2178, 2950, 8205, 6028, 5997, 9676, 0]
and_high frames=29906 (0.15% of nodes, 0.19% of and-expansions) direct_evals=79523 (0.02% of child_evals)
composition zero=102 low=966 high=2973 all=25865 moves=73011 shuffles=66189 (mean 0.907)
proven_high=818 (draw=23 loss=785 shuffle=807)
rank_all[0]=817 [1]=0 [2-4]=1 [5+]=0 nf=0
rank_shuffle[0]=807 [1]=0 [2-4]=0 [5+]=0 nf=0
```

### m22 control, first-outcome (T = 60)

```text
nodes=858117 child_evals=14156269
and_high frames=87 (0.01% of nodes) direct_evals=454 (0.00% of child_evals)
composition zero=1 low=6 high=67 all=13 moves=424 shuffles=269 (mean 0.634)
proven_high=3 (loss=3 shuffle=2) rank_all[0]=2 [2-4]=1 rank_shuffle[0]=2
```

## Go/no-go bar: 0/3

1. **Refutation-shuffle rank mass at ≥ 2 — FAIL.** The selected child of a
   high-clock AND frame is already at rank 0 in 152/153 (FO) and 817/818
   (default) of proven frames; exactly one frame each had the selected
   child in ranks 2–4, none at 5+. History and killers already promote the
   relevant move to the top. There is no headroom above the existing
   heuristics for this signal class.
2. **Mixed shuffle composition — FAIL.** Mean shuffle fraction of legal
   moves at high-clock AND expansions is 0.925 (FO) / 0.907 (default);
   90.7% / 86.6% of those nodes have *only* shuffles. A uniform bonus over
   shuffles is a no-op on an all-shuffle move list, so the lever cannot
   even bite where the surface exists.
3. **≥ ~10% of stress-case node work — FAIL by 2–3 orders of magnitude.**
   High-clock AND frames are 0.011% of nodes (FO) / 0.15% (default), with
   directly-attributed child evals of 0.002% / 0.02%. Even a perfect
   reorder of those frames is bounded far below measurement noise.

The plan's "record the clock distribution so T can be re-picked" clause
resolves negatively too: AND expansions sit overwhelmingly at clock 0–9
(98.4% FO, 97.0% default). Clocks ≥ 20 cover 9,086 frames = 0.065% of FO
nodes; even lowering T to 20 leaves no surface. The searched tree rarely
sustains a high halfmove clock — the OR side's early-searched pawn pushes
and captures keep resetting it, so the "high-clock defender shuffle"
region of the game tree is nearly empty, and the plan's premise (defender
shuffles as the drawing resource concentrating disproving work at high
clocks) does not manifest as node work under this solver's exploration
order.

## Backlog re-ranking

- **Backlog #3, AND side: closed — measured no-go** (this report).
- **Backlog #3, OR side (clock-reset bonus): closed — the same measurement
  kills it.** The plan required the OR half to be "re-justified against
  this measurement". The spike's OR-side clock histogram (first-outcome
  stress run) shows 566 of 2,845,274 OR expansions (0.02%) at clock ≥ 60
  and 0.11% at clock ≥ 20. The bonus's trigger condition
  (`rule50 ≥ threshold`) almost never occurs in the searched tree, so the
  lever has no surface — independent of lean's separate oracle-floor
  argument against OR-side ordering quality.
- **DF-PN+ `H`/`Cost` clock-flavored frontier estimates: recommend closed.**
  Its premise (remaining clock budget meaningfully differentiating
  frontier work) contradicts the measured clock distribution, and it
  remains in the plan10 threshold-arithmetic hazard class. Reopening it
  would need new evidence that clock-heavy regions become dominant under
  some *other* lever's changed exploration order.
- Surviving conversion-initiative levers: #4 (parallel design spike, joint
  with `lean` #2) and #5 (reading round; plan3 in flight). The parked #2a
  (engine-seeded ordering guidance) is unaffected by this result.

## Notes for the `lean` #5 lane (AND-side ordering in general)

The one positive observation: at high-clock AND nodes the defender's
selected child is already served at rank 0 by history/killers *from move
one* in practice — the plan's worry ("history only pays after the first
re-proof") does not hold on the stress profile, because the 96%-re-proof
churn accumulates history scores precisely at the nodes that matter. Any
future AND-side ordering signal should be sized against this data first.

## Problems encountered

- Direct child-eval attribution needed per-depth bucketing
  (`eval_bucket`/`eval_base`): summing subtree deltas at frame exit
  double-counts nested frames (the same trap `research_repetition_cache.md`
  hit with interval-based proof-work metrics).
- A frame that exits before `sort_moves` must not consume a previous
  frame's snapshotted order at the same depth; the spike cleared the slot
  at frame entry and only recorded on sampled expansions.
- Minor and spike-only: an initial hook placement used `scratch_depth`
  before its binding; irrelevant after the revert.

## Unresolved parts / missing tests

- The OR-side clock histogram was collected only in first-outcome mode;
  the default-mode OR distribution is not captured. Given the FO numbers
  and the AND picture, this does not affect the decision.
- Rank histograms cover proven frames only; unproven/work-cut frames have
  no well-defined selected child and are unmeasured by construction.
- m22 default mode was not spiked (m22 FO surface is already ~0.01%).
- Since no production code was written, tasks 2–6 of the plan (term,
  unit tests, drift protocol, soundness gates, stress comparison) did not
  apply. Fast gate (`make test`) re-run green on the reverted tree; the
  repetition/move-order/plan6 `--include-ignored` suites were not re-run
  because the tree is byte-identical to the state they last passed in.

## Next steps

- Mark backlog #3 closed in `initiative.md` (done in the same change).
- plan5 becomes the next conversion plan; candidates are #4 (parallel
  spike) and the plan3 reading-round outputs.
- File the "history/killers already solve AND-side high-clock ordering"
  observation into the `lean` #5 lane notes so no future plan re-derives
  it the hard way.
