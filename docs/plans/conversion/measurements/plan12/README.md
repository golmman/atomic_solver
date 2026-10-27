# Plan 12 measurements — restart-schedule grid

Phase 1 of `docs/plans/conversion/plan12.md`: the restart-schedule grid run
against an **isolated copy** of HEAD carrying the temporary
`SPIKE_CHUNK_BASE` / `SPIKE_CHUNK_FACTOR` / `SPIKE_CHUNK_CAP` env gate in
`bounded_search`. The product tree was never patched; the copy was deleted
after the run.

## Provenance

| File | What it is | Reproduce |
| --- | --- | --- |
| `driver.py` | Arm × TT × case matrix runner (S0–S5), parses solver stdout/stderr and the `benchmark --suite quick` table | `python3 driver.py --root <isolated-build-tree> --out results` |
| `env.json` | Commit, host, toolchain, CPU/memory, TT sizes, ε, timeouts, gate description | — |
| `results/S0.json` … `results/S5.json` | Parsed per-cell results for one arm | produced by `driver.py` |
| `results/grid.json` | All arms in one file | produced by `driver.py` |

Raw solver transcripts (`logs/`, `*.out`, `*.err`) are deliberately not
committed per the repository measurement layout rules.

## Arms

| Arm | Env override | Meaning |
| --- | --- | --- |
| S0 | — | shipped `500_000` ×2, no cutoff (control) |
| S1 | `SPIKE_CHUNK_BASE=18446744073709551615` | no restart |
| S2 | `SPIKE_CHUNK_BASE=500000 SPIKE_CHUNK_FACTOR=1000` | single restart at 500k |
| S3 | `SPIKE_CHUNK_BASE=500000 SPIKE_CHUNK_CAP=5000000` | early-restart phase only |
| S4 | `SPIKE_CHUNK_BASE=500000 SPIKE_CHUNK_FACTOR=1.25` | many restarts |
| S5 | `SPIKE_CHUNK_BASE=500000 SPIKE_CHUNK_FACTOR=4` | few restarts |

## Headline

S0 reproduced the Phase 0 identity anchors exactly (all five single cases at
both TT sizes, both quick-suite totals, m22 default golden). No arm improved
stress FO by ≥10% at **both** TT sizes while keeping all outcomes: every
non-shipped arm either lost a case or timed out at tt 128. D1 = **keep S0**.
The full grid is in `results/grid.json`; the report tabulates it.
