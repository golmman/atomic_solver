# Plan 12 measurements (#19 pre-pass keys via upstream `hash_after`)

Executed 2026-10-10 on the x86_64 container (same host as plan11; see
`env.json`). All walls are sequential, base/new interleaved, `date` deltas;
identity = md5 of stdout.

## Files

| file | content |
| --- | --- |
| `quick_base.json` / `quick_plan12.json` | `benchmark --suite quick --json --first-outcome --timeout 3 --runs 1`, plan11 binary vs plan12; `child_evals`/`nodes`/`outcome` identical in 59/59 cases (totals 38,974,090 / 2,488,917 both) |
| `shuffle_fo_post_plan12_leaves.txt` | `perf report --no-children` leaf table, plan12 binary, shuffle-win first-outcome, `--timeout 20` (`hash_after` is fully inlined, no own leaf) |

## Wall A/B (interleaved, md5-verified per run)

| workload | plan11 base | plan12 | Δ |
| --- | --- | --- | --- |
| m22 default ×5 | 2.61–2.71 s (mean 2.65) | 2.29–2.35 s (mean 2.32) | **−12.5%** |
| m22 FO ×5 | 2.12–2.14 s (mean 2.13) | 1.85–1.86 s (mean 1.86) | **−12.9%** |
| shuffle-win FO ×2 | 34.38 / 34.38 s | 30.04 / 30.16 s | **−12.6%** |
| quick suite ×1 (time_mean total) | 5.606 s | 4.828 s | **−13.9%** |

Expectation was −5–10%; measured −12–14% on all four workloads.

## Pre-pass residual share (V12-style probe)

Throwaway /tmp build with the pre-pass loop duplicated (stdout md5
unchanged `b965d37c`): m22 default 2.316 → 2.437 s (+0.121 s) ⇒ the
pre-pass is **≈5.2% of m22-default wall** post-plan12 (≈14% post-plan11).

## Post-plan12 pie (shuffle-win FO, `--timeout 20`)

`dfpn` 32.1% (top leaf), `evaluate_child` 26.1%, `do_move` 9.1% +
`undo_move` 4.2%, existence cluster (`compute_checkers` 5.0% +
`has_legal_move_with_state` 5.0% + `populate_state` 3.4% + `legal` 3.1%)
≈22.6%, `generate_legal_with_state` 4.0%, sort leaves ≈3.9%, TT store 0.5%.
