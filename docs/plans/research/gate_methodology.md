# Gate methodology for lever measurements (draft, plan11)

Status: **draft** — thresholds below are calibrated on the plan11 pilot
([`measurements/plan11/`](measurements/plan11/)) and are to be pinned (or
revised with stated reason) by plan12 before the first #19 re-score arms run.
Scope: every plan that measures a search-behavior lever against the shipped
baseline under initiative `research` (#18 initialization family, #19 re-score
arms, #20 restarts).

## Premise

The shipped salt-0 trajectory is one draw from a heavy-tailed per-case
distribution (plan11 pilot: stress spread ≥ 3× with censoring, m20 mixed
censored/solved, m22/dec10 spread 1.65–1.67×; reexamination §1). Any
single-draw per-case comparison ("wins on X, regresses control Y") is below
the noise floor on the hard class and is **retired as a gate** from now on.

## Noise channel

The channel is the TT bucket-index salt (`--salt <u64>` / `Search::set_salt`),
which remaps bucket sharing without changing search semantics. Properties
established in the pilot:

- **Deterministic.** A rerun at the same (case, salt) reproduces stdout and
  child evals bit-for-bit (within-salt variance = 0). No wall-clock term
  enters the metric as long as runs are budget-capped (`--budget`) and the
  budget fires before `--timeout`.
- **Semantics-neutral.** Zero decisive-outcome flips across salts on the
  pilot corpus; salt-0 trajectories are bit-identical to shipped behavior.
- **One realization per salt.** Five salts give five *correlated-ish* but
  distinct collision patterns, not five independent samples of all possible
  noise. Other channels (tie-breaking jitter, history aging, hash-order
  effects) are unmeasured — recorded as caveats, not gates, until a plan
  measures them.

## Pre-registration contract (every lever plan)

Before any run, the plan states:

1. **Corpus.** ≥ 20 cases including the whole m20–m23 family (the known
   heavy-tailed class) and ≥ 10 decisive-suite controls. Case list fixed
   before the first run; no post-hoc case swaps.
2. **Salt set.** {0, 1, 2, 3, 4} minimum; more salts allowed, never fewer.
   Salt 0 always runs (trajectory-identity anchor).
3. **Budgets.** Per-case child-eval caps fixed in advance (hard class ≥ the
   reexamination caps: stress 2.5 B, others 1 B; extend for censored-heavy
   cases like m20 if pre-registered). Wall `--timeout` strictly a secondary
   safety cap.
4. **Metric.** First-outcome `child_evals` (deterministic), plus outcome and
   censoring flag per run. PV length is informational only.

## Comparison rule (per case, paired by salt)

For each salt `s`: pair `(baseline_s, candidate_s)` from fresh runs of both
arms at the same salt. Per case report:

- the per-salt ratio `candidate_s / baseline_s` over pairs where **both**
  sides are uncensored;
- the censoring pattern (both censored / only candidate / only baseline).

Case verdicts:

- **censored-win**: candidate solves where baseline is censored on ≥ 1 salt,
  and is never censored where baseline solves;
- **censored-loss**: the mirror image;
- **win** / **regression**: median paired ratio ≤ `W_LO` / ≥ `W_HI` over
  usable pairs (draft: `W_LO = 0.9`, `W_HI = 1.1` — the pilot's
  not-sensitive cases move ≤ 1.32× between salts, but paired same-salt
  ratios have lower variance than per-arm spreads; plan12 pins these);
- **unclear**: none of the above (e.g. symmetric censoring, or spread inside
  the thresholds).

Lever verdict (draft, plan12 pins): **adopt** requires ≥ 1 censored-win or
win on the hard class *and* no regression/censored-loss on any pre-registered
case; **reject** on any regression or censored-loss; otherwise **defer to
more salts** (extend to 9–15 salts on the affected cases before re-judging).
A lever that only wins on salt-0-sensitive shapes without a paired-salt win
is reported as **unjudged**, never as a win.

## Censoring rules

- A budget-exhausted run is **right-censored at the cap**: its eval count is
  not a measurement and never enters a mean, median, or ratio.
- Never impute censored evals (no `cap × 0.5` conventions).
- Mixed censoring (one arm censored, other not, same salt) is the strongest
  evidence the gate has; it must be reported per salt, not aggregated away.
- If more than `⌈S/2⌉` of the salt set is censored on a case for either arm,
  the case is **under-budgeted** for that lever: the plan must either raise
  the (pre-registered) cap or drop the case to a separate "unresolved" list.
  It contributes no win and no regression.

## Soundness invariants (unchanged, hard gates)

1. Salt 0 bit-identical to shipped trajectories (benchmark identity check).
2. Zero decisive-outcome conflicts across salts on any case — a conflict is
   a soundness defect (TT/GHI interaction), halt and diagnose, not noise.
3. Budget-capped runs report the no-result draw; a plan must never read a
   censored `draw` as a proven draw.

## Corpus sizing note (from the pilot)

Five salts suffice to *detect* sensitivity (spread ≥ 1.5× or mixed
censoring) on shapes as heavy-tailed as stress, but the win/regression
thresholds above need the paired-salt variance estimate that only the full
rollout provides — plan12 must publish the pinned thresholds together with
the per-case paired-ratio distribution of the baseline corpus.
