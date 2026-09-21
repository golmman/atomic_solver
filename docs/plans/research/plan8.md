# Plan 8: POC #14 — per-node confidence-conditioned ε (separation study, conditional trajectory POC)

Initiative: `research`, new POC candidate **#14** (row added to the backlog
when this plan opens). This is the documented successor of plan7
(`report7.md`): with the node-count program measured out, report7's
next-steps asked whether any open question remains that a one-session POC
can still decide. This plan admits one — under the explicit escape clause
plan4 wrote into the ε surface's closure:

> "No further ε-constant or ε-schedule work under `research`: the threshold
> surface is response-flat to structure (H1) and chaotic (anti-H2); **a
> revisit needs a new mechanism**, not a new constant or schedule."
> — `report4.md`, Next steps.

**Why this is a new mechanism, not a re-run.** plan4 closed ε as a function
of *path state* (depth, remaining depth, rule50 clock, linear schedules):
global functions of where the search is. This plan tests ε as a function of
*local node information read at the threshold-computation site* — the same
place the 1+ε trick already consumes the best/second-child comparison. The
hypothesis is that the rescue-vs-waste mix of threshold padding (aborting a
good child mid-inflation vs. digging into a dead one) is *predictable from
features observable at the node*, which no path schedule can express. The
prior is weak — plan4's chaos finding caps all trajectory shaping, and
plan6's class-B data (below) shows the natural df-pn-native signal is
degenerate on the hot class — which is why the plan is separation-study
first, with a kill gate before any behavior-changing arm.

**The idea.** At an OR frame, the threshold for the best child is
`min(th_pn, epsilon_ceil(second_pn))` (`src/search/dfpn/core.rs`); the AND
side mirrors it on `dn`. The padding is headroom for mid-search proof-number
inflation plus insurance against pessimistic estimates. Under the trust
model discussed for the ε default (confidence that the over-budget child is
genuinely good ⇒ pad more; confidence it is genuinely dead ⇒ pad less), a
*per-node* ε — high-confidence nodes padded more, low-confidence nodes less
— could beat any global constant. Soundness is unaffected by construction:
ε only shapes the trajectory, never a proof's validity.

Per the working agreement: one idea, throwaway instrumentation, `src/`
changes reverted before the report, no production hand-off from this plan
(a GO produces a hand-off item to `conversion`).

## Goal

Answer candidate #14 with data: **is the rescue-vs-waste outcome of a
threshold-cut frame separable by cheaply observable node-local features —
and if so, does a confidence-conditioned per-node ε convert that separation
into a first-outcome `child_evals` win over the best global constant?**

Phase 0 is a counter-only separation study (no behavior change) that can
close the *entire idea class* on its own: if no observable feature separates
"transient peak on a good child" from "genuine dead end", no confidence
function can exist, and the ε surface closes with a fourth leg. Phase 1
(only if the kill gate fails) runs a small env-gated arm table against the
**ε=0.375 baseline** — not the shipped 0.125 default — because the question
is "does conditioning beat the best global constant", and plan4 already
measured that constant's numbers.

## Context

Six prior results shape this plan:

