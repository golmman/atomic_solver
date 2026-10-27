# Upstream report: `atomic-movegen` 2.3.1 (performance-only patch)

Recorded 2026-10-27. No new upstream plan; this is the consumer-side update
report for the patch release.

## Release

- **Published version:** `atomic-movegen` 2.3.1 on crates.io
  (2026-10-27), semver-patch, no public API change.
- The consumer's requirement was already `"2.3"`; only `Cargo.lock` changed
  (`cargo update -p atomic-movegen`, 2.3.0 → 2.3.1).

## Upstream changes

Internal, behavior-preserving performance work:

- Pawn generation is table-driven and order-preserving: push, double-push,
  capture, and en-passant targets now come from compile-time tables instead
  of per-target square arithmetic, wrap guards, and repeated board lookups
  (upstream: ≈6.9% faster on fixed depth-6 perft).
- Commoner-safety and castling attack detection use a boolean early-exit
  probe instead of materializing an attacker bitboard (≈1.3–1.6%).
- The trivial-legality predicate is hoisted per node (`TrivialLegality`) and
  classifies candidates from precomputed state (≈2.2–2.3%).
- Additional move-order golden positions and pawn-table unit tests.

The changed files are `src/attacks.rs`, `src/board.rs`, `src/movegen.rs`,
`src/types.rs`, and `tests/move_order_golden.rs`. There are no signature or
semantic changes; `Move`/`MoveList` order is explicitly covered by upstream
goldens.

## Correctness gates

- `make test` — green (fast gate).
- Targeted slow tier after the bump:
  - `test_move_order` with `--include-ignored` — 3/3 green (32 s).
  - `test_trajectory_golden` with `--include-ignored` — green.
- Consumer benchmark, default suite, `--first-outcome --timeout 3 --runs 2`,
  2.3.0 vs 2.3.1: every non-timeout case is identical in
  `child_evals`, `nodes`, `outcome`, and `pv_len` (8 cases; the two
  wall-clock-censored cases, `m19` and `startpos`, are expected to differ
  because more work fits in the fixed timeout).

## Performance confirmation

Measured 2026-10-27 on the container host (aarch64, release builds,
rustc 1.99.1). Both solver binaries were built from identical source with
only `Cargo.lock` toggled between 2.3.0 and 2.3.1. Runs were pinned with
`taskset -c 0` and alternated between versions; the table reports medians.

### Raw movegen: depth-6 perft

The crate's own `perft` example was built from the two crates.io sources.
Node counts were identical to the upstream expected values.

| Position | Nodes | 2.3.0 | 2.3.1 | Time change | Speedup |
| --- | ---: | ---: | ---: | ---: | ---: |
| startpos | 118,926,425 | 0.4417 s | 0.3900 s | −11.7% | +13.3% |
| perft #3 | 146,715,064 | 0.7480 s | 0.6819 s | −8.8% | +9.7% |
| perft #10 | 38,665,634 | 0.2250 s | 0.2203 s | −2.1% | +2.1% |
| perft #14 | 245,931,633 | 1.0393 s | 0.9565 s | −8.0% | +8.7% |
| perft #13 | 2,160,817,389 | 8.3785 s | 7.5352 s | −10.1% | +11.2% |

This directly confirms the upstream claim that the patch speeds up move
generation. The gain varies by position (pawns present, capture density,
legal-vs-pseudo-legal workload); the two long positions are around
9–11%.

### Solver: fixed child-eval budget

Fixed work makes wall-time comparison meaningful. Each run used
`--budget 50000000 --timeout 60 --outcome-only`, so every run exhausted the
same deterministic budget: `m19` 50,000,011 evals and startpos 50,000,001
evals, with identical output in both versions.

| Case | Metric | 2.3.0 | 2.3.1 | Change |
| --- | --- | ---: | ---: | ---: |
| m19 | wall median | 5.1292 s | 4.9080 s | −4.31% |
| m19 | CPU median | 4.9945 s | 4.7885 s | −4.12% |
| startpos | wall median | 5.3465 s | 5.2653 s | −1.52% |
| startpos | CPU median | 5.2179 s | 5.1412 s | −1.47% |

The solver-level gain is smaller than the raw movegen gain because move
generation is only part of search time. The earlier timeout-based check
(`--timeout 3`) was not a reliable estimate: it compared different amounts
of work reached before the wall-clock cut. The fixed-budget experiment
above is the controlled measurement.

## Verdict

Upgrade adopted. The patch is source-compatible, preserves move order and
all measured search results, confirms a real movegen speedup, and yields a
small but repeatable solver-level speedup under fixed work.
