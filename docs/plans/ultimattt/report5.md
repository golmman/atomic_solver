# Report: Plan 5 — Adopt `ultimattt`-style work-bounded iterative deepening

This report documents the implementation of `docs/plans/ultimattt/plan5.md`.

## Summary

Replaced the hybrid depth/work `solve_outcome` bootstrap with a pure
work-bounded iterative-deepening loop.  Each `dfpn` probe now uses
`max_depth = u32::MAX` and a doubling `max_work` chunk, reusing the
transposition table, history, and killer tables between chunks and resetting
only path-dependent state.  `refine_sppv` now binary-searches the depth bound,
and `max_work` enforcement in `dfpn` was hardened so that work-bounded calls
stop cleanly before recursing with an exhausted budget.

This prioritizes decisive outcomes for deep positions; `bootstrap_success_depth`
is concrete after a decisive outcome, and `bootstrap_fail_depth` is set to `0`
because a pure work-bounded loop has no reliable deepest-searched depth.

## Changes applied

### `src/search/dfpn/mod.rs`

- `solve_outcome` was rewritten:
  - Removed the fixed `max_depth` schedule.
  - Runs `dfpn` with `max_depth = u32::MAX` and a doubling work chunk starting
    at `500_000`.
  - Retains the transposition table and move-ordering state between chunks.
  - Records `bootstrap_success_depth` from the decisive root TT entry, then a
    validated PV length, then `max_ply` as a last resort.
  - Sets `bootstrap_fail_depth = 0`.
  - Includes an unbounded fallback (`max_work = u64::MAX`) if the work loop
    doubles to `u64::MAX` without a decisive result.
- `refine_sppv` was rewritten:
  - Binary-searches `[lo, hi]` with `probe = lo + (hi - lo) / 2`.
  - Uses three doubling-work retries per probe.
  - Initializes `current_best_len` to `hi` when `last_pv` is empty so that any
    proven shorter PV is recorded.
  - Updates `hi`/`lo` after the retry loop rather than inside it.
- `Search::solve` (the `!refine_shortest` branch) now also extracts a concrete
  `bootstrap_success_depth` using the same precedence as `solve_outcome`
  instead of defaulting to `u32::MAX`.
- Removed the now-unused `reset_history_and_killers` helper.
- Added per-chunk logging before `max_work` is increased in `solve_outcome`
  and `refine_sppv`, including work done, elapsed time, deepest ply reached,
  total nodes, and nodes per second.
- Added `max_depth_reached` tracking via `path_push` and `reset_search_state`.

### `src/search/dfpn/core.rs`

- Hardened `max_work` enforcement: before the recursive `dfpn` call, the
  search checks whether `work_spent >= max_work` and breaks if so, then passes
  the remaining budget (`max_work.saturating_sub(work_spent)`) to the child.
- This guarantees that a work-bounded `dfpn` call never recurses with a zero
  or negative remaining budget.

### `src/search/dfpn/children.rs`

- Added a comment explaining that `remaining_depth == u32::MAX` on an unsolved
  transposition-table entry means an unbounded work cutoff.  With
  `max_depth == u32::MAX` during the bootstrap this only applies to the root,
  which is never evaluated here, while deeper entries carry
  `u32::MAX - ply` matching `child_max_depth` on reuse.  The `<=` guard still
  rejects over-deep summaries during finite `refine_sppv` probes.

### `src/main.rs`

- No change.  The staged `solve_outcome -> find_ppv -> refine_sppv` sequence
  is preserved, and `main.rs` continues to use `Search::solve_outcome`
  directly rather than `Search::solve`.

### `examples/benchmark.rs`

- No change.  The benchmark already uses `Search::solve`, which calls
  `solve_outcome` when `refine_shortest` is `true`.

## Verification

Standard quality checks:

```bash
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo test --release
$ cargo doc --no-deps
```

- `cargo fmt` completed with no changes.
- `cargo clippy --all-targets` reported zero warnings.
- `cargo doc --no-deps` built cleanly.
- `cargo test` (debug) and `cargo test --release` showed two failing tests in
  `test_plan6`:
  - `m27_ppv_only` expected the `--no-refine-shortest` PV to start with
    `b1b8 g8f7`; the solver produced a 11-plies PPV starting with
    `b1b8 g8h7`.
  - `m27_streaming_output` expected the PPV after `find_ppv` to be 7 plies;
    it received the 11-plies PPV.
- `m27_shortest_pv` **passed** in release and still reports a 7-plies win.
- `m24_ppv` (run with `--ignored`) **passed** in release within 60 seconds.

### Regression checks

All four regression FENs returned decisive outcomes within 60 seconds:

```text
$ cargo run --release -- --fen '6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25' --timeout 60
outcome: loss
pv: g8g7 b1b8 g7h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6
sppv search finished
warning: PV validation failed for Loss

$ cargo run --release -- --fen '6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26' --timeout 60 --no-refine-shortest
outcome: win
pv: b1b8 g8h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6

$ cargo run --release -- --fen '4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24' --timeout 60
outcome: win
pv: e3f4 e8b8 e1e8 b8e8 b1b8 g8h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6
sppv search finished

$ cargo run --release -- --fen '4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K b - - 0 24' --timeout 60
outcome: loss
pv: <long loss line>
sppv search finished
```

