# Plan 4: Increase transposition-table twin capacity

## Start

- Read `docs/plans/review/report3.md` to confirm GHI simulation is solid and to
  note any blockers before changing twin storage.

## Goal

Reduce repeated re-solving in repetition-heavy positions by increasing the
number of path-dependent twins kept per transposition-table entry.

## Background

`MAX_TWINS = 2` in `src/search/tt.rs`. When many paths reach the same board,
old twins are evicted. Eviction is safe (the base entry falls back to `(1, 1)`),
but it causes repeated work and interacts badly with simulation reuse.

## Implementation tasks

1. Increase `MAX_TWINS` to a larger fixed value (e.g. 8 or 16) and measure TT
   pressure on a few cyclic positions.
2. If fixed capacity still shows high eviction, replace the
   `[TwinEntry; MAX_TWINS]` array with a `Vec<TwinEntry>` or `smallvec`-style
   inline storage. Note that this removes `Copy` from `TtEntry`; update
   `TranspositionTable` initialization accordingly.
3. Add a counter/metric (behind `#[cfg(test)]` or a debug field) that records
   the number of twin insertions and evictions. Use it to decide whether the
   chosen capacity is sufficient.
4. Add a regression test that repeatedly reaches the same position via different
   move orders and verify the solver still returns the correct outcome without a
   large node blow-up.
5. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
6. Final task: write `docs/plans/review/report4.md` summarizing the capacity
   change, benchmark/metric results, and whether a dynamic structure was adopted.

## File changes

- `src/search/tt.rs`
- `src/search/dfpn.rs` (metrics, if added)

## Risks

- Larger fixed arrays increase TT memory per entry. With the default 64 MB,
  each extra twin adds a small fraction; confirm with
  `std::mem::size_of::<TtEntry>()` before and after.
- Dynamic allocation in the TT can hurt performance; prefer fixed-size increase
  unless metrics justify it.

## Verification

- Memory size check.
- Regression test on a cyclic position passes and node count is stable or
  improved.
