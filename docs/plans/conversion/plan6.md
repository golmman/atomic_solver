# Plan 6: Threshold-cut-frame pricing — Phase 0 diagnostic spike

Initiative: `conversion` backlog #6 (handed over from `dfpn` at its dormancy,
2026-09-17). Prerequisite reading: `lean/report9.md` (the M2/M5 accounting
that produced the 99.7–99.9% cut-frame figure this plan measures into),
`dfpn/report10.md` and `dfpn/report11.md` (the threshold-folding hazard and
its precise localization — every mechanism candidate here is tested against
them), `dfpn/research_repetition_cache.md` (the repetition-dominated work
profile: 96% re-proofs, cost in re-descent churn), `conversion/report1.md`
(the closed reuse-widening lane this plan must not re-enter), and
`src/search/dfpn/core.rs` (the `dfpn` frame loop whose exit sites the
instrumentation classifies).

**Scope decision (recorded per the plan2/plan4 precedent).** This plan is
**Phase 0 only**: a counter-only diagnostic spike, fully reverted after
measuring. No pricing mechanism is designed to a shippable bar inside this
plan. If the diagnostics clear the go bar, the mechanism plan opens as
`plan7.md`, drafted *from* this spike's data (working-agreement rule 2: one
lever per plan; the mechanism is a different lever than the measurement). If
it fails the bar, backlog #6 closes as a documented no-go and the
initiative's remaining levers are re-ranked on the evidence.

## Goal

The `lean` plan9 spike established the single largest structural fact about
where the solver's work goes: **99.7–99.9% of AND-frame own child evals sit
in threshold-cut frames** — frames that exhaust their DF-PN thresholds
without reaching a refutation exit (m22 8,625,570/8,645,613; stress case
148,390,289/148,602,798). The same accounting showed OR-side work is also
dominated by cut frames (130,560/156,596 frames on m22; 2,632,886/2,845,274
on the stress case). Cut frames never reach an outcome exit, so *no*
move-ordering signal can touch that mass — the `lean` plan9/plan4 spikes
measured the ordering surface empty from both sides. Reducing it means
changing how unsolved-subtree exploration is **priced**: the DF-PN threshold
arithmetic that decides how much work a frame is allowed to burn before
returning unsolved to its parent.

That arithmetic is the one part of the search no plan has yet dissected on
this class. The two structural predecessors both died for *folding* reasons,
not scheduling ones: plan10 folded solved **Win/Loss** facts into unsolved
parents' child thresholds and destabilized the m22 control (3.1 s → 120 s
timeout); the EWS/MOPNS readings closed because their threshold-targeting
mechanisms land in the same per-outcome sum/min folding hazard. What is
*not yet measured* is whether the cut-frame mass has an anatomy that a
pure **scheduling** lever — changing when and how thresholds grow, without
ever consulting a solved fact or widening any cache — can address.

