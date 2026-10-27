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
- The same benchmark's censored cases show a small throughput gain:
  `m19` 29.72M → 30.67M child evals in 3 s (+3.2%), `startpos`
  27.89M → 28.61M (+2.6%). This is a solver-level sanity check, not a
  controlled upstream micro-benchmark.

## Verdict

Upgrade adopted. The patch is source-compatible, preserves move order and
search results on all solved benchmark cases, and moves a little more work
through the same wall-clock budget.
