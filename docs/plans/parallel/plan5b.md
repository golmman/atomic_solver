# Plan 5b — SPDFPN prototype, stage 2b: measurement campaign, GO/NO-GO verdict, revert-if-missed

Executes `parallel` backlog **#4, stage 2b** (the stage-2 measurement and
verdict half; the mechanism is plan5's deliverable). Sized for one
sitting. **This plan owns the stage's pre-registered GO bands and the
owner's condition-1 revert-if-missed clause; its final task is
`report5.md`** (the stage-2 report covering plan5 + plan5b, as the
backlog row's "final task = report5.md" intends).

Prerequisite reading: `plan5.md` + `report5a.md` (what landed, smoke
numbers, any design deviations), `plan4b.md`/`report4b.md` (the landed
TT and the pinned ≤ +7 % tax), `research_spdfpn.md` §2 (the paper's own
numbers: efficiency 0.85/4 threads, f_s ≤ 1.40 work inflation) and §4
(domain-transfer risks), `initiative.md` (the 2026-10-03 owner decision
and its three conditions).

## Owner conditions being executed (normative here)

- **Condition 1 — revert-if-missed**: the stage-1b tax acceptance holds
  only while this plan's GO bands are met. A NO-GO verdict reverts the
  sharded-TT refactor; the tax goes with it.
- **Condition 2 — explicit gate**: the landed TT's sequential tax stays
  pinned at ≤ +7 % median hard-class wall (plan4b); nothing in stage 2
  may erode the N = 1 byte-identity surface.
- **Condition 3 — measurement honesty**: wall measurements at 2–4
  threads are real (4-CPU container); **N ≥ 8 are simulated only** and
  never gate a verdict.

## Measurement protocol (pre-registered)

Reference binary: the **same build at N = 1** (the wall-speedup
denominator; the sharded tax is already accepted, so N = 1 of the landed
build is the honest baseline). For transparency, also report speedup vs
the saved unsharded pre-plan4b binary (the "tax-adjusted" figure).

1. **W sweep first, then locked** (plan5 leaves W = 1000 as the initial
   constant): W ∈ {250, 1000, 4000} child_evals on m22 at 4 threads,
   3 reps each; pick the best median wall; **lock W for the campaign**
   and record the sweep (the paper's 100–500 calls suggests smaller
   granules, our cheap nodes larger — measure, don't assume).
2. **Campaign cases** (the hard class, per the initiative's measurement
   conventions): m22 first-outcome (30 s cap) and shuffle-win
   first-outcome (100 s cap) — both points, every verdict-relevant
   metric.
3. **Threads**: N ∈ {2, 3, 4} — real. ≥ 5 interleaved reps per
   (case, N), interleaved with N = 1 reference runs in the same session
   to defeat session drift (plan4's protocol).
4. **N ∈ {8, 16}**: simulated only — clearly labeled
   extrapolations (e.g. efficiency-trend projection), non-gating, never
   presented as wall measurements.
5. **Metrics per (case, N)**:
   - median wall speedup vs N = 1 (primary);
   - **work inflation** = median total `child_evals` (all threads) /
     sequential `child_evals` — the f_s analog (paper: ≤ 1.40 at 16);
   - **outcome agreement**: every run's decisive outcome must equal the
     sequential solver's; any disagreement is a soundness violation
     (there is no acceptable rate);
   - per-thread work split (balance/fragmentation diagnostic);
   - panics, lock failures, job-lock hold anomalies: zero expected.
6. **N = 1 drift re-check at the end** of the session (quick-suite
   `child_evals`, stdout bytes, snapshot sha) — the campaign must not
   have perturbed the sequential surface.

## Pre-registered verdict bands (not renegotiable mid-campaign)

Let S4 = median wall speedup at 4 threads, on **both** hard-class cases;
I4 = work inflation at 4 threads; V = any soundness violation.

- **GO**: S4 ≥ 2.0× on both cases, I4 ≤ 2×, V = 0. → Stage 2 ships as
  opt-in; backlog #4 closes won; the tax acceptance becomes
  unconditional.
- **MARGINAL**: 1.5× ≤ S4 < 2.0× on either case (or exactly one case
  misses S4 ≥ 2.0×), I4 ≤ 2×, V = 0. → **Owner decision**, no unilateral
  ship or revert. The conservative literature band (~1.8×/4 threads,
  `initiative.md` condition 3) sits here — a marginal result is a
  realistic outcome, not a surprise; the ask to the owner must include
  both speedup figures (vs N = 1 and vs unsharded) and the inflation.
- **NO-GO**: S4 < 1.5× on either case, or I4 > 2×, or V ≠ 0 (any
  violation is an immediate NO-GO regardless of speedup).

### Revert-if-missed execution (NO-GO only)

1. Stop all measurement; record the numbers first (the verdict's
   evidence survives the revert).
2. Revert stage 1b + 2a: reverse-apply the re-land
   (`git apply -R` of the plan4b-applied patch; delete
   `src/search/tt/shard.rs`; remove `--threads` and the
   `dfpn::parallel` machinery), restoring the byte-identical sequential
   solver — verified by clean `git diff` vs pre-plan4b, `make test`
   green, snapshot sha, and a final quick-suite `child_evals` pass.
3. `initiative.md`: backlog #4 closed NO-GO (stage 2 verdict + the
   refunded tax); `docs/plans/README.md` `parallel` row updated (close
   event). The option-A stage then has no remaining lever — the
   initiative's closure records which measured no-gos now cover the
   whole parallelism space (option C, lean plan7, SPDFPN).
4. On MARGINAL: **no revert without the owner's call** — leave the tree
   landed and escalate with the numbers.

## Non-goals

- No re-tuning W/shard-count mid-campaign to chase a band (the sweep is
  pre-campaign and locked; post-hoc tuning would invalidate the
  pre-registration).
- No code changes beyond the recorded W constant, except the
  revert-if-missed execution itself.
- No real N ≥ 8 claims; no wall claims from simulated numbers.
- No proof-tree/pipeline work (event ordering nondeterminism is
  documented in plan5; its downstream impact is a finding if observed).

## Risks

- **Session drift across a multi-hour campaign**: interleaving with
  N = 1 references is the defense; medians over ≥ 5 reps the second.
- **The 4-thread result near the GO edge** (condition 3's warning):
  report both speedup denominators and per-thread splits so a MARGINAL
  escalation to the owner is decision-ready, not a coin flip.
- **Inflation concentrated in one worker** (AND-side imbalance): the
  per-thread split diagnostic distinguishes a mechanism fix (a plan5
  follow-up item) from a flat NO-GO.

## Deliverables

- `measurements/plan5/` — campaign README (provenance table), `env.json`
  update, sweep + campaign results (`state/*.json`, `*.csv`); logs
  gitignored.
- `initiative.md` — backlog #4 row final state (won / MARGINAL
  escalated / NO-GO + revert executed).
- `docs/plans/README.md` `parallel` row — updated on the stage-2
  resolution (this is a close-or-pivot event either way).
- **`report5.md` in this directory (final task)**: W sweep, campaign
  medians (speedup, inflation, agreement tally per N and case), verdict
  against the pre-registered bands, decision executed (ship / owner
  escalation / revert + tax refund), problems, unresolved parts, next
  steps (ship follow-ups or the initiative's closure state).
