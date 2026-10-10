# plan12 — Item #17 (phase 2): gate calibration + ≥20-case baseline rollout

Initiative: `research` (re-opened 2026-10-09). Continues reopened backlog
item **#17** (salt-seeded statistical gate); the pilot (plan11) is done with
verdict **GO**. Self-contained: a fresh session can execute this without
reading the full record.

## Motivation

plan11 promoted the TT bucket-index salt into the product CLI (`--salt`,
`--budget`, `evals:` line), ran the 6-case × 5-salt pilot, and drafted
[`gate_methodology.md`](gate_methodology.md) with **draft** thresholds. The
draft itself states what is missing: the win/regression thresholds and the
loud-case handling need the per-case paired-ratio evidence that only the
full pre-registered corpus provides (≥ 20 cases, whole m20–m23 family, ≥ 10
decisive controls). Until that is pinned, no #18/#19/#20 lever may be
measured. One pilot anomaly also needs a look before the salt set is frozen:
on dec10, salts 1 and 4 landed within 3 child evals of each other
(3,901,844 vs 3,901,847) — if salt pairs are effectively correlated on
multiple cases, five salts are fewer channels than they appear.

## Objective

1. The pre-registered baseline corpus of the gate: **22 cases × 5 salts**,
   first-outcome `child_evals`, censored runs recorded (110 runs).
2. A small read-only **salt-correlation probe** over the same runs (no extra
   solver invocations unless the pre-registered PIVOT rule fires).
3. **Pinned gate thresholds** published by upgrading
   `gate_methodology.md` from draft to v1.0: corpus + caps, W_LO/W_HI, the
   loud-case direction-agreement requirement, the hard-class definition, and
   the adopt/reject/defer rule — with the numbers derived from this corpus.
4. No `src/` changes; no lever measured; #19 re-score arms are **plan13**,
   not this plan (one discovery per plan).

## Pre-registered scope decisions

- **D1 — corpus (fixed before the first run).** 22 named fixtures from
  `tests/fixtures/`: the whole m-family **m20_white, m21_white (= stress),
  m22_white, m23_white** plus the **decisive controls dec01–dec18**
  (contiguous range from `decisive_positions.txt` — no cherry-picking;
  includes the pilot cases dec10/dec13). Fixture `expected` outcomes are
  part of the corpus contract.
- **D2 — salt set.** {0, 1, 2, 3, 4}. Salt 0 always runs (trajectory-identity
  anchor: all six pilot cases must reproduce their plan11/reexamination
  salt-0 child-eval counts exactly).
- **D3 — budgets (fixed before the first run).** Per-case child-eval caps:
  stress 2,500,000,000 (reexamination cap); **m20_white 2,000,000,000**
  (the pre-registered extension the methodology allows for censored-heavy
  cases: it sat at 3/5 censored on the 1 B pilot cap); all others 1,000,000,000.
  Wall `--timeout 600` strictly secondary. A run that hits either cap
  without a decisive outcome is right-censored; its eval count never enters
  a mean, median, ratio, or spread.
- **D4 — no code change.** The plan11 product surface is used as-is
  (`atomic_solver --fen <FEN> --tt-size 128 --first-outcome --outcome-only
  --salt S --budget C --timeout 600`; `evals:` on stderr). Identity gate D5(a)
  of plan11 is inherited unchanged (tree untouched this plan).
- **D5 — soundness invariants (hard gates, any violation halts).**
  (a) salt-0 reproduction of every pilot-case baseline count;
  (b) zero decisive-outcome conflicts across salts on any case;
  (c) every uncensored outcome matches its fixture `expected` value
  (win/loss from the side-to-move perspective) — a mismatch is a solver
  defect, not noise;
  (d) a budget-censored `draw` is the no-result sentinel, never a proven
  draw (inherited invariant 3 of the methodology).
- **D6 — salt-correlation probe (read-only).** For every salt pair (a, b)
  and every case where both runs are uncensored, compute the relative eval
  difference `|e_a − e_b| / min(e_a, e_b)`. A pair is **correlated** if it is
  near-identical (< 10⁻⁴ relative) on **≥ 2 cases**. One such pair on dec10
  alone (salts 1 vs 4) is already known and *not* sufficient — the pilot
  flagged it as worth one look, not as a defect.
