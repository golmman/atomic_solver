# plan8 measurements (2026-10-11) — salt-seeded process portfolio at gate grade: ADOPT

Executes `docs/plans/parallel/plan8.md`. Zero product-code changes: each racer
*is* the unmodified solver at a different `--salt`; soundness is inherited per
process. Stage-0 identity gate passed (5/5 exact plan12 anchor reproductions).
Stage-1 pre-computation frozen from the committed plan12 rollout before any
race. Stage-2 rem12 extension: 6 solo salt runs — **not** salt-invariant (6
distinct draws, spread 1.31×, all uncensored `win`), so it stayed a real
out-of-sample raced case. Stage-3 pilot clean. Stage-4 campaign: 3 rounds × 6
cases (extension trigger never fired; max W_first spread 0.7%).

| artifact | role |
| --- | --- |
| `derive_portfolio8.py` | stage 1: evals-space portfolio table from plan12 (frozen pre-race) |
| `campaign8.py` | stages 3–4: pilot + campaign driver (baseline + 4-racer race per round) |
| `analyze_portfolio8.py` | stage 5: mechanical H1–H4 evaluation, distributions, CPU cost |
| `state/stage0_identity.json` | stage-0 identity-gate record (H4a) |
| `state/derived_evals.json` | stage-1 frozen pre-computation |
| `state/stage2_rem12.json` | stage-2 rem12 per-salt baseline distribution |
| `state/races8_raw.jsonl` | one JSON line per run (baseline/racer/race; pilot labeled) |
| `state/races8.json` | computed distributions + hypothesis verdicts |
| `state/races8.csv` | flat per-run table |
| `env.json` | environment/config provenance |
| `logs/` | gitignored solver stderr/stdout transcripts |

## Verdict: H1 ∧ H3 ∧ H4 all pass → **ADOPT** (pre-registered rule)

| case | baseline salt-0 (fresh median) | W_first median | ratio | evals_first (= evals_min every race) | verdict |
| --- | --- | --- | --- | --- | --- |
| **m20_white** | 313.5 s, **budget-censored @ 2 B, no answer** | **95.89 s** | 0.31× (censored-win) | 791,401,553 (= plan12 s5 anchor; 0.40× of the 2 B burnt cap) | **conversion** (H1) |
| dec10 | 0.680 s / 4,262,128 evals | 0.522 s | **0.768×** (H2 ≤ 0.80) | 2,918,636 (0.685×, = plan12 s2) | win |
| rem12 (out of sample) | 9.410 s / 69,605,385 | 8.361 s | **0.889×** | 59,573,726 (0.856×, = stage-2 s3) | out-of-sample win |
| stress | 30.02 s / 249,480,478 | 31.396 s | 1.046× (H3 ≤ 1.10) | 252,069,909 (1.010×, = plan12 s5) | no regression |
| m22_white | 1.846 s / 14,156,269 | 1.962 s | 1.063× (H3) | 14,156,269 (1.000×, salt-0 first) | no regression |
| dec13 | 0.636 s / 3,822,602 | 0.670 s | 1.054× (H3) | 3,822,602 (1.000×, salt-0 first) | no regression (basin count 2 — cannot support adopt, only veto; it does not veto) |

- **H1 pass**: every m20 race had ≥ 1 uncensored decisive racer (all three:
  s5 at 791.4 M evals, s3 and s2 also decisive); W_first median 95.89 s ≤
  160 s gate. The shipped default run burns ≥ 2 B evals / ~313–340 s and
  returns nothing; the portfolio converts it to a proven win at ~96 s.
- **H2 pass** (dec10 0.768× ≤ 0.80×). The m23_white clause is recorded, not
  gated (not in the raced subset): evals prediction 0.976× from plan12.
  No contention anomaly.
- **H3 pass**: stress 1.046× / m22 1.063× / dec13 1.054× — the 4-way
  contention tax on the winning racer is ≤ 6.3 %, far inside the 1.10× gate.
- **H4 pass**: (a) 5/5 stage-0 identity; (b) 63/63 decisive racer runs
  reproduce their plan12/stage-2 anchor evals + outcome exactly (determinism
  survives concurrency); (c) zero decisive-outcome conflicts; (d) censored
  runs recorded censored (m20 salt-0 racer ×3), wall-killed recorded killed
  (stress s2/s3 ×3, killed at first-decisive + 240 s grace), evals excluded.

**Prediction vs measurement**: the frozen pre-computation held exactly —
evals_first = evals_min = the predicted basin minimum on every raced case, and
wall tracked evals (first finisher was the min-evals racer in all 18 races).

**Cost accounting**: m20 conversion costs ~747 core-s per race vs the shipped
solo's ~313 core-s that yields *no answer* — the first answer ever on this
case, at 96 s wall. Elsewhere the tax is ~4× core-seconds for ≤ 6 % wall
regression (or wins: dec10/rem12). Not-raced cases are covered by the
pointwise-dominance pre-computation (racers include salt 0; the 14
salt-invariant cases are the same computation 4×).
