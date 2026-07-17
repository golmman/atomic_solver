# Plan 13 Implementation Report

This report documents the implementation of `docs/plans/review/plan13.md`, which
adds dedicated GHI (graph-history interaction) regression tests to protect the
changes from plans 10-12.

## Changes made

### `src/search/dfpn.rs`

- Added `pub fn twin_stats(&self) -> (u64, u64)` on `Search` so integration
tests can observe twin insertions if desired.

### `src/search/tt.rs`

- Added `PartialEq` and `Eq` to `EntryResult` so unit tests can compare lookup
results directly.
- Added `find_and_best_result_for_multiple_paths` unit test: stores twins for
multiple path codes and verifies that `find_result_for_path` and
`best_result_for_path` return the correct twin only for the matching path code.

### `tests/test_ghi.rs` (new)

- `promotion_transposition_outcome_is_consistent`: solves the two-pawn promotion
start `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` (white to move, expected `Win`) and then
reuses the same `Search` (and TT) to solve the transposed board
`QQ2k3/8/8/8/8/8/8/4K3 b - - 0 1` (black to move, expected `Loss`). Verifies
that both runs produce decisive outcomes and non-empty PVs.
- `cyclic_rook_position_does_not_claim_win` (ignored by default): the rook-safe-area
draw `8/8/8/8/2k5/8/8/4KR2 w - - 0 1` must not be reported as `Win`.
- `reversible_cycle_does_not_claim_win` (ignored by default): performs a
reversible rook/king shuffle that returns to the same board with a higher
`rule50` and asserts the solver still does not claim `Win`.

### `tests/test_epsilon.rs`

- Added `epsilon_thresholds_do_not_claim_win_in_cyclic_position` (ignored in
debug builds): runs the rook-safe-area cyclic position with `ε = 0.0`, `0.25`,
and `0.5` and asserts none of them incorrectly claim `Win`, guarding the
threshold fix from Plan 9 against GHI-sensitive positions.

### Existing tests

- `tests/test_repetition.rs` (from Plan 12) continues to cover the
`repetition_key` separation with a reversible move cycle.
- `src/search/dfpn.rs` already contains the Plan 11 unit test
`try_use_tt_accepts_cross_path_win_twin`.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
$ cargo test --release
$ cargo test --release --test test_ghi -- --ignored
```

All commands completed successfully.

## Test-suite timing notes

- The default `cargo test` (debug) skips the slow cyclic GHI tests and the
`test_plan6` deep mate-suite tests, finishing quickly.
- `cargo test --release` runs the cyclic epsilon cross-check (three solves of
the rook-safe-area position), adding about 15 seconds.
- `cargo test --release --test test_ghi -- --ignored` exercises the two
ignored cyclic GHI tests and passes in about 5 seconds.

## Limitations

- A true **path-dependent transposition** (same board reached by two move
orders where one path allows a winning move and the other does not because of
repetition rights) is difficult to construct for atomic chess. The current
regression suite focuses on cross-path consistency and cyclic non-win assertions
instead.
- Running `cargo test --release -- --ignored` also runs the previously
ignored `test_plan6` deep mates, which still fail with the default 5-second
timeout because they require a longer search. The GHI-specific ignored tests
should be run with the targeted command `cargo test --release --test test_ghi -- --ignored`.
