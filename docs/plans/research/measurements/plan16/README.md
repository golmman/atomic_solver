# plan16 measurements (2026-10-10) — gated shaped-variant rollout of `mob-or` and `mobcap8` (item #18, Phase 1)

Evidence for [`../../report16.md`](../../report16.md). Zero product-code
changes remain: the temporary `ATOMIC_INIT` hook (init-only, at the `(1, 1)`
fallback site in `src/search/dfpn/children.rs`) plus the D2(d) helper
`examples/init_trace_check.rs` were added, measured, and reverted; the
pristine release build reproduces the plan12 salt-0 counts exactly before
and after (`git diff --exit-code` verified).

| artifact | role |
| --- | --- |
| `driver.py` | rollout driver: 3 arms × 22 cases × 6 salts = 396 runs; arm via the `ATOMIC_INIT` env var; resumable, ≤ 3 concurrent processes; `base` arm first (D2(a)/(b) gate) |
| `parse.py` | raw transcripts + Phase-0 anchors → `state/{runs,summary}.json`; hard-fails on any D2 invariant violation |
| `verdicts.py` | pinned-v1.0 verdict machinery (D3) + D4/D6 notes + paired-ratio dispersion → `state/verdicts.json` |
| `state/runs.json` | per-run records (arm, evals, outcome, censoring, wall) |
| `state/summary.json` | per-case-per-arm records + D2 check results (all pass) |
| `state/verdicts.json` | per-case verdict tables, lever verdicts, dispersion |
| `env.json` | environment, hook, run grid, soundness-invariant results |

Raw transcripts and anchor files (`/tmp/plan16/results`, `/tmp/plan16/raw`)
are not committed; `driver.py` regenerates them deterministically.

## Headline results (lever verdicts, pinned gate v1.0 applied verbatim)

**Both arms: reject.**

| arm | verdict drivers |
| --- | --- |
| `mob-or` | regressions: dec02 (1.78×), dec05 (1.26×), dec06 (2.15×), dec10 (1.07–1.76×); censored-loss: m23_white (3/6 salts), dec01 (catastrophic: 10–55× on uncensored pairs, 2 censored); hard-class wins: stress (censored-win), m22_white (win, median 0.67×); m20 unresolved under budget |
| `mobcap8` | regressions: dec05 (2.40×), dec10 (1.20–1.98×); hard-class wins: stress (censored-win), m22_white (win), m23_white (win, median 0.55×); dec01 win (0.45×); m20 unresolved under budget |

The plan15 nomination (stress/dec13 direction win + m22 ≤ 1.5× on 3/3
salts) replicates at gate grade — but the full-corpus rollout exposes
regressions plan15 never measured: `dec01` (catastrophic for `mob-or`),
`dec02`, `dec05`, `dec06`, and dec10 across the salt set. The dec10 caveat
(D6) was moot: neither arm's sole negative is dec10, and both arms have
regressions well outside the quiet band. No defer → the D4 salt-extension
rule does not fire.

## Verdict table (per case; ratios = candidate/baseline paired by salt)

See `state/verdicts.json` for the full tables (per-salt censoring patterns,
basin counts, ratios, dispersion). Summary:

| case | basins | mob-or | mobcap8 |
| --- | --- | --- | --- |
| m20_white | 4 | unresolved_under_budget | unresolved_under_budget |
| stress | 3 | censored-win | censored-win |
| m22_white | 6 | win | win |
| m23_white | 5 | censored-loss | win |
| dec01 | 6 | censored-loss | win |
| dec02 | 1 | regression 1.78× | unclear 0.98× |
| dec03 | 1 | win 0.075× | win 0.105× |
| dec04 | 1 | win 0.22× | win 0.34× |
| dec05 | 1 | regression 1.26× | regression 2.40× |
| dec06 | 1 | regression 2.15× | win 0.40× |
| dec07 | 1 | win 0.17× | win 0.21× |
| dec08 | 1 | unclear 1.00× | unclear 1.00× |
| dec09 | 1 | win 0.045× | win 0.045× |
| dec10 | 4 | regression 1.32× med | regression 1.48× med |
| dec11 | 1 | win 0.036× | win 0.14× |
| dec12 | 1 | win 0.097× | win 0.19× |
| dec13 | 2 | win 0.097× | win 0.10× |
| dec14 | 3 | win 0.072× | win 0.096× |
| dec15 | 1 | win 0.29× | win 0.23× |
| dec16 | 1 | win 0.086× | win 0.23× |
| dec17 | 1 | win 0.13× | win 0.28× |
| dec18 | 1 | win 0.039× | win 0.093× |

Per the pinned diversity clause, the many single-basin (`basins = 1`)
salt-invariant wins are exact per-salt effects (within-salt variance = 0)
and are real, but they cannot solely support an adopt; the rejects rest on
multi-basin regressions (dec10 basins=4, dec01 basins=6, m23 basins=5).

Paired-ratio dispersion (second real-arm calibration data point for the
comparison rule; the first was plan13's ε arms): `mob-or` n=118, median
0.169, p10 0.039, p90 1.78; `mobcap8` n=124, median 0.277, p10 0.093, p90
1.00. The shaped inits are strongly bimodal per case: big wins on most
decisive controls, isolated multi-x regressions on a handful of positions
(the init is a heuristic, and on some trajectories the steering is
destructive).