- **D7 — PIVOT rule (pre-registered).** If D6 finds a correlated salt pair,
  the canonical salt set extends to {0, 1, 2, 3, 4, 5} for the affected
  pair's replacement (the correlated member stays in the raw data, the
  methodology note lists the canonical set), and the affected cases re-run
  at the added salt. Analysis then uses all available salts. If no pair is
  correlated, the 5-salt set is frozen.
- **D8 — calibration procedure (produces the pinned numbers).**
  1. Per-case baseline record: uncensored evals, censored salts, spread,
     median; case classified **loud** iff (uncensored spread ≥ 1.5×) or
     (mixed censored/uncensored), else **quiet**.
  2. Cross-salt noise band: over quiet cases, all salt-pair ratios
     `e_a / e_b`; report the observed band — this is how far pure noise
     moves a *between-salt* comparison, and the reason single-draw gates
     are retired.
  3. **Pin W_LO/W_HI.** The null (identical arms, paired by salt) is
     degenerate at ratio 1.0 — within-salt reruns are bit-identical
     (plan11 PIVOT check) — so the paired-median thresholds guard against
     direction-mixed effects, not against noise. W_LO = 0.9, W_HI = 1.1
     stay unless the rollout data shows a reason to move them; any change
     is stated with the reason in the methodology note.
  4. **Pin the loud-case rule:** on a loud case, win/regression additionally
     requires direction agreement on ≥ 4 of 5 salts (paired ratio < 1, or
     > 1, respectively); otherwise the case verdict is **unclear** and the
     lever defers to more salts on that case.
  5. **Pin the hard class:** the m20–m23 family plus every case whose
     baseline median ≥ 10 M child evals. Lever **adopt** requires ≥ 1
     censored-win or win on a hard-class case and no regression /
     censored-loss anywhere; **reject** on any regression or censored-loss;
     otherwise **defer** (extend salts on affected cases first).
  6. **Pin the under-budget rule:** > ⌈S/2⌉ censored salts on a case for
     either arm ⇒ case under-budgeted, contributes no win/regression.

## Method

- **Phase 0 — corpus tooling.** `measurements/plan12/driver.py` +
  `parse.py` (adapted from plan11: D1–D3 constants, resumable, ≤ 3
  concurrent processes, transcripts to a non-committed RAW dir).
- **Phase 1 — rollout.** 110 runs; parse to `state/runs.json` +
  `state/summary.json`; run the D5 checks and the D6 probe.
- **Phase 2 — calibration.** `calibrate.py` → `state/calibration.json`
  (D8 tables); decide the PIVOT rule.
- **Phase 3 — pin + report.** Upgrade `gate_methodology.md` to v1.0
  (pinned). Write `report12.md` (final task) and update the
  `docs/plans/README.md` research row.

## Gates

- **GO** → plan13: first #19 re-score arms (ε = 0.375 / 0.5 first) under the
  pinned rule.
- **PIVOT** → D7 fires: extend the salt set, re-run affected cases, then
  continue with Phase 2.
- **HALT** → any D5 invariant violated (outcome flip, fixture mismatch,
  salt-0 identity failure): soundness investigation, not a measurement.

## Out of scope

- Any lever measurement (#18 initialization, #19 re-scores, #20 restarts).
- `src/` changes; `benchmark`/optimizer interface; proof-tree layer.
- Other noise channels (tie-breaking, history aging) — still caveats only.
- The `dfpn` reopen and `parallel` items.

## Budget envelope

Worst case ≈ m20 5 × 2 B + stress 3 × 2.5 B + 1 B ≈ 19 B child evals ≈
45 min serial on the reference host (plan11 measured ≈ 7.7 M evals/s on the
stress shape); all dec controls are seconds each. ≤ 1 h wall with 3-way
concurrency.

## Final task

Write `report12.md` in this directory: deliverables, gate result, pinned
threshold summary, problems encountered, missing tests, next steps
(plan13 kickoff prompt).
