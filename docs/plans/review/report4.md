# Plan 4 Implementation Report

This report documents the implementation of `docs/plans/review/plan4.md`, which
increases the transposition-table twin capacity and adds metrics for observing
TT pressure in repetition-heavy positions.

## Changes made

### `src/search/tt.rs`

- Increased `MAX_TWINS` from `2` to `8`.  This gives each `TtEntry` room for
  more path-dependent solved results before evicting the oldest twin.
- Added `TwinAction` to distinguish new twin insertions, updates of an
  existing twin for the same path, and evictions of the oldest twin.
- Added `twin_insertions` and `twin_evictions` counters to `TranspositionTable`,
  updated whenever `store_twin` inserts or evicts a twin.  `clear()` resets
  both counters.
- Exposed `TranspositionTable::twin_stats() -> (u64, u64)` to read the
  `(insertions, evictions)` counters.
- Added unit tests:
  - `tt_entry_size_is_reasonable` — asserts the per-entry size is still bounded
    after raising `MAX_TWINS`.
  - `twin_metrics_track_insertions_and_evictions` — verifies the counters
    correctly track insertions and the first eviction.
  - `clear_resets_twin_stats` — verifies `clear()` resets the counters.

### `tests/test_twin_capacity.rs` (new)

- Added a regression test `two_rooks_mate_with_transpositions` using
  `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1`.  White has two rooks that can be
  developed in either order (`Rf1-f3` + `Rg1-g3` transposes with `Rg1-g3` +
  `Rf1-f3`), so the position is transposition-heavy.  The test asserts the
  solver still finds `Outcome::Win` quickly and with fewer than 10,000 nodes.

## Capacity decision

A fixed-size array was kept rather than moving to a `Vec<TwinEntry>` because:

- `TtEntry` remains `Copy`, keeping `TranspositionTable` initialization simple
  and avoiding per-entry heap allocation.
- With `MAX_TWINS = 8`, the per-entry size stays well under 512 bytes (the
  unit test confirms this).  At the default 64 MB, the table still holds
  hundreds of thousands of entries.
- The metrics show no pathological eviction rates in the regression test,
  so the fixed increase appears sufficient for the positions tested.

If future, more cyclic positions show high eviction counts, the next step
would be to switch to a `Vec<TwinEntry>` or a `smallvec`-style inline storage.
That change would require removing `Copy` from `TtEntry` and updating the
`TranspositionTable` constructor.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
```

All passed:

- `cargo clippy --all-targets` is clean.
- `cargo test` passes all tests, including the new TT unit tests and the
  `two_rooks_mate_with_transpositions` regression test.
- `cargo doc` builds without warnings.

Observed node count for the transposition regression position was 135 nodes,
well below the 10,000-node threshold.

## Remaining concerns

- The twin replacement policy still evicts slot `0` when all slots are full,
  which is a simple FIFO-ish policy rather than true LRU.  This may matter for
  heavily cyclic positions that exceed 8 twins per board state.
- `solve_refined`, `validate_pv`, and the remaining GHI edge cases noted in
  earlier reports are still open.
