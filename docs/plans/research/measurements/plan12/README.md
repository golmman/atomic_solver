# plan12 measurements (2026-10-10) — gate calibration + 22-case baseline rollout

Evidence for [`../../plan12.md`](../../plan12.md) (item #17 phase 2) and
[`../../gate_methodology.md`](../../gate_methodology.md) (pinned v1.0).
Runs the product CLI unchanged from plan11 (no `src/` changes in plan12).

| artifact | role |
| --- | --- |
| `driver.py` | runs the pre-registered grid: 22 cases × salts {0–4} (+5 after the D7 PIVOT fired), 3 concurrent processes, resumable |
| `parse.py` | raw outputs → `state/runs.json` + `state/summary.json` (D5 a/b/c checks, D6 correlation probe, per-case loud/quiet) |
| `calibrate.py` | `state/summary.json` → `state/calibration.json` (D8: basin counts, quiet noise band, pinned constants) |
| `state/runs.json` | all 132 runs: evals, decisive outcome, censoring reason, wall |
| `state/summary.json` | per-case record: per-salt draws, censoring, spread, loud flag, D6 probe result |
| `state/calibration.json` | per-case basin counts + hard/loud classification, quiet cross-salt noise band, pinned thresholds |
| `env.json` | environment, caps, salt definition, gate provenance, D7 PIVOT record |

Raw transcripts are not committed (regenerable: `driver.py` is
deterministic per run; plan11's PIVOT duplicate established that a rerun
reproduces stdout bit-for-bit, and this rollout's salt-0 values reproduce
plan11's exactly).

## Commands

```bash
cargo build --release
RAW=/tmp/plan12/results python3 driver.py
python3 parse.py /tmp/plan12/results
python3 calibrate.py
```

## Headline results (first-outcome child evals, 128 MB TT, 6 salt draws)

| case | median | spread | basins | loud | hard class |
| --- | --- | --- | --- | --- | --- |
| stress (m21_white) | 252,069,909 | 3.04× | 3 | **yes** | **yes** |
| m20_white | 886,953,635 | 1.75× | 4 | **yes** | **yes** |
| m22_white | 17,951,864 | 1.67× | 6 | **yes** | **yes** |
| m23_white | 11,398,560 | 1.28× | 5 | no | **yes** |
| dec10 | 3,901,852 | 1.65× | 4 | **yes** | no |
| dec01 | 5,757,577 | 1.25× | 6 | no | no |
| dec13 | 5,031,775 | 1.32× | 2 | no | no |
| dec14 | 2,443,946 | 1.04× | 3 | no | no |
| dec06 | 1,507,480 | 1.00× | 1 | no | no |
| other 14 dec | — | 1.00× | 1 | no | no |

- **Zero outcome flips**, every uncensored outcome matches its fixture
  expectation, salt-0 identity holds on all pilot cases (132 runs).
- **14 of 22 cases are salt-invariant** — the gate is trivially stable
  there; all sensitivity lives in the m-family + dec01/dec10/dec13/dec14.
- **Quiet cross-salt noise band [0.760, 1.316]** — a between-salt single
  comparison moves ±32% on pure noise: single-draw gates are retired.
- **D7 PIVOT fired**: salt pair (1, 4) near-identical (< 10⁻⁴ rel) on
  dec10 + dec13; salt 5 added per the pre-registered remedy; further
  near-twin pairs (1, 5), (3, 5) on low-diversity cases → per-case basin
  counts are now part of the pinned methodology (channel diversity is
  bounded by the case's trajectory structure, not by the salt count).
- **Pinned**: W_LO = 0.9 / W_HI = 1.1 (null degenerate at 1.0) + ≥ 75%
  direction agreement; hard class = stress, m20, m22, m23; under-budget =
  > 3 censored of 6; adopt/reject/defer rule in `gate_methodology.md`.
