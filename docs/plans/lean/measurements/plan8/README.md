# Plan 8 measurement artifacts

All wall measurements on the aarch64 container host, 2026-09-14. This host
runs ~10% slower than the reference host used for the post-plan6 profile and
shows higher wall variance (±5%), so all wall deltas were measured with
**interleaved A/B runs** (base/new binaries alternating in the same session).

## Phase 0 — attribution spike (temporary instrumentation, reverted)

- `spike_counters_m22.err` / `spike_counters_shuffle.err` — one run each of
  `atomic_solver --first-outcome --outcome-only` (`--timeout 30` m22,
  `--timeout 100` shuffle) with temporary `AtomicU64` counters in
  `sort_moves` / `score_with_context` / `nearest_commoner_map`
  (`ATOMIC_SOLVER_SPIKE=1` dump). Counters were reverted; the shipped diff
  contains none of them.
- `m22_first_outcome_instrumented_leaves.txt` — `perf report --no-children`
  leaf table of the same instrumented binary (m22 first-outcome).
- `m22_first_outcome_pre_plan8_leaves.txt` — clean binary, pre-change leaf
  table (baseline reference for the post-plan8 comparison).

## Drift evidence

- `quick_before.json` / `quick_after.json` — `benchmark --suite quick --json
  --first-outcome --runs 1`; `child_evals`, `nodes`, and `outcome`
  bit-identical in all 59 cases.
- `m22_fo_before_stdout.txt` / `m22_fo_after_stdout.txt` — m22 first-outcome
  stdout byte-identical (md5 `ea72f7ea…`).
- Shuffle-win first-outcome stdout byte-identical (md5 `ffd3d015…`; not
  copied here, recoverable via the command in `report8.md`).
- Slow-tier lean3 golden and the fast gate (`make test`) green.

## Phase 1 micro-benchmark (nearest_commoner_map variants, k = 1 and k = 4)

Temporary `examples/dbg_plan8_map.rs` (removed), 5M calls each, release:

| variant | k=1 ns/call | k=4 ns/call |
| --- | --- | --- |
| brute-force arithmetic (old) | 48.5 | 97.2 |
| const Chebyshev table (shipped) | **0.7** | **1.3** |
| multi-source BFS (plan's original prescription) | 175.9 | 187.0 |

## Wall comparison (interleaved A/B, median of 5 rounds)

- m22 first-outcome `--timeout 30`: base 3.72 s → plan8 **3.61 s** (−3.0%).
- Shuffle-win first-outcome `--timeout 100`: base 68.59 s → plan8
  **67.05 s** (−2.2%).

## Post-plan8 profile

- `m22_first_outcome_post_plan8_leaves.txt` (+ `.data`) — post-plan8 leaf
  table. `score_with_context` leaf 3.30% → 3.02% (pre → post). Note: on this
  aarch64 host, LTO inlining moves fragments between `populate_state` /
  `compute_checkers` leaves across builds; compare clusters, not single
  leaves.
