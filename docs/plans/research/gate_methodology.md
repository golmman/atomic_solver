# Gate methodology for lever measurements (pinned, plan12)

Status: **pinned v1.0** (plan12, 2026-10-10) — supersedes the plan11 draft.
Evidence: the 22-case × 6-salt baseline rollout
([`measurements/plan12/`](measurements/plan12/), 132 runs). The standing
rule is unchanged: any revision needs a stated reason and data. Scope:
every plan that measures a search-behavior lever against the shipped
baseline under initiative `research` (#18 initialization family, #19
re-score arms, #20 restarts).

## Premise

The shipped salt-0 trajectory is one draw from a heavy-tailed per-case
distribution. On the plan12 corpus (22 cases × 6 salts, zero outcome
flips, salt-0 identity verified):

- **14 of 22 cases are salt-invariant** (identical child evals on every
  salt): for these the gate is trivially stable — any real lever effect
  shows as an exact per-salt ratio.
- On the eval-sensitive cases, the **quiet cross-salt noise band is
  [0.760, 1.316]** (min/max ratio over distinct draws within every quiet
  case): a *between-salt* single comparison can move ±32% on pure noise.
  Single-draw per-case gates sit inside this band and are **retired**.
- **Loud cases** (uncensored spread ≥ 1.5× or mixed censoring): stress
  (3.04×), m20_white (1.75×), m22_white (1.67×), dec10 (1.65×).

## Noise channel

The channel is the TT bucket-index salt (`--salt <u64>` /
`Search::set_salt`), which remaps bucket sharing without changing search
semantics. Properties:

- **Deterministic.** A rerun at the same (case, salt) reproduces stdout and
  child evals bit-for-bit (plan11 PIVOT duplicate; re-confirmed by salt-0
  identity across the plan11→plan12 sessions). Within-salt variance = 0.
- **Semantics-neutral.** Zero decisive-outcome flips across 132 runs;
  every uncensored outcome matches its fixture expectation; salt-0
  trajectories are bit-identical to shipped behavior.
- **Per-case diversity is bounded by the case's trajectory structure.**
  Salt draws can coincide: pairs (1, 4), (1, 5), and (3, 5) are
  near-identical (< 10⁻⁴ relative evals) on the low-diversity cases dec10,
  dec13, dec14 (the plan11-flagged dec10 salt1/salt4 anomaly, confirmed
  pair-specific). The plan11 D7-style salt extension (salt 5 added) did
  **not** yield a globally clean extra channel. Consequently:
  - the canonical salt set is **{0, 1, 2, 3, 4, 5}** (all draws are used;
    correlated near-twins are documented, not hidden);
  - every case report includes its **basin count** — the number of
    *distinct draws* (draws clustered at 0.25 % relative granularity,
    anchored at each basin's first member). Basin counts on the corpus:
    hard class 3–6; dec13 = 2, dec14 = 3, dec10 = 4; the 14 salt-invariant
    cases = 1.
  - Other channels (tie-breaking jitter, history aging, hash-order
    effects) remain unmeasured caveats, not gates.

## Pre-registration contract (every lever plan)

Before any run, the plan states:

1. **Corpus.** The frozen plan12 corpus (22 cases: m20–m23 family +
   dec01–dec18; list and caps in
   [`measurements/plan12/driver.py`](measurements/plan12/driver.py)).
   Plans may extend it, never swap or drop cases post hoc.
2. **Salt set.** {0, 1, 2, 3, 4, 5} for the canonical corpus; never fewer
   than 5 draws. Salt 0 always runs (trajectory-identity anchor: the
   plan11/plan12 salt-0 baselines must reproduce exactly).
3. **Budgets.** Per-case child-eval caps fixed in advance — canonical
   caps: stress 2.5 B, m20_white 2 B, all others 1 B. Extensions
   pre-registered only. Wall `--timeout` strictly a secondary safety cap.
4. **Metric.** First-outcome `child_evals` (deterministic), plus outcome
   and censoring flag per run. PV length is informational only.

## Comparison rule (per case, paired by salt)

For each salt `s`: pair `(baseline_s, candidate_s)` from fresh runs of
both arms at the same salt. Per case report: per-salt paired ratio
`candidate_s / baseline_s` over pairs where **both** sides are
uncensored; the censoring pattern (both censored / only candidate / only
baseline); and the baseline basin count.

Case verdicts:

- **censored-win**: candidate solves where baseline is censored on ≥ 1
  salt, and is never censored where baseline solves;
- **censored-loss**: the mirror image;
- **win**: median paired ratio ≤ `W_LO = 0.9` **and** ≥ 75 % of the
  paired ratios < 1 (direction agreement);
- **regression**: median paired ratio ≥ `W_HI = 1.1` **and** ≥ 75 % of
  the paired ratios > 1;
- **unclear**: none of the above (symmetric censoring, spread inside the
  thresholds, or mixed direction).

Threshold rationale (pinned): the null (identical arms, paired by salt)
is degenerate at ratio 1.0 — within-salt reruns are bit-identical — so
`W_LO`/`W_HI` guard against direction-mixed effects, not against noise;
the direction-agreement clause guards a median dragged by one extreme
draw. The empirical paired-ratio dispersion of the first lever arms
(plan13) is the first real test of this rule; a revision needs data.

**Diversity clause.** A case whose baseline (or candidate) basin count is
< 3 is **low-diversity**: its draws are not independent, so it cannot be
the sole support of an *adopt* verdict (it can still veto via a
regression or censored-loss). Report the basin count next to every
verdict.

## Lever verdict

- **adopt**: ≥ 1 censored-win or win on a **hard-class** case **and** no
  regression / censored-loss on any pre-registered case. Hard class =
  the m20–m23 family plus every case with baseline median ≥ 10 M child
  evals — on the canonical corpus: **stress, m20_white, m22_white,
  m23_white**.
- **reject**: any regression or censored-loss.
- otherwise **defer**: extend the salt set on the affected cases before
  re-judging.
- A lever that only wins on salt-0-sensitive shapes without a paired-salt
  win is reported as **unjudged**, never as a win.

## Censoring rules

- A budget-exhausted run is **right-censored at the cap**: its eval count
  is not a measurement and never enters a mean, median, or ratio.
- Never impute censored evals (no `cap × 0.5` conventions).
- Mixed censoring (one arm censored, other not, same salt) is the
  strongest evidence the gate has; it must be reported per salt, not
  aggregated away.
- If more than `⌈S/2⌉` of the salt draws (S = 6 on the canonical corpus,
  so > 3) are censored on a case for either arm, the case is
  **under-budgeted** for that lever: the plan must either raise the
  (pre-registered) cap or move the case to a separate "unresolved" list.
  It contributes no win and no regression.

## Soundness invariants (hard gates, unchanged)

1. Salt 0 bit-identical to shipped trajectories (salt-0 baseline
   reproduction + benchmark identity on any `src/` change).
2. Zero decisive-outcome conflicts across salts on any case — a conflict
   is a soundness defect (TT/GHI interaction), halt and diagnose, not
   noise.
3. Budget-capped runs report the no-result draw; a plan must never read a
   censored `draw` as a proven draw.

## Erratum (plan11 headline table)

The plan11 README/report headline tables list the per-case **min** in
their "salt 0" column. The authoritative per-salt values are plan11's
`state/runs.json` (m23_white salt 0 = 9,673,403, not 9,440,650 — salt 2).
The plan12 rollout reproduces every plan11 salt-0 value exactly
(stress 249,480,478; m22 14,156,269; dec13 3,822,602; dec10 4,262,128;
m23 9,673,403; m20 censored), so cross-session identity holds; the
affected tables were corrected in place.
