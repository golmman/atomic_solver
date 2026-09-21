# Report 7: POC #12 — TT eviction/turnover measurement, conditional replacement-priority POC (backlog #12)

Executed 2026-09-20 per `plan7.md`. Measurement-first: Phase 0 counter-only
turnover instrumentation (store/probe classes, occupancy, eviction-shadow
probe classification), Phase 1 env-gated replacement-policy arms only because
the pre-registered Phase 0 kill gate failed, Phase 2 full revert. All
instrumentation and POC code is throwaway; `git diff --exit-code` verified
clean before this report was written. Raw outputs and derived tables:
`measurements/plan7/`.

## Verdict

**CLOSED (H2): the churn is real, but the two-slot priority layout cannot
convert it into a first-outcome `child_evals` win — and at the shipped
128 MB default the harmful class is marginal.** No arm of the
replacement-policy POC clears the ≥10% stress gate; the incumbent
`(live, solved, work, generation)` victim priority survives as a measured
local optimum. Backlog #12 closes with the direct eviction data joining the
plan4 ε-closure as the no-go record — and with a premise correction of its
own (the TT-size invariance study does not transfer to the current solver;
see below).

## Gate decision

| Gate | Criterion | Decision |
|---|---|---|
| KILL (Phase 0) | LS-evictions < 0.1% of stores AND occupancy < ~80% at 128 MB; or churn at 32 MB cost-free (node parity) | **Not met** — stress: LS 0.146%, occupancy 95.4% full; 32 MB stress catastrophically diverges (unsolved at 3.33 B evals vs solved at 249 M). Phase 1 proceeds per plan. |
| GO (Phase 1) | ≥10% stress FO win, controls within +5%, 0 quick outcome flips | **Not met** — V1: stress timeout at 3.95 B evals, m22 +3400%; V2: stress +38.2%, m22 +23.7%. (Quick outcomes: 59/59 preserved for both arms.) |
| NO-GO / H2 | churn real, no arm clears the bar | **Met** — churn characterized (below); #12 closes. |

## Churn characterization (Phase 0, 128 MB default, exact-baseline runs)

Instrumentation is trajectory-neutral: stress reproduced 249,480,478 child
evals and the 8-chunk `work_done` fingerprint bit-identically; the quick suite
was bit-identical before/after instrumentation and after the revert.

1. **The harmful transition (a) exists but is marginal at the default.** New
   unsolved stores evicted live solved entries 19,647 times on stress
   (0.141% of 13.9 M stores; total LS class incl. solved-vs-solved victims
   0.146%), 32 on m22, 7 on dec13, 0 on dec10. Both plan7 conditions for
   "negligible" technically fail (0.146% > 0.1%), but the absolute mass is
   three orders of magnitude below what Phase 1 showed to be exploitable.
2. **Occupancy is case-dependent, not uniformly below capacity.** Stress ends
   at 95.4% full buckets (4.08 M live entries); m22 4.6%, dec13 0.25%, dec10
   0.09%. The stress case genuinely runs near saturation at 128 MB — but
   solves exactly at the baseline, so saturation is not itself costly here.
3. **Unsolved-vs-unsolved turnover dominates store traffic on stress**
   (7.34 M live-unsolved victims by unsolved stores, 52.8% of stores), yet
   only 1.21% of probes miss on a key evicted earlier this run
   (shadow-history approximation; under-counts by construction). Most
   eviction churn is amortized noise, not re-proof cost — consistent with
   plan1's finding that the work mass is in threshold-cut frames, not
   re-proofs.
