# plan19 measurements — TT-eviction arm V2 re-score (research #19, final arm)

Provenance: repo HEAD `c044d4e` ("add research plan19") + the temporary
`ATOMIC_TT_REPL=v2` hook in `src/search/tt/table.rs::insert_new` (applied,
measured, reverted in the same session; `git diff --exit-code` verified).
Host: x86_64 container, 4 cores. Build: release, LTO thin.

| file | role | committed |
|---|---|---|
| `driver.py` | rollout driver: 22 cases × 6 salts × {base, v2}, resumable, ≤ 3 concurrent | yes |
| `parse.py` | raw → `state/runs.json` + `state/summary.json`; enforces D3(b) 132-cell baseline identity and D3(d) outcome invariants (exit 1 on violation) | yes |
| `verdicts.py` | pinned v1.0 verdict machinery (D4) + lever verdict + D7 dispersion (fifth calibration point) | yes |
| `env.json` | run commands, hook spec, D3(a)/(c) fidelity record incl. the direction arbitration and the plan7 quick-anchor erratum candidate | yes |
| `state/runs.json` | per-run records (264) | yes |
| `state/summary.json` | per-case-per-arm aggregates + D3 checks | yes |
| `state/verdicts.json` | verdict tables + dispersion | yes |

Raw transcripts (`/tmp/plan19/...`) are not committed (`.gitignore`d per the
AGENTS.md measurement layout); the drivers plus the parsed state are the
reproducible record.
