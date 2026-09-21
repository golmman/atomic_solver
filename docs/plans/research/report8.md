# Plan 8 Report: POC #14 — per-node confidence-conditioned ε (separation study + conditional trajectory POC)

## Summary

Phase 0 (counter-only, env-gated, trajectory-neutral instrumentation)
measured the rescue-vs-waste outcome of every threshold-cut frame on the
stress gate object at both ε values, keyed per position with within-run
resolution labels. The pre-registered kill gate was evaluated honestly in
both directions: **no single observable feature separates** transient-peak
cuts from dead-end cuts on any bucket covering ≥ 5% of cut-frame eval mass
(best single lift 1.94×, under the 2× bar), but **composite two-feature
buckets do** (OR ∧ depth 1–4 at 2.42× lift / 8.5% mass; depth 1–4 ∧ static
margin tie at 2.73×), so the gate failed and Phase 1 ran per the plan.

Phase 1 measured three env-gated conditioned-ε arms against the ε=0.375
baseline. **Every arm fails catastrophically**: OR-side pad-more at shallow
frames +300.0% on stress (910,954,290 vs 227,834,433) and +76.7% on m22;
AND-side pad-less on the tie region times out stress at 2.71 B evals and
regresses m22 +40.9%. No arm is within an order of magnitude of the ≥ 10%
GO bar, controls are outside +5%, and both arms flip quick-suite outcomes
via 5-second timeouts (`dec01`, `m23_white` `win → draw`, status `timeout`).

The ε mix-shift check explains why: the same conditioning buckets that
separate at ε=0.375 (2.42–2.73×) **collapse to base rate at ε=0.125**
(0.99–1.93×; no bucket of any width passes at all). The "confidence signal"
is not a stable node-local property — it is a fingerprint of the trajectory
the global constant happens to produce. Conditioning on it moves the
trajectory that generated it, which is exactly plan4's trajectory-chaos
mechanism resurfacing at node granularity.

**Gate decision: NO-GO (H2).** Backlog candidate #14 is closed with the
separation table and the arm table as the record; the confidence feature is
recorded as non-exploitable for node-count purposes. Per the plan's
consequence and report7's next-steps: **recommend closure of the `research`
initiative's node-count program** (see below).

## The ε-surface closure is now four-legged

plan4's escape clause required a *new mechanism* to revisit ε. This plan
supplied one (node-local, threshold-site conditioning — not a path
schedule), measured it, and it closes the surface with a fourth leg:

1. no global constant Pareto-dominates (plan4 Phase 0; ε=0.375 best at −8.7%);
2. no path-level schedule (depth/clock/linear) beats the constant (plan4 Phase 1);
3. no regional structure — the response is trajectory chaos (plan4);
4. **no node-local confidence signal converts into an eval win** (this plan):
   the signal that exists (a) is degenerate on the hot class in the
   df-pn-native form plan6 already measured (97.0% of cut mass at ratio
   < 1.25, minimum-increment growth), (b) separates only in composite
   buckets that appear at ε=0.375 and vanish at ε=0.125, and (c) every arm
   built on it destroys the search.

Any future ε revisit now needs a mechanism outside this entire class — not
a constant, not a schedule, not regional, not node-local-threshold
conditioning.

## Phase 0 — separation study (gate decisions)

Setup: counter-only instrumentation (`RESEARCH8_STATS=1`), fixed
open-addressing key→record table (2^24 × 32 B, 7/8 load cap, recording
stops on overflow), one record per position key (first cut wins, eval mass
accumulated), features captured at the two threshold-computation sites plus
the static-ordering margin retained from the frame's `sort_moves` scores.
Rescue labeling: a cut key is *rescued* if a later re-entry proved/refuted
it within the run, *dead* if never resolved in-run (conservative), with
draw-resolved and resolved-before tracked separately. The rescue rate is
mass-weighted, matching the metric of record; the plan's "iteration round"
feature was dropped (constant under `--first-outcome`). Full contract,
bucket definitions, and counters: `measurements/plan8/summary.md`.

Gate evidence (stress ε=0.375; base rescue rate 12.95% of cut-frame eval
mass, 0.47% of keys):

- **Single features — kill gate holds.** No ≥ 5%-mass bucket reaches 2×:
  OR-side 1.31×, depth 1–4 1.30×, static-margin=0 1.44×, exit-gap n/a
  1.94×, clamp=1 1.20×, ratio/gap hot buckets ≈ 1.00×. The 97–99%-mass
  buckets (AND tie, minimum-increment gap, primary-dimension cut) all sit
  at base rate — plan6's degenerate-signal finding replicated on the
  eval-mass weighting.