Decisive outcomes were returned for all four positions.  The `fen1` run
emitted `warning: PV validation failed for Loss` during `refine_sppv`; the
PPV printed by `find_ppv` was returned without a warning, but
`verify_ppv` reports that the line is not a true longest-defense PPV
(`g7h7` at defender ply 3 is not a longest defense).  This appears to be a
pre-existing PPV extraction issue that is exposed by the larger
`bootstrap_success_depth` values produced by the work-bounded bootstrap.

### Benchmark

Without refinement:

```text
runs=10 timeout=5s epsilon=0.125 refine_shortest=false

| name | outcome | nodes | child_evals | mean (s) | min (s) | max (s) | pv_len |
|------|---------|------:|------------:|---------:|--------:|--------:|-------:|
| two_rook_mate | win | 6 | 35 | 0.000 | 0.000 | 0.000 | 3 |
| epsilon_mate | win | 533 | 11582 | 0.003 | 0.003 | 0.003 | 5 |
| promotion_transposition | win | 819 | 6601 | 0.001 | 0.001 | 0.002 | 15 |
| m26 | win | 299 | 2461 | 0.001 | 0.000 | 0.001 | 11 |
| opening_f2 | win | 658 | 13675 | 0.004 | 0.003 | 0.004 | 7 |
| rook_pawn_endgame | win | 714 | 5268 | 0.001 | 0.001 | 0.001 | 9 |
| m19 | draw | 869556 | 17895496 | 5.000 | 5.000 | 5.000 | 0 |
| startpos | draw | 695456 | 17004649 | 5.000 | 5.000 | 5.000 | 0 |
```

With refinement:

```text
runs=10 timeout=5s epsilon=0.125 refine_shortest=true

| name | outcome | nodes | child_evals | mean (s) | min (s) | max (s) | pv_len |
|------|---------|------:|------------:|---------:|--------:|--------:|-------:|
| two_rook_mate | win | 303 | 649 | 0.000 | 0.000 | 0.000 | 3 |
| epsilon_mate | win | 1525812 | 3749402 | 1.288 | 1.283 | 1.300 | 5 |
| promotion_transposition | win | 1227812 | 3514575 | 0.733 | 0.723 | 0.745 | 7 |
| m26 | win | 3305039 | 10002555 | 2.441 | 2.431 | 2.459 | 7 |
| opening_f2 | win | 1314579 | 7151565 | 1.949 | 1.939 | 1.976 | 7 |
| rook_pawn_endgame | win | 1442898 | 5270049 | 1.288 | 1.263 | 1.314 | 7 |
| m19 | draw | 841354 | 17357693 | 5.000 | 5.000 | 5.000 | 0 |
| startpos | draw | 694304 | 16953522 | 5.000 | 5.000 | 5.000 | 0 |
```

The non-refined benchmark is comparable to the Plan 4 baseline for the
shallow decisive positions.  The refined benchmark spends extra nodes to
tighten PVs, reducing `pv_len` for `m26` (11 -> 7) and
`promotion_transposition` (15 -> 7), which is the expected behavior.

## Tools and examples used

- `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo test --release`,
  `cargo doc --no-deps` for the standard quality checks.
- `examples/benchmark.rs` for the release benchmark runs.
- `examples/verify_ppv.rs` to check whether the printed PVs are genuine
  longest-defense PPVs.  It refuted both the `fen1` and m27 Win PPVs, showing
  that `find_ppv` can return a winning line that is not a true PPV.
- `examples/play_and_solve.rs` and `examples/solve_depth_limited.rs` to inspect
  particular child positions during debugging.

## Problems encountered

- The work-bounded bootstrap proves decisive results quickly, but the first
  proven Win is an upper-bound depth, not the shortest mate.  `find_ppv` then
  extracts from transposition-table entries whose `best_move` was chosen during
  `Outcome`-mode search and may not represent the longest defense.  This
  produces a printed PV that `extract_pv_checked` accepts (terminal outcome is
  correct) but `verify_ppv` rejects (a defender reply is not a longest
  defense).
- `m27_ppv_only` and `m27_streaming_output` fail because they expect the PPV
  to already be the shortest line.  Under the new design `find_ppv` returns the
  line found by the bootstrap, and `refine_sppv` tightens it; without refinement
  the longer line is emitted.
- Debug builds of the active `test_plan6` m27 tests exceed the 5-second timeout,
  although release builds finish `m27_shortest_pv` and `m24_ppv` within the
  limits.

## Open ends and next steps

- Decide whether `find_ppv` should guarantee a true longest-defense PPV.  If
  so, add a re-search pass that clears or bypasses `Outcome`-mode transposition
  entries so `ProofMode::Ppv` can select the correct defender replies.  A
  smaller change is to make `find_ppv` return the shortest proven line (SPPV
  semantics), which would satisfy the existing `m27_ppv_only` and
  `m27_streaming_output` tests but changes the PPV/SPPV contract.
- Tighten `extract_pv_internal` so it does not fall back to a transposition
  entry whose `depth` does not match the remaining plies in the line.  This
  would prevent following mismatched `best_move`/`depth` pairs.
- Update or annotate `m27_ppv_only` and `m27_streaming_output` if the project
  accepts non-shortest PPVs in PPV mode.
- Re-run the full release test matrix once a PPV-extraction fix is in place;
  the work-bounded bootstrap is sound for outcomes, but PV quality needs an
  additional pass.

## Files changed

- `src/search/dfpn/mod.rs`
- `src/search/dfpn/core.rs`
- `src/search/dfpn/children.rs`
- `docs/plans/ultimattt/report5.md` (this report)
