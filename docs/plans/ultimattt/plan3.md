# Plan 3: Best-child stability and work-based TT replacement

## Goal

Reduce DF-PN sibling thrashing and keep the most valuable subtrees in the transposition table by adding a `best_child` field and a `work` counter to each TT entry.

## Background

`ultimattt` stores two extra fields in every transposition-table entry:

- `child: u8` — the index of the child that was previously selected as most-proving. `select_child` first checks whether this child still satisfies the new threshold, and if so reuses it instead of recomputing the argmin.
- `work` — the amount of search effort spent under this entry. The replacement policy keeps solved entries and high-work entries, evicting cheap, recently-seen leaves.

`atomic_solver` currently recomputes `best_and_second_unsolved` from scratch on every `dfpn` iteration and uses a simple 2-slot bucket replacement based only on recency/generation.

## Files to modify

- `src/search/tt/entry.rs`
- `src/search/tt/table.rs`
- `src/search/dfpn/selection.rs`
- `src/search/dfpn/core.rs`
- `src/search/dfpn/children.rs` (for `ChildSelection` plumbing)

## Concrete changes

1. Extend `TtEntry` with:
   ```rust
   pub(crate) best_child: u8,   // u8::MAX means "unknown / unset"
   pub(crate) work: u64,        // cumulative child_evals spent under this subtree
   ```
   Update `Default` and `TtSummary` accordingly.
2. Update `Search::select_from_children` to:
   - Accept the previous `best_child` value (or read it from the TT entry).
   - If the stored child is still valid and still has the best `pn` (OR) or `dn` (AND) among unsolved children, return it as the most-proving child without scanning for a second-best unless a second-best is actually needed.
   - Otherwise recompute and store the new index.
3. Update `Search::tt.store` calls in `core.rs` to record:
   - the selected `best_child` index converted to a move or index,
   - `work` incremented by the search effort spent in the subtree (use `self.child_evals` or a per-subtree counter).
4. Update `TranspositionTable::insert_new` to prefer:
   - solved over unsolved,
   - then higher `work` over lower `work`,
   - then newer generation.

## Verification

- Run `cargo test` and `cargo test --release`.
- Run `examples/benchmark.rs` and compare node counts and wall-clock time.
- Verify `fen1` unbounded solve remains fast.
- Check that `TtEntry` size increase does not blow the 64 MB transposition table budget unreasonably (adding 9 bytes per entry is negligible for a 64 MB table).

## Risks / notes

- `best_child` is an index, not a move, so it is only meaningful if the move list order is deterministic. `sort_moves` is deterministic given the same history/killer/TT state, but the move list order may change as history tables age. To be safe, store the best child's move instead of its index, or recompute the index from the move on each lookup.
- `work` must be monotonically non-decreasing in a single entry; the replacement policy relies on this.
- These two fields modify the same TT layout, so they should be implemented together to avoid two entry-format changes.
