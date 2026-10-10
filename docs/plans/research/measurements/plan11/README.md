# plan11 measurements (2026-10-10) — salt-seeded statistical gate pilot

Evidence for [`../../plan11.md`](../../plan11.md) (item #17) and
[`../../gate_methodology.md`](../../gate_methodology.md). Runs the product
CLI (no probe patch): the `PROBE_SALT` mechanism from `measurements/reexam/`
was promoted into the `--salt` CLI option / `Search::set_salt` in phase 0 of
this plan, with the D5 identity gates re-run on the product surface.

| artifact | role |
| --- | --- |
| `driver.py` | runs the D4 grid: 6 cases × 5 salts (0–4) + the PIVOT duplicate, 3 concurrent processes, resumable |
| `parse.py` | raw outputs → `state/runs.json` + `state/summary.json` (D6 analysis, D5(c) flip check) |
| `state/runs.json` | all 31 runs: evals, decisive outcome, censoring reason, wall |
| `state/summary.json` | per-case min/median/max over uncensored draws, spread, salt-sensitivity classification |
| `state/bench_identity_pre.json` | D5(a) baseline: `benchmark --suite quick --json --first-outcome` (defaults), timing fields stripped |
| `state/bench_identity_post.json` | D5(a) post-change; byte-identical to `*_pre.json` on every trajectory field |
| `env.json` | environment, caps, salt definition, gate provenance |

Raw transcripts are not committed (regenerable: `driver.py` is deterministic
per run — the budgeted first-outcome search has no wall-clock dependence, and
the PIVOT duplicate confirmed a rerun reproduces stdout bit-for-bit).

## Commands

```bash
cargo build --release
RAW=/tmp/plan11/results python3 driver.py
python3 parse.py /tmp/plan11/results
```

## Headline results (first-outcome child evals, 128 MB TT)

| case | salt 0 | salts 1–4 | spread (uncensored) | sensitive |
| --- | --- | --- | --- | --- |
| stress (m21_white) | **249,480,478** | 758,176,477; censored ×3 (2.5 B) | 3.04× | **yes** |
| m20_white | censored (1 B) | censored ×2; 861.8 M, 912.1 M | — (2 draws) | **yes** (mixed) |
| m22_white | 14,156,269 | 15.8–23.7 M | 1.67× | **yes** |
| m23_white | 9,673,403 | 9.4–12.1 M | 1.28× | no |
| dec13 | 3,822,602 | 5.03 M ×4 | 1.32× | no |
| dec10 | 4,262,128 | 2.9–4.8 M | 1.65× | **yes** (barely) |

- Salt 0 reproduces every reexamination baseline exactly: stress
  249,480,478; m22 14,156,269; dec13 3,822,602; dec10 4,262,128. The
  product-surface salt and the throwaway probe are the same mechanism.
- Zero outcome flips (decisive outcomes agree across salts everywhere).
- New data vs. reexamination: **m20_white** (not in the reexam corpus) is
  the hardest shape of all — the shipped salt-0 trajectory does *not* solve
  it at the 1 B budget while salts 3/4 do. The fixture note "hardest" was
  measuring one unfavourable draw.
- PIVOT check: duplicate stress salt-1 run is bit-identical (within-salt
  variance = 0) — the salt channel is a clean, deterministic noise
  realization; no additional noise channel is required before rollout.
