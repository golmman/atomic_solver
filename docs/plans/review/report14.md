# Plan 14 Implementation Report

This report documents the evaluation of the twin replacement policy requested by
`docs/plans/review/plan14.md`.

## Instrumentation added

### `src/search/tt.rs`

- Added `TtEntry::live_twin_count()` to count occupied twin slots in an entry.
- Added `peak_twins: u8` to `TranspositionTable`, tracking the maximum number
  of live twins observed in any single entry.
- Exposed `TranspositionTable::peak_twins()` and `Search::peak_twins()` for
  instrumentation and unit testing.
- Updated `TranspositionTable::store` and `store_twin` to refresh
  `peak_twins` after a twin is inserted.
- Updated `TranspositionTable::clear` to reset `peak_twins`.

### `examples/twin_stats.rs` (new)

A small diagnostic example that solves GHI-sensitive positions and prints:
- outcome and node count
- twin insertions and evictions
- peak live twins in any single entry

## Measurement method

Ran the diagnostic example on the regression positions from Plan 13:

```text
$ cargo run --example twin_stats --release
```

Positions tested:
- Promotion transposition start: `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1`
- Promotion transposed board: `QQ2k3/8/8/8/8/8/8/4K3 b - - 0 1`
- Cyclic rook-safe-area position: `8/8/8/8/2k5/8/8/4KR2 w - - 0 1`
- Same cyclic position after a reversible rook/king shuffle.

## Results

```text
promotion start:
  outcome: Win, nodes: 607
  twin insertions: 0, evictions: 0
  peak live twins in one entry: 0
promotion transpose:
  outcome: Loss, nodes: 22
  twin insertions: 0, evictions: 0
  peak live twins in one entry: 0
cyclic rook safe:
  outcome: Draw, nodes: 1073931
  twin insertions: 83, evictions: 0
  peak live twins in one entry: 2
cyclic rook safe (after 4 reversible moves):
  outcome: Draw, nodes: 1070559
  twin insertions: 81, evictions: 0
  peak live twins in one entry: 2
```

## Decision

The fixed eight-slot FIFO twin array is sufficient for the current test suite:
- No twin evictions were observed on any of the GHI-sensitive positions.
- The maximum number of live twins in any one entry was only 2, far below the
  8-slot capacity.
- Promotion transpositions were solved path-independently, producing no twins
  at all.

Therefore, the FIFO replacement policy is retained. No dynamic storage, ring
buffer, or LRU change was made.

## Correctness invariants preserved

The existing invariants remain unchanged:
- Path-independent base results (`!repetition_seen`) override twins.
- `find_result_for_path` checks an exact path-code match before returning.
- `best_result_for_path` returns the stored twin for the current path code.
- Eviction only reduces reuse; it never returns an incorrect result.

## Unit tests

- `peak_twins_tracked`: verifies `peak_twins` is updated when twins are stored
  across multiple entries and reaches the capacity of 8.
- `clear_resets_twin_stats` updated to assert `peak_twins` is reset by `clear`.
- The existing `twin_metrics_track_insertions_and_evictions` test continues to
  cover FIFO fill and eviction behaviour.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
$ cargo test --release
$ cargo run --example twin_stats --release
```

All commands completed successfully.

## Notes and future work

- `peak_twins` is exposed as a public method because it is a low-overhead
  diagnostic metric that may be useful for future TT tuning. If desired, it can
  be hidden behind `#[cfg(test)]` later.
- If future positions are found that fill all 8 twin slots and cause repeated
  evictions, an LRU replacement within the fixed array could be implemented
  without changing `TtEntry` layout by moving the most-recently accessed twin to
  the end of the array and evicting from the front. This was not needed now.
