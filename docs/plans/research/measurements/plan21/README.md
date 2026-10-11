# plan21 measurements — in-context child results (research #21, REJECT at gate grade)

Provenance: repo HEAD `ad900b3` ("add research plan21") + the temporary
`ATOMIC_INCTX` hook (`Search::inctx_probe` in `src/search/dfpn/mod.rs` +
the `evaluate_child` probe branch in `src/search/dfpn/children.rs` + the
O(1) child-context plumbing in `src/search/dfpn/core.rs`, applied,
measured, reverted in the same session; `git diff --exit-code` verified).
Host: x86_64 container, 4 cores. Build: release, LTO thin.

Phase 0 (liveness/fidelity/coverage) passed all kill gates; the full
pre-registered Phase-1 rollout was executed: 22 cases × 6 salts ×
{base, inctx} = 264 runs at ≤ 3 concurrency. The D3(b) baseline identity
held 132/132 cells against plan12; the D3(d) outcome invariants held on
all 264 runs (no outcome flip anywhere).

| file | role | committed |
|---|---|---|
| `driver.py` | rollout driver (22 cases × 6 salts × {base, inctx}; 264 runs) | yes |
| `parse.py` | raw → `state/` parser with the D3(b)/(d) hard gates | yes |
| `verdicts.py` | pinned-v1.0 verdict machinery (single candidate arm) + dispersion | yes |
| `env.json` | run commands, hook spec, D3(a)/(c) fidelity record, Phase-0 probe results, coverage quantification, kill-gate outcome | yes |
| `state/runs.json` | per-run records (264) | yes |
| `state/summary.json` | per-case-per-arm records + D3 check results | yes |
| `state/verdicts.json` | per-case verdict table, lever verdict, dispersion | yes |

Raw transcripts (`/tmp/plan21/...`: pristine identity runs, anchor chain,
effect probes, the C8-instrumented coverage probe, rollout runs) are not
committed (`.gitignore`d per the AGENTS.md measurement layout); `env.json`
records the parsed results, and the anchors are regenerable one-liners.

Phase-0 coverage probe note: the C8 counters (the reexamination's
instrumentation) were applied temporarily on top of the hook for the
coverage probe only and removed before the rollout; the hook-off C8
numbers replicate the reexamination's `r1_dec10` record exactly
(10,956 stale events / 1,624,604 evals-after-stale = 38.1 %), and the
hook-on numbers show the cache route captures 100 % of that mass
(0 / 0 at 340 hits from 2,134,858 probes).
