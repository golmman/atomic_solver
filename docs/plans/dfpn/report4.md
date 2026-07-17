# DF-PN+ Shortest Win PV Refinement - Report

## Goal

Allow the DF-PN+ solver in `atomic_solver` to refine the Principal Variation (PV) for a forced win and print the PV each time a shorter win/loss is found, instead of stopping at the first winning line.

## Problem

The solver would find any winning line and immediately stop. For the reported FEN

```
6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28
```

it produced the long PV

```
g8f8 c5c4 c3d5 c4c3 f3f4 c3d2 d6c5 d7d6 c5d6
```

instead of the shorter forced win

```
g8g7 e6f5 g7g6
```

The root cause was the `Win` early-out in `dfpn`: as soon as `is_solved_by_children` saw a `Loss` child, the search returned and never looked at siblings.

## Solution

### Root-only win refinement

In `src/search/dfpn.rs` the `dfpn` loop now only refines `Win` nodes at the root `OR` node (`is_or_node == true` and `path_stack.len() == 1`). When `refine_shortest` is enabled, a root `Win` continues searching unresolved siblings and prints the PV whenever a shorter win/loss is found. Non-root `Win` nodes still stop at the first winning line to keep the search practical. This is gated by the new `refine_shortest` field on `Search`.

### Stop at the shortest possible win

Initially the refinement would keep searching all siblings even after finding a 3-ply win. Now the solver searches with a depth budget. After finding a forced win, it searches again with a depth limit one ply shorter, repeating the process until the smallest depth that still yields the same outcome is found. This is a general search bound, not an atomic-chess-specific rule, and it avoids exploring the long `g8f8` subtree once the shorter `g8g7` line is known.

### Depth-bounded `dfpn`

`dfpn` now takes a `max_depth` parameter. When the remaining depth reaches zero, the search stores a bounded draw and returns, so recursive calls with a finite budget terminate instead of looping. `solve_refined` finds an initial win/loss without a depth bound, then clears the transposition table and runs a sequence of depth-bounded searches to find the shortest forced result.

### Depth-consistent transposition table usage

`try_use_tt` now checks `entry.depth` against the current `max_depth` and rejects table entries that are deeper than the current budget, so a bounded search does not accidentally reuse results from a deeper search.

### Last-PV fallback

`Search` now records the last printed PV. If `extract_pv` at the end of the search cannot reconstruct the PV chain from the transposition table, `solve` falls back to the last printed PV, so the final result is never shorter than the best line found.

### Configurable timeout

`Search` now stores a `timeout: Duration` and exposes `set_timeout`. `solve` uses `self.timeout` instead of a hard-coded 5s, so the CLI can afford longer refinements.

### Two-deep transposition table

While testing the reported FEN, the final `extract_pv` sometimes collapsed to a single move because `TranspositionTable::store` was overwriting entries with different keys. The table was changed to a two-deep bucket scheme: `probe` checks both slots, and `store` keeps the previous primary entry in the secondary slot while writing the new entry as primary. This preserves the PV chain entries during heavy searches.

### CLI

`src/main.rs` enables `refine_shortest(true)` and sets a 60s timeout so the binary prints live PV updates and ends with the shortest forced win.

## Result

Running the CLI on the reported FEN now prints:

```
outcome: win
pv: g8f8 c5c4 c3d5 c4c3 f3f4 c3d2 d6c5 d7d6 c5d6
nodes: 4774
outcome: win
pv: g8g7 e6f5 g7g6
nodes: 5130
outcome: win
pv: g8g7 e6f5 g7g6
```

The first winning line is printed, then the shorter `g8g7` line is found and printed as the final PV. The search now finishes almost immediately after the 3-ply win is found, instead of running to the timeout.

## Files Changed

- `src/search/dfpn.rs` - `refine_shortest`, `set_timeout`, depth-bounded `dfpn`, `solve_refined`, PV printing
- `src/search/tt.rs` - two-deep transposition table buckets and `clear`
- `src/main.rs` - enable refinement and 60s timeout
- `tests/test_plan4.rs` - regression test for the reported FEN

## Verification

- `cargo fmt --check` - clean
- `cargo clippy` - clean
- `cargo test` - all tests pass
  - `test_plan2` - 10/10 passing
  - `test_plan3` - 1/1 passing
  - `test_plan4` - 1/1 passing (≈0.20s)
- `cargo run --release` on the reported FEN now finishes with 5,130 nodes and the shortest PV.