4. **Spin-off finding — the TT-size invariance premise is stale.** The plan's
   context leg #1 ("decisive point invariant across TT ∈ {32 MB, 128 MB,
   2 GB}") dates from the 2026-09-11 position study, i.e. *before plan9's
   repetition cache and the plan10/11 changes*. Direct measurement on the
   current solver: 32 MB stress does not reach a decisive outcome within
   600 s / 3.33 B child evals (13× the 128 MB solve cost), deterministically
   reproducible. The invariance claim was true of the old solver and is false
   of this one; any future capacity or pressure work (`lean` #16 territory)
   must treat the post-plan9 solver as TT-sensitive on the stress class.
   This is a measurement record, not a default-size proposal — the 128 MB
   default is not re-litigated here.

## Phase 1 arms and why they fail

- **V1 (solved-slot immunity: new unsolved store never evicts a live solved
  slot; store dropped).** Mechanically sound — harmful evictions went to
  zero on the affected transitions — but it dropped 15.0 M stores on stress
  (vs the ~20 K evictions it prevents): unsolved bounds are the search's
  working set, and discarding them forces re-searches that dwarf the saved
  re-proofs. Stress: no decisive outcome in 600 s (3.95 B evals); m22
  +3400%. Decisively refuted.
- **V2 (steeper work priority: `remaining_depth` tiebreak before
  `generation` among live unsolved victims).** Trajectory reshaping without
  cost reduction: stress +38.2% (though it found a 97-ply first win vs the
  baseline's 477-ply line), m22 +23.7%. The existing work-dominant priority
  already captures the exploitable structure.

Both arms preserved all 59 quick-suite outcomes (evals bit-identical — the
variants never fire on quick-suite positions; the table never fills there).
Per the pre-registered gates this is a NO-GO: the churn profile plus the arm
table close #12 without a `lean` hand-off.

## Closure record

Backlog #12 is closed with three legs, mirroring the plan's H0 three-legged
record shape but under H2:

1. **Direct eviction data** (this plan, Phase 0): harmful-class churn
   0–0.146% of stores at the default; churn not probe-cost-dominant
   (≤1.21% of probes on stress).
2. **Arm-table refutation** (Phase 1): both pre-registered policy variants
   regress or fail to terminate on the gate object; the incumbent priority
   replacement is a measured local optimum within the fixed two-slot layout.
3. **Binding constraint recorded**: the two-slot bucket is the layout that
   makes solved-slot protection affordable only by dropping unsolved stores,
   which is fatal. Any future eviction-policy work would need a layout
   change, which the RAM = TT only contract and the snapshot format fix as
   out of scope.

## Premise corrections (for `initiative.md`)

1. The candidate-table premise "instead of uniform replacement" was already
   stale when this plan was drafted (documented in plan7.md): priority
   replacement by `(live, solved, work, generation)` predates the
   initiative. The genuinely open question was the harmful transition's
   existence and cost — now answered (exists, marginal, unexploitable).
2. New correction found by this plan's measurement: the TT-size invariance
   study cited as #12's pre-weakening does not transfer to the post-plan9
   solver (32 MB stress unsolved at 3.33 B evals). The pre-weakening leg is
   replaced by this plan's direct eviction data.

## Problems encountered

- The Phase 0 kill gate's conjunctive condition A fails on *both* conjuncts
  for stress (0.146% > 0.1%; 95.4% > 80%), so the cheap close did not
  trigger and Phase 1 was run as pre-registered — worth noting that the
  0.1%/80% thresholds sit close enough to the stress profile that a
  marginally different profile could have forced a Phase 1 for any hard
  case; the thresholds were adequate here only because Phase 1's arms
  failed decisively, not marginally.
- No other blockers. The eviction-shadow probe classifier (2^21-slot
  direct-mapped history) worked as designed; `stale_*` counter classes
  stayed zero exactly as predicted by the inert-generation premise
  (context #4 confirmed by direct count).

## Unresolved parts / missing tests

- The `miss_evicted` probe class is an approximation (bounded shadow
  history, under-counting); the exact class would need per-key history,
  which the RAM contract forbids for anything but throwaway instrumentation.
- The 32 MB stress non-solution was confirmed twice at 600 s but not run to
  natural completion (projected cost ≫ 1 h); the characterization "unsolved
  at ≥3.33 B evals" is the record, not an exact unsolved cost.
- No `make test-full` run: nothing shipped; the hygiene gate (`make test`)
  is green on the reverted tree.

## Next steps

Per the plan6 CLOSED consequence, the remaining pre-weakened targets are
literature #6 (ML node priors — sharpened no-heuristic-component blocker)
and #7 (mating-net recognizers — plan3's zero-harvestable-subgames data),
plus POC candidate #13 (frontier prediction priors, itself pre-weakened by
the plan9 ordering closure). All are now weakened by direct or transferred
measurements; the initiative should consider whether any open question
remains that a one-session POC can still decide, or whether to recommend
closure of the initiative's node-count program in favor of the `conversion`
tempo class.

## Verification summary

- Drift: `benchmark --suite quick --json --first-outcome --timeout 5`
  bit-identical clean → instrumented → post-revert (`quick_clean.json` /
  `quick_instrumented.json` / `quick_postrevert.json`).
- Baseline: stress FO 249,480,478 reproduced exactly under instrumentation.
- Phase 1: 59/59 quick outcomes preserved for both arms (behavior-altering
  policy, so eval deltas are expected and allowed).
- Revert: `git diff --exit-code` clean; stress chunk fingerprint identical
  post-revert; `cargo fmt --check`, `cargo clippy --release --all-targets`,
  `make test` green.
