# plan13 — Item #19 (first arms): ε = 0.375 / 0.5 re-score under the pinned gate

Initiative: `research` (re-opened 2026-10-09). Executes reopened backlog
item **#19** (re-score the unjudged closures), first arms per report12's
GO: **ε = 0.375 and ε = 0.5 vs. the shipped ε = 0.125 baseline** — the
cheapest re-score (single option change per arm, no `src/` changes).
Self-contained. Independent of plan14 (#23 tie-break channel); either
execution order is valid.

## Motivation

plan4 (backlog #11) measured global-ε single draws on a pre-gate solver:
ε = 0.375 "Pareto-improved all four cases" (stress −8.7%, m22 −12.7%,
dec13 +2.6%, dec10 −14.5%) and ε = 0.5 won stress −37.3% but regressed
controls (m22 +12.7%, dec13 +52.4%). The re-examination (§1) showed
single-draw gates sit below the noise floor (quiet cross-salt band
[0.760, 1.316]), so those results are **unjudged, not won** — exactly the
class item #19 re-scores. This plan is also the *first empirical test* of
the pinned paired-ratio comparison rule (gate_methodology.md v1.0 pins
W_LO/W_HI on a degenerate-null argument; its dispersion has never been
observed on real lever arms).

## Objective

1. Fresh paired runs of both ε arms and the shipped baseline over the
   frozen plan12 corpus (22 cases × 6 salts × 3 arms = 396 runs).
2. Per-case verdicts per the pinned comparison rule (censored-win /
   censored-loss / win / regression / unclear; adopt / reject / defer at
   the lever level).
3. The observed paired-ratio dispersion recorded as the methodology's
   first real-arm calibration data point.
4. No `src/` changes; the product surface is used as-is
   (`--epsilon` exists).

## Pre-registered scope decisions

- **D1 — corpus, caps, salt set (inherited verbatim, frozen).** The 22
  plan12 fixtures and per-case caps (stress 2.5 B, m20_white 2 B, others
  1 B; `--timeout 600` secondary); salts {0, 1, 2, 3, 4, 5}; salt 0 always
  runs. Extensions pre-registered only.
- **D2 — arms (fixed).** Baseline: ε = 0.125 (shipped default).
  Candidates: ε = 0.375, ε = 0.5 — each differs from the baseline by the
  single `--epsilon` option; no other flag changes. Command line per run:
  `atomic_solver --fen <FEN> --tt-size 128 --first-outcome --outcome-only
  --salt S --budget C --timeout 600 [--epsilon E]` (`evals:` on stderr).
- **D3 — soundness invariants (hard gates, any violation halts).**
  (a) **Cross-session identity:** every fresh baseline run at salt 0 must
  reproduce its plan11/plan12 recorded value exactly (stress 249,480,478;
  m22 14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20
  censored) — the null is degenerate, so any mismatch is an identity
  failure, not noise; (b) the same holds at every salt for the baseline
  arm (plan12's per-salt recorded values are the reference); (c) zero
  decisive-outcome conflicts within any arm across salts, and every
  uncensored run of every arm matches its fixture `expected` outcome (ε
  changes work, never the proven value — an arm-level outcome flip is a
  soundness defect); (d) a budget-censored `draw` is the no-result
  sentinel, never a proven draw.
- **D4 — verdict machinery (pinned v1.0, applied verbatim).** Per case:
  per-salt paired ratios `candidate_s / baseline_s` over pairs where both
  sides are uncensored; censoring pattern per salt; baseline basin count
  from plan12. Verdicts: censored-win / censored-loss / win (median ratio
  ≤ 0.9 ∧ ≥ 75% of ratios < 1) / regression (median ≥ 1.1 ∧ ≥ 75% > 1) /
  unclear. Lever verdict per arm: **adopt** = ≥ 1 censored-win or win on a
  hard-class case (stress, m20_white, m22_white, m23_white) ∧ no
  regression / censored-loss anywhere; **reject** = any regression or
  censored-loss; otherwise **defer**. Low-diversity baselines (basin
  count < 3) cannot solely support an adopt. Under-budget rule: > 3 of 6
  censored salts on a case for either arm ⇒ case unresolved, contributes
  nothing.
- **D5 — defer extension rule (pre-registered).** A defer is re-judged
  only after extending the salt set **by {6, 7} on the affected cases,
  both arms**, fresh runs at the added salts; the pooled verdict is then
  recomputed. One extension per arm; a second defer stands.
- **D6 — hand-off rule (pre-registered).** An **adopt** arm becomes a
  sized hand-off note to `conversion` (the ε thread's owner: #7 / plan4
  spin-off), stating the per-case paired tables and the proposed default
  change. **Reject** arms are recorded with their tables; #19's remaining
  closures (clock-budget reuse, history/killer arms, eviction V2) are
  later plans, not this one.

## Method

- **Phase 0 — tooling.** `measurements/plan13/driver.py` + `parse.py`
  adapted from plan12's (D1–D2 constants, arm dimension added, resumable,
  ≤ 3 concurrent processes, transcripts to a non-committed RAW dir).
- **Phase 1 — rollout.** 396 runs; parse to `state/runs.json` +
  `state/summary.json`; run the D3 checks.
- **Phase 2 — verdicts.** `verdicts.py` → `state/verdicts.json` (D4
  tables per case per arm, plus the paired-ratio dispersion summary that
  the methodology's threshold rationale cites as its first calibration
  data).
- **Phase 3 — report.** D6 hand-offs if any; write `report13.md` (final
  task); refresh the `docs/plans/README.md` research row **only** if an
  adopt hand-off or a methodology revision results.

## Gates

This plan produces verdicts, not a GO/NO-GO:

- **adopt** → hand-off to `conversion`; #19 ε closure resolved positively.
- **reject / defer** → recorded with tables; the ε closure legs gain their
  first gate-grade data.
- **HALT** → any D3 invariant violated: soundness/identity investigation,
  not a measurement.

## Out of scope

- The remaining #19 closures (clock-budget solved-entry reuse, `lean`
  plan10 history/killer arms, TT-eviction arm V2) — later plans.
- #18 initialization, #20 restarts, #21 in-context child results, #23
  tie-break channel (plan14).
- `src/` changes; benchmark/optimizer interface; proof-tree layer;
  methodology revisions (any W_LO/W_HI change needs stated data per the
  standing rule — this plan only *records* dispersion).

## Budget envelope

plan12 measured 132 runs ≈ 42 min CPU-serial; 396 runs ≈ 2.5 h CPU-serial,
≤ ~1 h wall at 3-way concurrency. Censoring is bounded by the caps (the
ε arms may add censored cells — e.g. m20 at ε = 0.5 — which the
under-budget rule absorbs). Salt-0 baseline cells duplicate plan12 by
design (identity audit).

## Final task

Write `report13.md` in this directory: deliverables, per-arm verdict
tables with basin counts and censoring patterns, paired-ratio dispersion
summary, problems encountered, missing tests, next steps (kickoff prompt:
remaining #19 arms or plan14 per owner choice).