Deliverable: a measurement report (`report6.md`) giving the anatomy of
threshold-cut frames on the stress case (first-outcome and default) and the
`m22_white` control, and a go/no-go verdict against the bars below. Primary
metric throughout: `child_evals` (per the initiative's conventions); wall
time secondary.

## Background — the frame-exit taxonomy this plan measures

The `dfpn` frame loop (`core.rs`) has exactly five exits after a child
sweep or re-evaluation:

1. **`solved`** — `selection.solved_outcome` fires (Win: one winning child;
   Loss/Draw: all children solved). The frame returns a decisive fact.
2. **`threshold-cut`** — `(th_pn != INF && pn >= th_pn) || (th_dn != INF &&
   dn >= th_dn)`: the frame's achieved bounds reached its thresholds without
   an outcome. It stores unsolved bounds and returns.
3. **`no-best-child`** — `mv == Move::NONE`: every child is `explored`,
   nothing left to descend.
4. **`work-cut`** — the `max_work` budget (a work chunk, the child-eval
   budget, or a refinement round cap) is exhausted mid-frame; the frame
   stores unsolved bounds and unwinds.
5. **`time-cut`** — `time_exceeded()`.

The root frame is called with `INF`/`INF` thresholds every work chunk
(`bounded_search`: `self.dfpn(pos, INF, INF, max_depth, call_max_work,
true)`), so the root never threshold-cuts; the entire cut-frame mass lives
in interior frames whose thresholds come from the DF-PN dynamics: at an OR
frame, the child threshold is `epsilon_ceil(second_child.pn)` and
`th_dn − dn + child_dn`; the AND side mirrors it. That arithmetic — *what
the parent is willing to spend on the best child before re-selecting* — is
the pricing knob. The `1 + ε` factor (`--epsilon`, default 0.5 via
`epsilon_ceil`) and the second-child bound are the only inputs.

Two hazard precedents constrain any mechanism that might come out of this:

- **plan10 hazard (no folding).** Solved facts must never enter unsolved
  parents' threshold arithmetic. A scheduling lever is in bounds only while
  thresholds remain functions of (bounds, thresholds, ε) alone — never of
  solved outcomes from the TT, the repetition cache, or any cross-path
  index.
- **plan1 lane (no reuse widening).** No new cache key, context, or
  adoption rule. The monotone-draw-cache and cross-clock surfaces are
  measured-closed; a pricing mechanism changes *when frames are cut*, not
  *what results are remembered*.

Candidate mechanism classes the diagnostics are shaped to discriminate
(each is a hypothesis to be evidenced, not a commitment):

- **(A) Chunk-boundary resumption.** `bounded_search` restarts the root
  frame with fresh `INF`/`INF` thresholds every chunk; each chunk re-walks
  the solved skeleton from the root before reaching the frontier. If a
  material share of cut-frame evals is "re-walking structure the previous
  chunk had already resolved in-run", the pricing defect is the reset, and
  the candidate is carrying threshold/skeleton state across chunks within a
  run (eval-count-based, deterministic).
- **(B) Threshold-growth shaping.** The child threshold is
  `epsilon_ceil(second_child bounds)`; if cut frames exit with a large gap
  between the exiting bound and the threshold (or between the best and
  second child's bounds), the next re-entry prices a large step. A capped
  or reshaped growth schedule (pure scheduling; thresholds stay functions
  of bounds/thresholds/ε) could cut re-entry churn.
- **(C) Per-frame work allocation.** `child_max_work =
  max_work.saturating_sub(work_spent)` gives every descent the full
  remaining budget. If cut frames' mass concentrates in a few deep frames
  repeatedly re-entered with the same best child, an allocation rule
  (smaller budgets for re-entries) could redirect the mass. Any such rule
  must keep the chunk-resume semantics (`unsolved` stores) exactly as
  documented.

## Phase 0 — diagnostic spike (go/no-go, then close or hand to plan7)

Counter-only, env-gated instrumentation; **zero production behavior change**
(default off ⇒ runs and tests bit-identical); fully reverted after
measuring, per the initiative's "measure before planning" cadence (plan1/
plan4/plan10/plan11 pattern).

### Step 0 — baseline re-verification (no instrumentation yet)

The inherited baselines are `dfpn` post-plan9 numbers
(`dfpn/initiative.md` Measurement conventions): stress first-outcome
249,480,478 child evals / 13,907,467 nodes / ~53.8 s; default mode
338,094,183 / 19,943,731 / ~72.3 s. `dfpn` plans 12–13 (repetition-cache
trajectory, bounded refinement `--refine-cap`, default 0.25) landed after
those numbers were recorded, so:

```bash
cargo run --release -- --fen '<STRESS_FEN>' --timeout 120 --first-outcome --outcome-only
cargo run --release -- --fen '<STRESS_FEN>' --timeout 120
cargo run --release -- --fen '<M22_FEN>'   --timeout 30  --first-outcome --outcome-only
```

(`<STRESS_FEN>` = `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`
— `make stress`; `<M22_FEN>` = `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/
1R4RK w - - 0 22`.) Record the HEAD numbers in `report6.md`. If the
first-outcome baseline reproduces 249,480,478 exactly, all inherited
cross-references stand. If it drifted (post-plan9 `dfpn` changes), the HEAD
numbers become the plan's baseline — noted explicitly, with the lean
report9 *ratios* (99.7–99.9% cut share) treated as re-verifiable, not
literal. Use generous `--timeout` so no run is resource-cut (the metrics
are eval counts; a time-cut invalidates the run).

### Instrumentation (env-gated `CONV6_SPIKE=1`, default off)

Pattern: `dfpn` report10/11 — a temporary `src/search/dfpn/spike6.rs`
module holding a stats struct on `Search`, counters written at existing
decision points, a temporary `examples/solve_stats.rs` runner, and
kill-switch env vars (`CONV6_SPIKE_OFF=1` for the identity check). With
`CONV6_SPIKE` unset (or `=0`) the binary must be bit-identical to HEAD —
verified by reproducing the step-0 baselines exactly before any measurement
(the plan1/plan4/plan11 anchor protocol).

Instrumentation sites and counters:

1. **Frame exit classification** (`core.rs`, at each of the five exit
   points): record `(is_or_node, depth, exit_reason)`, plus at
   threshold-cut exits the anatomy — which threshold fired (pn/dn), the gap
   `bound − threshold` for both, and the best child's bounds vs the
   second child's bounds (the ε step the next re-entry would price). Extend
   `lean` report9's per-frame own-eval accounting (each `evaluate_child`
   call attributed to exactly one enclosing frame slice; the partition must
   sum to the exact `child_evals` total — delta 0 is the attribution
   correctness check, as in report9).
