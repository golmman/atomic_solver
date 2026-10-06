# Plan 10 measurement artifacts

Sweep of the history/killer ordering constants (lean plan10, backlog #10),
release build, `--first-outcome`, aarch64 container host, 2026-09-15.
Instrumentation: temporary env-gated constant overrides (`LEAN10_*`) plus the
`LEAN10_STATS` sort_moves reorder diagnostic, applied on top of HEAD
(`c86d61e`), measured, and **fully reverted** — `src/` is byte-identical to
the pre-spike tree (`git diff` empty) and the post-revert m22 first-outcome
stdout md5 matches the step-0 anchor (`ea72f7ea…`, the report8/report9
value). Raw transcripts were deleted per AGENTS.md; the parsed record is
authoritative.

## Files

- `arms.csv` — every measured arm: env override, total quick-suite
  `child_evals` (ok cases), % vs default, timeouts/wrong, outcome-status
  flips, worst per-case regression vs default (> +10% only), phase
  (0 = baselines/hook probes, 1 = OFAT screen, 2 = cross-product).
- `results/quick_baseline.json`, `results/thorough_baseline.json` — step-0
  default captures (quick: 38,974,090 total evals, 59/59 ok; thorough:
  60 ok + 5 timeout cases at `--timeout 5`).
- `results/quick_arm_*.json` — one benchmark JSON per arm
  (`--suite quick --first-outcome --timeout 3 --runs 1`).
- `state/diag_quick_summary.json` — phase-0 `LEAN10_STATS=1` diagnostic
  aggregate: 482,956 of 2,477,336 `sort_moves` calls (19.5%) had the
  history+killer layer change the static-only top-1 pick.
- `env.json` — environment, exact commands, and the env-override contract.

## Commands

```bash
# non-perturbation anchor (byte-identical pre-spike and post-revert)
target/release/atomic_solver --fen '<m22_white FEN>' --timeout 30 \
  --first-outcome --outcome-only      # md5 ea72f7eae2437867c986dbf71268223f

# suite arms
LEAN10_<CONST>=<v> target/release/examples/benchmark \
  --suite quick --first-outcome --timeout 3 --runs 1 --json

# phase-0 diagnostic (per quick-suite case; FEN list was generated from
# tests/fixtures/decisive_positions.txt + move_order m23..m29)
LEAN10_STATS=1 target/release/atomic_solver --fen <FEN> --timeout 3 \
  --first-outcome --outcome-only      # stderr: spike10[...]: counters
```

## Headline numbers (quick suite, total `child_evals`, ok cases)

Default baseline **38,974,090**. Best arms and their gate-2 failures:

| arm | total | Δ | verdict |
| --- | --- | --- | --- |
| hs2-ag1 (BONUS 200, MAX 20000, age 1) | 31,345,410 | −19.6% | m23_white ok→timeout; dec05 5.19× |
| hs2-ag5 | 31,303,032 | −19.7% | dec01 ok→timeout |
| ag1 | 31,493,500 | −19.2% | m23_white ok→timeout; dec05 5.19× |
| hs05-ag100 (only −3%+ arm with zero flips) | 37,679,579 | −3.3% | dec14 1.59×, m24_white 1.33× |

Every arm with a material aggregate win violates pre-registered gate 2; the
flip-free runner-up fails it too. Decision: **no-go** (see `report10.md`).
