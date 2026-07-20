# Shortest-PV Fix for the DF-PN+ Solver - Report

## Summary

Implemented `docs/plans/pv/plan1.md` in `atomic_solver`. The reported position

```text
6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26
```

now returns a 7-plies winning principal variation starting with `b1b8 g8f7`, and the child position after `1.Rb8+ Kh7` is solved in 3 plies with `b8g8 c5c4 g8g6`.

Key changes:

- **Depth-aware DF-PN loop** (`src/search/dfpn/core.rs`): winning OR nodes now keep searching for a shorter win instead of exiting on the first proven child; losing AND nodes keep the longest known losing child as the most resistant defense.
- **`is_solved_by_children` consistency tests** (`src/search/dfpn/selection.rs`): added regression tests for mixed `Win`/`Draw` children returning `Draw` and for mixed `Win` depths returning the longest `Loss`.
- **Iterative-deepening refinement** (`src/search/dfpn/mod.rs`): `solve_refined` now probes downward from the bootstrap success depth, reusing the transposition table and move-ordering tables between probes and clearing only path state.
- **Atomic-check move ordering** (`src/search/ordering.rs`): `StaticAtomicScorer` now rewards moves that attack a square adjacent to the lone enemy commoner, helping the solver prefer fast atomic checks such as `b8g8` over slower direct checks such as `b8h8`.
- **PV/depth warning** (`src/search/dfpn/pv.rs`): `extract_pv_checked` emits a warning when an extracted PV length does not match an explicitly expected depth.
- **TT reuse safety** (`src/search/dfpn/simulate.rs`): `try_use_tt` no longer returns `Outcome::Draw` for a solved win/loss whose stored depth exceeds the current `max_depth`; it returns `None` so the search re-probes with the tighter bound.
- **Regression tests** (`tests/test_plan6.rs`): added `m27_shortest_pv` and `m27_kh7_fast_win`.

## Files Changed

| File | What changed |
|------|--------------|
| `src/search/dfpn/core.rs` | Added `best_win_depth` / `best_loss_depth` tracking; updated the solved-outcome block to keep searching for shorter wins when `refine_shortest` is enabled and to keep the longest losing child. |
| `src/search/dfpn/selection.rs` | Added unit tests for mixed `Win`/`Draw` children and mixed `Win` depths. |
| `src/search/dfpn/mod.rs` | Bootstrap now records `success_depth` and `fail_depth`; `solve_refined` performs iterative-deepening downward with TT/history reuse; old binary-refinement code kept as `solve_refined_unbounded` fallback. |
| `src/search/ordering.rs` | Added `SCORE_ATOMIC_CHECK` and an adjacent-commoner threat branch in `StaticAtomicScorer::score`. |
| `src/search/dfpn/pv.rs` | Added a length-vs-depth warning in `extract_pv_checked`. |
| `src/search/dfpn/simulate.rs` | Removed the `remaining_depth == u32::MAX => Draw` fallback and added a `twin.depth > max_depth` guard for cross-path twin simulation. |
| `tests/test_plan6.rs` | Added `m27_shortest_pv` and `m27_kh7_fast_win` regression tests. |
| `docs/plans/pv/report1.md` | This report. |

## Algorithm Details

### Depth-aware DF-PN loop

When `select_children` reports a solved `Win` child, `core.rs` now records the shortest depth seen so far and only breaks if:

- all children are solved, or
- `refine_shortest` is disabled (the old behavior).

For a solved `Loss` child it records the longest depth (most resistant defense). The final `tt.store` call uses the running best move/depth, so the transposition table propagates mate distance and `extract_pv` can follow it.

### Iterative-deepening refinement

`Search::solve` bootstraps with `refine_shortest = false`, doubling `max_depth` until a decisive result is found. It records the smallest decisive `max_depth` as `success_depth` and the largest `Draw` `max_depth` as `fail_depth`.

` solve_refined` then probes `max_depth = success_depth - 1, success_depth - 2, ...` while `hi > lo + 1` and time remains. Each probe calls `dfpn(pos, INF, INF, max_depth, true)`, reusing the existing TT and history/killer tables and only clearing the `path`/`path_stack`/`path_code`. The shortest PV with `outcome == best_outcome` is retained. If the bootstrap did not find a decisive result, the solver falls back to the previous unbounded binary-refinement routine.

### Atomic-check scoring

`StaticAtomicScorer` first checks for direct attacks on an enemy commoner. When the opponent has exactly one commoner and the moved piece does not attack that commoner, the scorer also checks whether the piece attacks any of the eight surrounding squares; if so, it adds `SCORE_ATOMIC_CHECK` (9,000). This sits below `SCORE_THREAT_LAST` (10,000) but above `SCORE_THREAT` (1,000), so direct checks still come first while atomic restriction moves such as `b8g8` are explored before quiet moves.

## Test Results

```bash
cargo fmt
cargo clippy --all-targets
cargo test --release
cargo doc
```

All clean and passing.

Selected output from `cargo test --release --test test_plan6`:

```text
running 25 tests
...
test m27_kh7_fast_win ... ok
test m27_shortest_pv ... ok
test m27_white_wins ... ok

test result: ok. 9 passed; 0 failed; 16 ignored
```

Full `cargo test --release` passed with no new warnings or failures.

## Manual Verification

Reported FEN:

```bash
cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"
```

Output:

```text
outcome: win
pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8
```

Child after `1.Rb8+ Kh7`:

```bash
cargo run --release --example solve_depth_limited -- \
    "1R6/3p3c/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7C w - - 2 27" 3
```

Output:

```text
outcome: Win nodes: 38
b8g8
c5c4
g8g6
```

## Known Limitations / Future Work

- The previously ignored `m19`–`m26` and `m29` prompt positions in `tests/test_plan6.rs` remain `#[ignore]`; they were re-evaluated conceptually but are not within the 5-second search limit for this task.
- Iterative-deepening TT reuse is correct for the reported position, but cross-path twins with incompatible depths are now skipped rather than simulated under a tighter bound. A future improvement could re-simulate them with the current `max_depth` limit.
- `SCORE_ATOMIC_CHECK` is a single global constant; per-position tuning could further improve move ordering on other positions.