1. **plan4 (backlog #11) — the ε surface and the baseline to beat.** No
   path-level schedule beat the global default (best arm 0.0%; firing arms
   regressed up to +351% and timed out m22); the response is trajectory
   chaos with no regional structure. The Phase 0 grid found global ε=0.375
   Pareto-improving: stress FO 227,834,433 (−8.7% vs the 249,480,478
   ε=0.125 baseline), m22 12,351,299 (−12.7%), dec13 3,921,011 (+2.6%),
   dec10 3,642,166 (−14.5%) — handed to `conversion` as a pending
   default-ε candidate. This plan's Phase 1 gate object is the 0.375
   stress number; if `conversion` adopts the default in the meantime, the
   baseline rebases to the shipped default and the gate is restated
   against it.
2. **plan6 class B — the natural signal is degenerate on the hot class.**
   At threshold-cut frames (~99% of stress FO child evals, mostly AND
   refutation frames), the second/best ratio sits in [1, 1.25) for 97.8%
   of AND cuts (88.1% OR); `epsilon_ceil(second) − best = 1` at 93.5% of
   AND cut exits (81.0% OR); the parent clamp (`ε(second) ≥ th`, where the
   padding does not engage at all) fires on 32.1% of OR cuts and 6.7% of
   AND cuts. I.e. at the frames where the work lives, best and second are
   statistically tied and the schedule already grows thresholds by the
   minimum increment. The df-pn-native confidence signal (the gap) reads
   "no confidence" almost everywhere on the hot mass. The scheme can still
   win — at a tie, tight thresholds are arguably *correct* (children
   interchangeable, switching cheap), so the candidate bet is that the tie
   region deserves ε→small while rare high-gap frames deserve padding —
   but that bet reduces ε *below* default on the hot class, the opposite
   direction of the measured 0.375 win. Phase 0 must measure, not assume.
3. **The trust model (discussion record, not a report).** The padding's
   cost is wasted digging into dead children; its benefit is not aborting
   a line that would have resolved (mid-search pn inflation is mechanical:
   expanding an AND child *sums* its descendants' numbers before they
   collapse, so even perfectly-estimated children transiently read
   expensive). The sign of confidence→ε *flips by node type*: OR-side
   confidence in the child ⇒ pad more; AND-side confidence that no
   refutation exists ⇒ pad less (abandon doomed defenses fast). The AND
   side is where 98.9% of the cut mass sits (conversion plan6 Phase 0), so
   the exploitable surface is mostly AND-side with inverted mapping.
4. **`lean` plan9 — ordering is at its oracle floor.** There is no
   "better ordering" left to buy confidence from; any confidence signal
   must be read off the search's own dynamic numbers (or the already-
   computed static scores in the frame's child slots), not from improved
   move ordering.
5. **plan1's partition.** ~80% of frame evals sit in threshold-cut frames.
   The rescue/waste mix inside exactly those frames is the entire
   exploitable surface for this candidate; everything outside is invisible
   to ε.
6. **Soundness and contracts.** ε never enters stored TT state (thresholds
   are not cached; entries hold path-independent pn/dn bounds), so per-node
   ε creates no GHI/TT hazard. Phase 1 must not add unbounded per-node
   history (RAM = TT only); the design needs no new state beyond the
   existing frame data. Phase 0's outcome-keyed event log is bounded
   in-run instrumentation and reverted.

## Hypotheses

- **H0 (no separable signal)**: no Phase 0 feature separates transient-peak
  cut frames (child later proven/refuted on re-entry within the run) from
  dead-end cut frames (never resolved) beyond the pre-registered lift
  threshold, on the gate object. The idea class closes: #14 closed with the
  separation study as the record, and the ε surface gains a fourth closure
  leg (no constant wins, no path schedule wins, no regional structure, **no
  local signal separates**).
- **H1 (separable and exploitable)**: a feature/bucketing passes the kill
  gate (below), and the best env-gated conditioned-ε arm yields **≥ 10%
  stress FO `child_evals` reduction vs the ε=0.375 baseline
  (227,834,433)** with m22/dec13/dec10 within +5% and zero quick-suite
  outcome changes → hand-off to `conversion` as a sized backlog item,
  conditional on the pending ε-default decision (the hand-off must state
  which global baseline it presupposes).
- **H2 (separable, not exploitable)**: the kill gate passes but no arm
  clears the GO bar — the signal exists but conditioning reintroduces the
  plan4 trajectory chaos. #14 closes with the separation table *and* the
  arm table as the record; the confidence feature is recorded for the
  parallel-spike design constraint file (it may still matter for worker
  scheduling, where node value is not the metric).

## Method

### Phase 0 — separation study (throwaway, 1 session)

Counter/logging-only instrumentation, env-gated (e.g.
`ATOMIC_EPSILON_STATS=1`); zero behavior change to thresholds, selection,
or stores.

**Event log** (keyed by Zobrist key + side to move; bounded map with a
pre-registered cap — stop recording on overflow, document the bound; no
overflow expected at 128 MB on the measured cases):

Per cut-frame exit (frame exhausting its threshold without a
proof/refutation exit), record:

- OR/AND side; depth; iteration round if cheaply available;
- `second/best` ratio (log-bucketed) and `epsilon_ceil(second) − best`;
- parent-clamp flag (`ε(second) ≥ th`);
- static top-2 margin from the frame's child slots (`children.rs` pooled
  slots carry the ordering scores; if the scores are not reachable at the
  threshold site without refactoring, drop this feature and record that);
- exiting gap `bound − threshold` (the plan6 class-B replication check).

Per position key, resolve at end of run (or lazily on re-entry):

- **rescued**: a later re-entry of the same key proved/refuted within the
  run (the earlier cut was a transient peak);
- **dead**: never resolved within the run (within-run dead end; the label
  is conservative — a "dead" child may resolve in a longer run, which
  weakens H1 signals, never H0).

**Runs** (release build, `--first-outcome`, 128 MB default TT):

| Case | ε | Role |
|---|---|---|
| stress `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` | 0.125 | gate object, baseline 249,480,478 must reproduce exactly |
| stress | 0.375 | gate object at the spin-off baseline (plan4: 227,834,433) |
| m22_white | 0.375 | control |
| dec13 | 0.375 | control |
| dec10 | 0.375 | control |

