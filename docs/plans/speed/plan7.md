# Plan 7: Stop copying `TtEntry` on every probe

## Start

Read `docs/plans/speed/analysis.md`.  Inspect `src/search/tt/entry.rs`,
`src/search/tt/table.rs`, and the callers in `src/search/dfpn/core.rs` and
`src/search/dfpn/children.rs`.

## Goal

Reduce the memory bandwidth wasted by copying the full `TtEntry` on every
transposition-table lookup.

## Background

`TtEntry` is `Copy` and contains eight `TwinEntry` slots:

<ref_snippet file="/workspace/atomic_solver/src/search/tt/entry.rs" lines="55-54" />

The hot paths copy the whole entry with `.copied()`:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn/core.rs" lines="77" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/children.rs" lines="140" />

A full `TtEntry` is well over 200 bytes, so every probe copies a large block of
memory.  Most callers only need a few fields (`outcome`, `pn`, `dn`, `depth`,
`remaining_depth`, `best_move`, and perhaps the twins for `try_use_tt`).

## Implementation tasks

Choose one of the two approaches below.  Both avoid copying the full entry.

### Option A — work with references where possible

1. Keep `TtEntry: Copy` but remove `.copied()` from the hot callers.
2. Change `try_use_tt` so it takes the 64-bit `key` and the search path context
   instead of `&TtEntry`.  Inside `try_use_tt`, do the lookup once, extract the
   few fields it needs into local variables, drop the reference before calling
   `store_twin`, and then call `store_twin(key, ...)`.
3. In `evaluate_child`, read the base fields (`outcome`, `pn`, `dn`, `depth`,
   `remaining_depth`, `repetition_seen`) directly from the probed reference
   without copying.

### Option B — introduce a `TtSummary` struct (recommended if Option A is awkward)

1. Add a small `TtSummary` struct to `src/search/tt/` containing only the fields
   used by `dfpn`:
   ```rust
   pub struct TtSummary {
       pub best_move: Move,
       pub outcome: Option<Outcome>,
       pub pn: u64,
       pub dn: u64,
       pub depth: u32,
       pub remaining_depth: u32,
       pub repetition_seen: bool,
   }
   ```
2. Add `TranspositionTable::probe_summary(&self, key) -> Option<TtSummary>`.
3. Use `probe_summary` in `evaluate_child` and for the unsolved-entry reads in
   `dfpn`.
4. `try_use_tt` can still iterate `entry.twins` by reference or copy only the
   single `TwinEntry` it intends to simulate.

### Either way

- Avoid `.copied()` for `TtEntry` in `core.rs` and `children.rs`.
- Keep the `TtEntry::Copy` derive if it is still needed for the
  `[TtEntry::default(); N]` initialization, but avoid relying on it in the
  search hot path.

## File changes

- `src/search/tt/entry.rs` (possibly `TtSummary`)
- `src/search/tt/table.rs` (`probe_summary` or reference-return changes)
- `src/search/dfpn/core.rs`
- `src/search/dfpn/children.rs`
- `src/search/dfpn/simulate.rs` (`try_use_tt` signature)

## Risks

- `try_use_tt` mutates the transposition table (`store_twin`), so a reference
  into the table cannot be held across the mutation.  Copy the needed twin data
  to the stack before simulating.
- Removing `TtEntry: Copy` breaks the `vec![[TtEntry::default(); 2]; buckets]`
  initialization because `[T; N]` repetition requires `T: Copy`.  If you remove
  `Copy`, replace the initialization with `Vec::with_capacity` and explicit
  pushing.
- Ensure `probe_summary` does not accidentally copy the whole `TtEntry` while
  extracting the summary.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
```

All outcomes and PVs must be identical.  A cachegrind or `perf` run should show
fewer bytes copied per `tt.probe`.

## Final task

Write `docs/plans/speed/report7.md` documenting which approach was chosen and the
observed reduction in `TtEntry` copies.
