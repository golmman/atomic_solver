# Plan 10: Use a TT generation counter instead of `tt.clear()`

## Start

Read `docs/plans/speed/analysis.md`.  Open `src/search/dfpn/mod.rs` and count
how many times `tt.clear()` is called during `solve`, `solve_refined` and
`solve_refined_unbounded`.

## Goal

Stop zeroing the transposition table between iterative-deepening probes.

## Background

The bootstrap and refinement loops call `tt.clear()` repeatedly:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn/mod.rs" lines="125-138" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/mod.rs" lines="214-281" />

Each `clear()` walks the whole table and zeroes it, throwing away entries that
were computed only moments earlier.  A generation counter makes old entries
logically stale without writing every bucket, so work from previous probes can
be reused.

## Implementation tasks

1. Add a `generation: u32` field to `TtEntry`.
2. Add a `current_generation: u32` field to `TranspositionTable`.
3. Initialize all default entries with `generation: 0` and set the table's
   `current_generation` to `1` on construction.
4. Update `TranspositionTable::probe` to only match entries where
   `entry.generation == self.current_generation` in addition to `entry.valid &&
   entry.key == key`.
5. Update `TranspositionTable::store` and `store_twin` to set
   `entry.generation = self.current_generation`.
6. Add `TranspositionTable::new_generation(&mut self)` that increments
   `current_generation` (and, on the rare `u32` wrap, fall back to physically
   clearing the table).
7. Replace the `tt.clear()` calls inside the iterative-deepening/refinement loops
   with `tt.new_generation()`.  Keep `tt.clear()` available for callers that
   genuinely want a fresh table.
8. (Optional) When choosing a bucket slot to overwrite, prefer a slot whose
   `generation` is older than `current_generation`; this naturally recycles
   stale memory instead of evicting live entries.

## File changes

- `src/search/tt/entry.rs`
- `src/search/tt/table.rs`
- `src/search/dfpn/mod.rs`

## Risks

- If `remaining_depth` semantics are not respected across generations, a stale
  entry with `remaining_depth < current_max_depth` could be treated as a hit.
  The generation counter only invalidates the whole previous generation; the
  existing `remaining_depth` check must still be performed inside `try_use_tt`.
- Adding a `u32` to `TtEntry` grows each entry by 4 bytes (plus alignment).  For
  large tables this is acceptable but must be accounted for.
- `u32` overflow is practically impossible, but a fallback clear on wrap is
  required for correctness.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
```

Run `examples/twin_stats.rs` or a short script that exercises `solve_refined` and
confirm that `twin_insertions` and `twin_evictions` behave as before while node
counts drop on multi-probe solves.

## Final task

Write `docs/plans/speed/report10.md` showing node counts before and after the
change on positions that use iterative deepening.