- **Composites — kill gate fails.** OR ∧ depth 1–4 = 8.5% mass at 31.4%
  (2.42×); depth 1–4 ∧ margin-tie = 7.3% at 35.3% (2.73×); exit-gap n/a ∧
  margin −15..−8 = 7.0% at 28.4% (2.19×). Sharpest: OR ∧ shallow ∧
  non-clamped ∧ margin-tie = 6.9% at 36.9% (2.85×).
- Interpretation: shallow OR frames that cut while their best child is
  *statically tied or better than the second* get rescued far more often
  than base — but only 0.5–4% of cut *keys* are ever rescued anywhere
  (7.3 M of 7.2 M keys are dead), so the entire separated mass is a thin
  high-work slice riding on an overwhelming dead-end floor.

The same evaluation at ε=0.125 (base 14.16%): best single lift 1.59×, and
**no composite bucket passes** — the OR-shallow bucket reads 0.99× and the
exit-gap n/a bucket 0.13×. The separation is ε-trajectory-dependent, not
node-intrinsic. This is recorded as the plan's key diagnostic finding; it
pre-weakens any conditioned-ε design before Phase 1 even runs, and was
confirmed post hoc after the Phase 1 arms failed.

Phase 0 drift gates, all green: quick suite bit-identical (env unset) with
the spike present (aggregate 38,974,090, 59/59, only wall-clock fields
differ); both instrumented stress runs reproduce 249,480,478 (ε=0.125) and
227,834,433 (ε=0.375) exactly with identical per-chunk fingerprints; all
three ε=0.375 controls reproduce (12,351,299 / 3,921,011 / 3,642,166); the
event-log cap never truncated any run (`overflow_dropped=0`).

## Phase 1 — conditioned-ε POC (arms and gates)

Arms (base ε=0.375 via `--epsilon`; each recomputed from frame-local data
at the two `min(...)` sites; no new persistent state):

| Arm | Mapping (else base) | stress | m22 | dec13 | dec10 |
|---|---|---|---|---|---|
| OR_SHALLOW | ε=1.0 @ OR ∧ depth ≤ 4 | 910,954,290 (+300.0%) | 21,819,225 (+76.7%) | 4,199,073 (+7.1%) | 3,967,790 (+9.0%) |
| OR_SHALLOW_NC | same, non-clamped frames only | ≡ OR_SHALLOW (proven no-op for pad-more) | ≡ | ≡ | ≡ |
| AND_TIGHT | ε=0.0 @ AND ∧ tie (second < 1.25·best) | timeout (draw @ 2.71 B evals) | 17,406,960 (+40.9%) | 4,828,633 (+23.1%) | 3,419,891 (−6.1%) |

Findings:

1. **Pad-more destroys the gate object.** Rescued-looking shallow OR frames
   are exactly where the search already extracts the win cheaply under
   ε=0.375; padding their thresholds inflates 8.5% of the cut mass into
   +300% total evals — the padded children were mostly dead after all
   (within-run rescue ≠ value under a changed trajectory).
2. **Pad-less on the AND tie explodes.** The tie region is 52% of cut mass;
   tightening thresholds there (cut refutation search sooner) times out
   stress and regresses two controls. The minimum-increment growth plan6
   observed is not slack to be harvested — it is load-bearing.
3. **The non-clamped restriction is vacuous for pad-more arms**: raising ε
   saturates at `min(th, ·) = th` on clamped frames, so OR_SHALLOW_NC is
   bit-identical to OR_SHALLOW. The plan's mandatory restriction can only
   ever differentiate pad-less mappings (covered by AND_TIGHT).
4. Quick-suite outcomes: both distinct arms flip `dec01` and `m23_white`
   (`win → draw`, both via 5-second timeouts — the arms explode even small
   positions; no soundness divergence on any finishing run).
5. GO gate: fails on every clause (stress +300% vs required −10%; controls
   outside +5%; quick outcomes not preserved). **NO-GO / H2.**

## Gate decision and closure record

- Phase 0 kill gate: **failed** (composite separation passes the
  pre-registered bar) → Phase 1 ran, per the plan.
- Phase 1: **NO-GO (H2)** — separation real, exploitability nil. #14 is
  **closed with both tables as the record**
  (`measurements/plan8/summary.md`, `analyze.py`, `stats_*.csv`).