Running both ε values on stress tests whether the rescue/waste mix (and the
signal's separation) shifts with the global constant — the conditioning
must beat 0.375 *as-is*, so the mix at 0.375 is the operative one.

**Drift gate for Phase 0**: counters only — `benchmark --suite quick --json
--first-outcome` bit-identical before and after instrumentation, and the
stress FO eval counts reproduce the two baselines exactly.

**Phase 0 kill gate** (closes #14 under H0): no candidate feature has a
rescue-rate lift ≥ 2× the base rescue rate on any confidence bucket that
covers ≥ 5% of cut-frame eval mass on the stress ε=0.375 run. (Both numbers
pre-registered here; marginal cases are recorded but do not pass.)

Otherwise proceed to Phase 1 with the separating feature and its bucketing
as the conditioning basis.

### Phase 1 — confidence-conditioned ε POC (only if the kill gate fails)

Env-gated modification of the threshold computation at the two `min(...)`
sites in `core.rs` (OR: `epsilon_ceil(second_pn)`; AND:
`epsilon_ceil(second_dn)`), using the Phase 0 separating feature. No new
persistent state; the feature is recomputed from frame-local data at each
use. Pre-register the *gate*, not the exact arms — the arms follow from
Phase 0's table — but constrain the family:

- at most 3 arms, each a step function over the Phase 0 buckets mapping to
  ε_eff ∈ [0, 1] (respecting the existing `[0.0, 1.0]` range contract);
- at least one arm must implement the AND-side *inverted* mapping (pad
  less where confidence-of-no-refutation is high), since that is where the
  mass is;
- at least one arm must restrict modulation to non-clamped frames
  (`ε(second) < th`), targeting the 68–93% of cuts where the padding
  actually engages.

Gates (pre-registered):

- **GO**: best arm ≥ 10% below 227,834,433 stress FO child evals; m22 /
  dec13 / dec10 within +5% of their ε=0.375 values (12,351,299 /
  3,921,011 / 3,642,166); zero quick-suite outcome changes (59/59
  preserved; eval deltas expected — behavior-altering, so the
  bit-identical gate does not apply, but no outcome may flip); run-to-run
  determinism trivially preserved (env-gated, no clocks, no threads) →
  hand-off to `conversion` as a sized backlog item with the arm table,
  conditioned on the pending ε-default decision.
- **NO-GO / H2**: no arm clears the bar → close #14 with the separation
  and arm tables; record the confidence feature's non-exploitability.

### Phase 2 — revert and record

All `src/` instrumentation and POC code reverted; `git diff --exit-code`
verified before the report is written. Feature tables, bucket
distributions, run outputs, and the arm table (if Phase 1 ran) archived
under `measurements/plan8/`.

## Decision gates

| Gate | Criterion | Consequence |
|---|---|---|
| **KILL (Phase 0)** | no feature: ≥2× rescue-lift on a bucket ≥5% of stress (ε=0.375) cut-frame eval mass | Close #14 under H0; fourth leg of the ε-surface closure (no constant, no schedule, no regional structure, no local signal). Next plan: none — recommend initiative closure per report7's next-steps. |
| **GO (Phase 1)** | ≥10% stress FO win vs ε=0.375, controls within +5%, 0 quick outcome flips | Hand off to `conversion` as a sized item (conditioned on the ε-default decision); #14 marked mined-with-POC; spike code archived. |
| **NO-GO (Phase 1)** | separation real, no arm clears the bar | Close #14 under H2 with both tables; record the feature as non-exploitable for node-count purposes. Next plan as per KILL. |

## Deliverables

- `docs/plans/research/measurements/plan8/` — feature/bucket definitions,
  the separation contingency tables (per case × ε), run outputs, quick-suite
  bit-identity proof, and the Phase 1 arm table if reached.
- `docs/plans/research/report8.md` — verdict (H0/H1/H2), gate decision,
  separation characterization, closure or hand-off record, and the
  admissibility note (why this is the plan4 "new mechanism" and not a
  schedule re-run).
- `docs/plans/research/initiative.md` — backlog row **#14** added with
  status, History entry for plan8, and — if #14 closes under H0 — the
  strengthened ε-surface closure note feeding the initiative-closure
  recommendation.
- No bibliography changes expected (no new literature).

## Verification

- Phase 0 instrumentation is observation-only: quick suite bit-identical
  before/after; stress FO eval counts reproduce 249,480,478 (ε=0.125) and
  227,834,433 (ε=0.375) exactly (recorded in `measurements/plan8/`).
- Phase 1, if run: zero outcome changes on the quick suite and the three
  controls; all raw outputs archived; the bounded event-log cap never
  silently truncated the stress run (asserted, count recorded).
- `git diff --exit-code` clean at plan close (throwaway code reverted).
- Housekeeping: `cargo fmt --check`, `cargo clippy --release
  --all-targets`, `make test` green (hygiene gate; nothing shipped).

## Non-goals

- No production implementation — a GO verdict produces a *hand-off item*
  to `conversion`, not a landed change.
- No ε-default decision here: the 0.125 → 0.375 default validation
  (default-mode + thorough-suite + goldens) belongs to `conversion` and is
  presupposed, not performed, by this plan's hand-off.
- No new persistent search state (no per-node history, no statistics
  storage in shipped code) — RAM = TT only.
- No TT layout, snapshot-format, `ProofEvent`, or optimizer-interface
  changes; no proof-tree work; no wall-time-only work; the metric of
  record is first-outcome `child_evals`.
- No #13 (frontier priors), no #4 (frame overhead), no literature mining.

## Final task

Write `docs/plans/research/report8.md` (verdict, gate decision,
separation characterization, closure or hand-off record, admissibility
note), add backlog row #14 and the History entry in `initiative.md` (plus
the closure recommendation if H0/H2), and verify the revert.
