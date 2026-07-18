# Plan 16 Implementation Report

## Summary

Added a `remaining_depth` field to the transposition table and integrated it
into `dfpn` so that depth-bound cutoff results are not reused for larger depth
budgets.  This fixes the `max_depth == 0` storage bug where a forced-win
position was incorrectly reported as a proven `Draw` and then reused as a draw
in a follow-up deeper search.

The key insight is that `depth` and `remaining_depth` are different:

- `depth` is the length of the shortest line that proves a Win/Loss.
- `remaining_depth` is the maximum depth budget for which the stored result is
  known to be valid.

A terminal position is valid for all budgets (`remaining_depth = u32::MAX`).
A Win/Loss found within a finite search is also proven (`remaining_depth =
u32::MAX`) because the result is real, but it is only a Win/Loss for budgets
at least `depth`.  A `Draw` stored because the search hit a depth bound is only
a cutoff and is valid for budgets up to the budget that produced it.

With this distinction `dfpn` can safely reuse transposition-table entries
across iterative-deepening calls without clearing the table between depths.

## Changes made

### `src/search/tt.rs`

- Added `remaining_depth: u32` to `TwinEntry` and `TtEntry`.
- Updated `TranspositionTable::store` and `TranspositionTable::store_twin` to
  accept and persist `remaining_depth`.
- Updated `TtEntry::store_twin` (internal) to record the field.
- Updated unit tests that exercise twin storage to pass the new argument
  (`u32::MAX` for the proven results used in those tests).

### `src/search/dfpn.rs`

- Updated all `tt.store`/`tt.store_twin` call sites to pass a `remaining_depth`:
  - Terminal positions: `u32::MAX` (proven for all budgets).
  - `max_depth == 0` cutoff draws: `0`.
  - Intermediate PV update for a solved Win/Loss: `u32::MAX`.
  - Final loop store: `u32::MAX` for Win/Loss, `max_depth` for Draw/unsolved.
  - Kawano simulation twin copy: the twin's own `remaining_depth`.

- Rewrote `try_use_tt` to use `remaining_depth` when deciding whether a cached
  result can be reused:
  - Reject any entry whose `remaining_depth < max_depth`.
  - For a stored `Draw`, return it for any budget up to `remaining_depth`.
    `depth` is not checked because a draw is a draw for all smaller budgets.
  - For a stored `Win`/`Loss`, return it only when `entry.depth <= max_depth`.
  - If `entry.depth > max_depth` but `remaining_depth == u32::MAX` (a proven
    result whose shortest line is longer than the current budget), return
    `Draw` as the correct cutoff result.

- Updated internal unit tests that call `store`/`store_twin` directly.

### `tests/test_review.rs`

The regression test `depth_zero_cutoff_is_not_reused_as_proven_draw` (added in
the previous Plan 16 work-in-progress) now passes.  It calls `search_depth(0)`
on a winning position, confirms the cutoff `Draw`, then calls `search_depth(3)`
on the same `Search`/transposition table and confirms the win is still found.

## Verification results

```text
$ cargo fmt                    # passed
$ cargo clippy --all-targets   # passed
$ cargo doc                    # passed
$ cargo test --release         # passed
$ cargo test --all-targets     # passed
```

Key tests that previously failed:

- `test_plan6::m27_white_wins` now passes.
- `test_plan6::m28_white_wins` now passes.
- `test_review::depth_zero_cutoff_is_not_reused_as_proven_draw` now passes.

CLI / example checks:

```text
$ timeout 5 cargo run --example solve_depth_limited --release -- \
    "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19" 2
outcome: Draw nodes: 766
d6f8
e8e4
```

The full `test_plan6` suite runs to completion with the expected pass/ignore
profile; the previously-failing m27 and m28 white/loss cases are now solved
within the 5-second limit.

## Remaining concerns

The `remaining_depth` scheme used here is intentionally conservative: every
solved `Draw` produced by a finite-budget `dfpn` call is stored with
`remaining_depth = max_depth`, even if the draw is actually proven (e.g. all
children are terminal draws).  This means a proven draw found at budget `N` is
not reused for budgets `> N`; the solver re-searches and confirms it.  This is
correct but slightly less efficient than propagating a proven-draw validity
from children.  A future refinement could track proven draws explicitly, but the
current behavior is sufficient to fix the reported failures and keep the table
sound for iterative deepening.