- The confidence feature (static top-2 ordering margin × depth × side, at
  the cut site) is recorded as **non-exploitable for node-count purposes**.
  It may still matter for the parallel-spike design constraint file
  (`conversion` #4): rescue mass concentrates at shallow OR frames, i.e.
  worker-value estimation, not threshold shaping, is where node-local
  confidence could pay.
- Per the plan's KILL/NO-GO consequence and report7's Next steps: **the
  node-count program of the `research` initiative has no remaining
  candidate** — #10–#13 answered, #14 closed, literature targets #5/#8
  closed, #6/#7 pre-weakened. Recommendation: close the initiative (or
  re-scope to the parallel-spike constraint file under `conversion` #4).

## Admissibility note (new mechanism, not a schedule re-run)

plan4 closed ε as a function of *path state* (depth, remaining depth,
rule50, linear schedules — global functions of where the search is) and
wrote the escape clause: "a revisit needs a new mechanism". This plan tests
ε as a function of *node-local information read at the threshold-computation
site itself* — the best/second comparison and the frame's static-ordering
state, the same data the 1+ε trick already consumes. That is a different
function class: no path schedule can express "pad more iff this frame's
best child is statically tied with its second". The separation study
confirmed the class is not empty (composites separate where singles do
not) — and the arm table then showed it is nonetheless not exploitable,
because the separating signal is itself trajectory-relative (mix-shift:
separation present at ε=0.375, absent at ε=0.125). The closure is therefore
a measured result about a genuinely new mechanism, not a re-run of the
schedule family plan4 already rejected.

## Verification

- Observation-only Phase 0: quick suite bit-identical before/after
  (`quick_clean.json` vs `quick_spikeoff.json`; only wall-clock fields
  differ); stress FO eval counts reproduce 249,480,478 (ε=0.125) and
  227,834,433 (ε=0.375) exactly, with identical per-chunk fingerprints;
  controls reproduce 12,351,299 / 3,921,011 / 3,642,166; all raw outputs in
  `measurements/plan8/`.
- Event-log cap: `overflow_dropped=0` on every run (max table use 7.59 M of
  16.7 M slots on stress ε=0.125); asserted in the dump line of each
  instrumented run's stderr.
- Phase 1 raw outputs archived (`p1_*`, `quick_OR_SHALLOW.json`,
  `quick_AND_TIGHT.json`); determinism trivially preserved (env-gated, no
  clocks, no threads — repeated fields identical across runs).
- `git diff --exit-code -- src/` clean at plan close; post-revert stress
  run reproduces the ε=0.375 fingerprint exactly.
- Housekeeping: `cargo fmt --check` clean, `cargo clippy --release
  --all-targets` clean, `make test` green (nothing shipped).

## Problems encountered

- The plan's event-log sizing ("no overflow expected at 128 MB") needed a
  concrete bound: resolved with a fixed 2^24-slot open-addressing table
  (512 MB, the one documented bounded exception to RAM = TT only, per the
  plan's own Phase 0 clause) and a stop-on-overflow counter; unused.
- The plan's "static top-2 margin from the frame's child slots" was not
  directly reachable (scores are folded into ordering and discarded); the
  spike retained static scores per frame depth in `sort_moves`
  (env-gated) instead of dropping the feature, and recorded the
  substitution.
- "Iteration round" is not cheaply available in a meaningful form under
  `--first-outcome` (constant `u32::MAX`); dropped with the substitution
  documented.
- The rescue-rate definition (key- vs mass-weighted) was left ambiguous by
  the plan; both were computed. The mass-weighted rate is the operative one
  (matches the metric of record); the key-weighted variant passes the gate
  trivially because 97%+ of keys are dead at near-zero mass each —
  recorded in `summary.md` to keep the gate auditable.
- Unrelated to this plan: `docs/notes.md` gained an entry and
  `docs/theory/deep-{pns-2015,dfpn-2017}/` appeared mid-session (not plan8
  work; left untouched).

## Unresolved parts / missing tests

- None blocking. The separation study is bounded to within-run labels
  (conservative by design); longer-run or cross-run labeling could shift
  rescued/dead margins but cannot change the Phase 1 result, which fails by
  3–30× the GO bar on the strongest arm.
- The composite-bucket kill-gate reading (pairs as legitimate "candidate
  features") is this report's interpretation; under the strict
  single-feature reading the plan would have closed at Phase 0 under H0.
  Both readings are archived with full tables, and both end in #14 closed
  (H0-by-strict-reading / H2-by-composite-reading); the closure is robust
  to the interpretation.

## Next steps

- No further plan under the current `research` scope: every backlog row is
  answered, closed, or pre-weakened. Recommend initiative closure per
  report7's next-steps, with the rescue-mass/shallow-OR observation noted
  in the `conversion` #4 parallel-spike constraint discussion.
- The ε=0.375 default-ε decision remains open (unchanged by this plan;
  nothing here presupposes or alters it — Phase 1 used it only as the
  gate baseline). Registered 2026-09-21 as `conversion` backlog #7 after
  it was found to be referenced-but-unregistered on the receiving side;
  the decision (adopt / won't-fix / park) belongs to a `conversion` plan.
