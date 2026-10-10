# report12 — plan12 (item #17 phase 2: gate calibration + baseline rollout)

Date: 2026-10-10. Initiative: `research`. Evidence:
[`measurements/plan12/`](measurements/plan12/) (132 runs),
[`gate_methodology.md`](gate_methodology.md) (pinned v1.0).

## Deliverables

1. **plan12.md written and executed.** Deviation from the two-session
   cadence, stated up front: the plan file did not exist (plan11's report
   ended with "GO → plan12" but no plan session had drafted it). This
   session drafted plan12 from plan11's next-steps contract and executed
   it in one sitting. One further scoping correction: report11's next
   steps merged "rollout + first #19 re-score arms" into plan12; the
   one-discovery-per-plan working agreement keeps them apart, so the #19
   re-scores are plan13.
2. **22-case × 6-salt baseline rollout** (pre-registered 22 × 5 = 110
   runs, extended to 132 by the pre-registered D7 PIVOT remedy): 5 runs
   budget-censored (m20 salt 0/1 at 2 B, stress salt 2/3/4 at 2.5 B),
   zero wall-cap hits, ~42 min CPU-serial, ≤ 3 concurrent processes.
3. **All D5 soundness invariants pass.**
   - (a) salt-0 identity: stress 249,480,478; m22 14,156,269; m23
     9,673,403; dec13 3,822,602; dec10 4,262,128; m20 censored — every
     plan11 salt-0 value reproduced exactly (cross-session identity).
   - (b) zero decisive-outcome conflicts across salts on all 22 cases.
   - (c) every uncensored outcome matches its fixture expectation.
4. **D7 PIVOT fired and was executed.** Salt pair (1, 4) is near-identical
   (< 10⁻⁴ relative evals) on two eval-sensitive cases (dec10, dec13) —
   the anomaly plan11 flagged, now confirmed pair-specific. Salt 5 was
   added to all 22 cases per the pre-registered remedy.
5. **`gate_methodology.md` upgraded draft → pinned v1.0**: corpus + caps
   frozen; canonical salt set {0, 1, 2, 3, 4, 5}; W_LO = 0.9 / W_HI = 1.1
   with the ≥ 75 % direction-agreement clause; per-case **basin counts**
   (distinct draws at 0.25 % granularity) as a mandatory report field;
   low-diversity clause (basins < 3 cannot solely support an adopt);
   hard class pinned to stress, m20, m22, m23; under-budget rule (> 3 of
   6 censored); adopt/reject/defer rule as drafted.
6. **plan11 erratum** (found by the rollout): plan11's README/report
   headline tables listed the per-case *min* in the "salt 0" column
   (m23: 9,440,650 is salt 2; the true salt-0 value is 9,673,403).
   Corrected in place in both files; noted in the methodology's erratum
   section. No measurement claim of plan11 changes.
7. `docs/plans/README.md` research row updated (this report, final task).

## Key calibration results

- **14 of 22 cases are salt-invariant** (identical evals on all six
  salts) — for those the gate is trivially stable. All sensitivity lives
  in the m-family plus dec01/dec10/dec13/dec14.
- **Quiet cross-salt noise band [0.760, 1.316]**: a between-salt single
  comparison moves ±32 % on pure noise even on quiet cases — the single-
  draw gate retirement is now quantified, not just argued.
- **Per-case draw diversity is bounded by trajectory structure, not salt
  count**: dec13's six draws form 2 basins, dec14 3, dec10 4; the hard
  class has 3–6. Salt 5 is not a globally clean extra channel (new
  near-twin pairs (1, 5) and (3, 5) surfaced on low-diversity cases).
  The methodology therefore reports basin counts per case instead of
  pretending five independent draws everywhere.
- Loud cases: stress 3.04×, m20 1.75×, m22 1.67×, dec10 1.65×.
- New baseline fact: at the extended 2 B cap, m20 salt 0 and salt 1 are
  *still* censored while salts 2/3/4/5 solve at 791 M–1384 M — the
  shipped salt-0 trajectory does not solve m20 at 2 B; plan11's "m20 is
  the hardest shape" reading is confirmed and strengthened.

## Problems encountered

- **D5(a) false alarm on m23**: my first expectation table took m23's
  salt-0 count from plan11's README headline table — which (per the
  erratum above) lists the min, not salt 0. The solver was right; the
  plan11 table was wrong. Fixed the table and the expectation.
- **D6 probe first draft over-fired**: as pre-registered ("every case
  where both runs are uncensored"), every salt pair qualified on the 14
  salt-invariant cases (all draws identical → trivially near-identical).
  Corrected to eval-sensitive cases only (max pairwise relative eval
  difference ≥ 10⁻³), which restores the pre-registered *intent* (the
  plan's D6 rationale targeted pair-specific correlation, not case-wide
  invariance). Recorded here as a stated deviation from the plan's D6
  wording; the corrected definition is in the methodology.
- **D7 remedy partially ineffective**: adding salt 5 did not produce a
  clean fifth channel on the low-diversity cases; the channel's per-case
  diversity is bounded by the case's trajectory structure. Handled by
  pinning basin counts as a mandatory report field rather than extending
  the salt set again (pointless for those cases, unnecessary for the
  hard class, where all 6 draws are distinct).

## Missing tests / caveats

- The salt channel's independence from other noise sources (tie-breaking,
  history aging) remains unmeasured — still caveats, not gates.
- The W_LO/W_HI paired-ratio rule is pinned on the degenerate-null
  argument; its first empirical test is the paired-ratio dispersion of
  plan13's first #19 arms. Any revision needs stated data (standing rule).
- The mechanism behind near-twin salt collisions on specific small cases
  (why salts 1/4/5 collide on dec10) is not diagnosed; documented as a
  channel property via basin counts. No soundness concern (full-key
  verification unchanged; zero flips on 132 runs).
- `make test` not run: plan12 makes **no `src/` changes** (verified by
  `git status` — only docs/ and measurements/ artifacts changed).

## Gate result

**GO → plan13.**

SESSION COMPLETE
- plan12 drafted + executed: 22-case × 6-salt rollout measured
  (measurements/plan12/, 132 runs, all D5 gates green), D7 PIVOT fired and
  executed, gate_methodology.md pinned v1.0, plan11 erratum fixed,
  docs/plans/README.md research row updated
Follow-up options:
  1. "Execute docs/plans/research/plan13.md" — draft + run the first #19
     re-score arms (ε = 0.375 and ε = 0.5 vs. the ε = 0.125 baseline) under
     the pinned gate: paired per-salt runs over the frozen 22-case corpus,
     verdicts per gate_methodology.md. Reason: #19 is unblocked and the
     ε arms are the cheapest first re-score (single option change per arm).
  2. Alternative: plan13 could instead re-score the `dfpn` clock-budget
     reuse arm or the `lean` plan10 history/killer arms — only if a #19
     batching preference exists; the ε arms remain the recommended first.