2. **Per-position lifetime map** (keyed by `tt_key`, on `Search`, unbounded
   in the spike — one diagnostic run, RAM documented): per position, total
   own evals across all frame entries, entry count, per-exit-reason eval
   sums, and a stability signature for the best child (e.g. hash of the
   best-move sequence across re-entries). Answers: is the cut mass
   *churn* (few positions re-entered many times) or *frontier* (many
   positions entered once or twice)?
3. **Chunk-boundary waste** (`bounded_search`): per chunk, total evals vs
   evals spent in the final partial chunk; plus the count/evals of
   re-evaluations of children whose `(pn, dn)` returned unchanged (the
   existing `explored`-marking path in the re-eval branch is the natural
   counter site). Answers candidate (A)'s surface.
4. **ε-sensitivity at cut exits**: for each threshold-cut, the ratio
   `second_child_bound / best_child_bound` (per fired threshold) and the
   histogram of `epsilon_ceil(second) − bound`. Answers whether (B)'s
   shaping has room or whether frames exit *at* the threshold (gap ≈ 0),
   i.e. ε is not the pacing factor.
5. **OR/AND split** of every counter above — the mechanism-scope decision
   needs both sides (report9's OR cut frames: 83–92% of OR frames).

All spike data goes to stderr or a temp file at run end (the runner
aggregates; the search stays bit-identical in *decisions* — counters are
side-effect-free reads of state already computed).

### Runs

- Stress first-outcome and default (HEAD baselines from step 0).
- `m22_white` control (FO; the ~3.1 s / 14,156,269-eval case) — the
  destabilization control in the plan10/11 lineage.
- `m24_white` (`4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23`,
  the report9 third case, cheap) as a small-class sanity point.

### Go/no-go bars

**GO** (→ draft `plan7.md` for exactly one mechanism, chosen by the data)
iff all of:

1. At least one candidate class (A/B/C — or a class the diagnostics
   suggest that still passes the constraints below) has a **measured
   addressable surface ≥ 10% of stress first-outcome child evals**
   (measured directly by the counters, not extrapolated) — comfortably
   above the ≥5% bar plan1 set for cache-surface levers, since a pricing
   lever carries more design risk.
2. The mechanism sketch passes the **plan10 hazard test on paper**: its
   thresholds/bounds are functions of (bounds, thresholds, ε) only — no
   solved-fact folding, no reuse-widening (plan1 lane), no
   repetition-semantics change.
3. The mechanism is eval-count-deterministic (no wall-clock input), so the
   drift protocol and the budget contract (`child_eval_budget` /
   `ExitReason::BudgetExhausted`) stay intact.
4. The `m22_white` control shows the same targeted surface exists (even if
   smaller) — i.e. the lever is not stress-only in a way that invites the
   plan10-style control destabilization.

**NO-GO** otherwise: revert everything, close backlog #6 in
`initiative.md` with the anatomy tables as the closing evidence, update the
`docs/plans/README.md` `conversion` row (the initiative's last open lever
is then #4 parallel, jointly owned with `lean` #2 — the dormancy question
that decision forces is recorded in the report, not acted on here).

Honest prior: the structural levers around this surface (plan1, plan10,
plan11, EWS, MOPNS, journal-GHI) all closed no-go, and (B) in its
df-MOPNS-threshold-targeting form is already in the hazard class — the
legitimate (B) residue is only the second-child/ε *schedule*, not
per-outcome threshold targeting. The plan's value is symmetric: a measured
go produces the first structural lever since plan9; a no-go exhausts the
diagnostic space and clears the way for reopening the parked parallel spike
(`conversion` #4 / `lean` #2) with data.

## Constraints (normative, inherited from the initiative)

- Working agreements 1–5 apply: measure before planning; one lever (Phase 0
  only, per the scope decision); two-sided validation (moot here — no
  production change — but the spike-off identity check stands in for the
  drift protocol); the deterministic budget contract untouched; every
  reuse-rule widening forbidden outright (this plan adds none).
- Spike code never reads the wall clock into search decisions, emits no
  `ProofEvent`s, and does not touch the TT, the repetition cache, or
  preflight.
- The spike's per-position map is unbounded by design (diagnostic only);
  the plan must note that this is spike-RAM, not a search-CLI contract
  change.
- All spike code is reverted at the end regardless of verdict;
  `git status` clean, tree byte-identical to HEAD, fast gate (`make test`)
  green before and after.

## Deliverable

`report6.md` in this directory: step-0 baselines at HEAD, the anatomy
tables (exit-class eval partition with delta-0 check, cut-gap and
second-child histograms, churn-vs-frontier distribution, OR/AND split,
chunk-waste numbers), the go/no-go verdict against the bars above, and — on
a go — the mechanism design brief that `plan7.md` is drafted from (with the
hazard-test argument written out). On no-go, the closure entry for backlog
#6 and the re-ranked initiative levers.
