# plan13 measurements (2026-10-11) — item #19 first arms: ε = 0.375 / 0.5 re-score

Evidence for [`../../plan13.md`](../../plan13.md) (item #19, first arms) and
[`../../gate_methodology.md`](../../gate_methodology.md) (pinned v1.0, first
real-arm application). Runs the product CLI unchanged (no `src/` changes;
the only per-arm difference is the single `--epsilon` option).

| artifact | role |
| --- | --- |
| `driver.py` | runs the pre-registered grid: 22 cases (frozen plan12 corpus) × salts {0–5} × 3 arms (base ε = 0.125, e375, e5) = 396 runs, 3 concurrent processes, resumable |
| `parse.py` | raw outputs → `state/runs.json` + `state/summary.json`; enforces the D3 soundness invariants (a) salt-0 identity, (b) fresh baseline ≡ plan12 per-salt records, (c) zero outcome conflicts + fixture match, (d) censored draw = no-result sentinel |
| `verdicts.py` | `state/summary.json` → `state/verdicts.json`: per-case paired-ratio verdict tables (D4), lever verdicts, paired-ratio dispersion (methodology calibration data) |
| `state/runs.json` | all 396 runs: arm, evals, decisive outcome, censoring reason, wall |
| `state/summary.json` | per-case-per-arm record: per-salt draws, censoring, decisive outcomes |
| `state/verdicts.json` | D4 verdicts + lever verdicts + dispersion summary |
| `env.json` | environment, caps, arm definition, D3 gate provenance |

Raw transcripts are not committed (regenerable: `driver.py` is
deterministic per (FEN, salt, ε) — confirmed by D3(b), where the fresh
baseline arm reproduced all 132 plan12 cells exactly).

## Commands

```bash
cargo build --release
RAW=/tmp/plan13/results python3 driver.py
python3 parse.py /tmp/plan13/results
python3 verdicts.py
```

## Headline results

**Both arms: lever verdict = reject** (any regression or censored-loss
vetoes adopt, per the pinned rule).

- **e375**: stress = **win** (fresh baseline censored at 2.5 B on salts
  2/3/4, e375 solves all three: 285 M / 329 M / 206 M) — but censored-loss
  on m22 (s3) and m23 (s0/s5), regressions on dec01 (up to **95×** on
  salt 5!), dec03, dec05, dec12, dec14; m20 under-budget (cand censored
  on 4 salts).
- **e5**: stress = **censored-win** (solves all three base-censored salts:
  266 M / 280 M / 315 M, and 0.36–0.63× where both uncensored) — but
  censored-loss on m23 (s4), regressions on dec01/dec02/dec03/dec05/
  dec11/dec12/dec13/dec15; m20 under-budget (cand censored on 5 salts —
  though it *solves* salt 0 at 389 M where the baseline is censored).
- The ε arms are **trajectory lotteries on the hard class**, not uniform
  improvements: e375 improves m23 on all four uncensored salts (0.53–0.72×)
  yet a *single* censored cell forces its censored-loss/reject.
- **Paired-ratio dispersion (first methodology calibration data point)**:
  e375 n=121, median 1.00, [p25, p75] = [0.850, 1.117], max 95.3;
  e5 n=122, median 1.02, [p25, p75] = [0.886, 1.385], max 2.94. 56% / 70%
  of pairs fall outside [0.9, 1.1]. On the 14 salt-invariant cases the
  ratios are exact (deterministic), so this spread is *real lever effect*,
  not noise — consistent with the pinned rule's premise, but it shows a
  single-arm median near 1.0 can hide massive per-case dispersion.
- plan4's single-draw readings are superseded: "ε = 0.375 Pareto-improves
  all four cases" is false (dec01 explodes, m22/m23 censor); "ε = 0.5 wins
  stress" is confirmed at gate grade (censored-win) but the control
  regressions are real and broad.
- **All D3 soundness invariants pass** (396 runs): fresh baseline
  reproduces plan12's per-salt cells exactly on all 22 × 6; zero outcome
  flips within any arm; every uncensored outcome matches its fixture.
