# Plan 14: Reconsider twin replacement policy

## Start

- Read `docs/plans/review/report13.md` to confirm the GHI regression suite is
  stable and the solver still passes all tests.

## Goal

Evaluate whether the fixed eight-slot FIFO twin array is sufficient for cyclic
positions and, if necessary, replace it with a dynamic or LRU strategy.

## Background

- `TtEntry` currently holds a fixed `[TwinEntry; 8]` array and evicts slot 0
  when all slots are full.
- Heavy cyclic positions can generate many twins, causing repeated evictions
  and re-solving.
- `TranspositionTable` already tracks `twin_insertions` and `twin_evictions`.

## Implementation tasks

1. Add temporary instrumentation or logging in `src/search/tt.rs` to record the
   maximum number of live twins per entry and the eviction rate when running
   the GHI regression tests from plan 13.
2. Run the regression suite (especially the cyclic/repetition tests) and note
   `twin_stats()` values. If evictions are rare and solver performance is
   acceptable, document the result and stop.
3. If evictions are frequent or performance is poor, replace the fixed array
   with one of the following:
   - **Option A:** keep an inline fixed-size ring buffer but use an LRU
     replacement policy instead of FIFO.
   - **Option B:** change `TtEntry` to hold `Vec<TwinEntry>` with a per-entry
     capacity hint.
   - **Option C:** store twins in a separate arena/hash map keyed by
     `(board_key, path_code)` and keep only a pointer/index in `TtEntry`.
4. If `TtEntry` is changed from `Copy`, update `src/search/tt.rs` and
   `src/search/dfpn.rs` to use `.cloned()` instead of `.copied()` where
   necessary, and ensure the bucket array initialization still compiles.
5. Preserve correctness invariants:
   - A base result (`!repetition_seen`) overrides twins and can be returned for
     any path.
   - `find_result_for_path` prefers an exact path-code match before simulating a
     twin from another path.
   - Eviction never returns a wrong result; it only reduces reuse.
6. Add unit tests for the chosen replacement policy:
   - Fill the twin capacity and verify the least-recently-used (or oldest, for
     FIFO) twin is evicted.
   - Verify that an exact-path twin is not evicted incorrectly.
7. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
8. Final task: write `docs/plans/review/report14.md` documenting the chosen
   replacement policy, benchmark/eviction data, and any memory or performance
   trade-offs.

## File changes

- `src/search/tt.rs`
- `src/search/dfpn.rs` (only if `TtEntry` ceases to be `Copy`)

## Risks

- Dynamic storage (`Vec`) adds per-entry allocation overhead and breaks `Copy`.
  Use it only if profiling shows the fixed array is a bottleneck.
- LRU tracking requires an access counter or timestamp; keep it small (e.g.
  `u8` or `u16`) to avoid inflating `TtEntry`.
- Changing `TtEntry` layout may affect transposition table memory usage;
  benchmark TT size before and after.

## Verification

- `cargo test` passes.
- The GHI regression suite from plan 13 does not show a regression in solving
  time.
- Eviction counts (if instrumented) are stable or reduced compared to the FIFO
  baseline.
