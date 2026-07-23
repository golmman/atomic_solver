# Report: Plan 3 — Best-child stability and work-based TT replacement

This report documents the implementation of `docs/plans/ultimattt/plan3.md`.

## Summary

Extended every transposition-table entry with two new fields:

- `best_child: u8` — a hint to the index of the most-proving child from the
  previous search.  `u8::MAX` means "unknown / unset".
- `work: u64` — a lower bound on the number of `child_evals` spent in the
  subtree rooted at this entry.

`select_from_children` now reuses the stored child when it is still valid and
still has the best `pn` (OR nodes) or `dn` (AND nodes) among the unsolved
children, avoiding a full `best_and_second_unsolved` scan.  The TT replacement
policy now prefers solved entries over unsolved ones, higher-work entries over
lower-work ones, and newer generations over older ones.

The `work` field is stored monotonically (`slot.work = slot.work.max(work)`),
and the `best_child` hint is validated against the stored `best_move` (re-
computing the index from the move if the move order has changed), so move-order
changes from aging history tables do not silently corrupt the hint.

## Changes applied

### `src/search/tt/entry.rs`

- Added `best_child: u8` and `work: u64` to `TtEntry`.
- Updated `TtEntry::default()` with `best_child = u8::MAX` and `work = 0`.
- Updated `TtSummary` to include `best_child` and `work`.
- `reinit_base_for_twin()` clears `best_child` but preserves `work`; a path-
  dependent twin entry still represents effort spent in the subtree.

### `src/search/tt/table.rs`

- `TranspositionTable::store` now takes `best_child` and `work` arguments.
  Existing slots update `best_child`, and `work` is taken as the maximum of
  the old and new values to keep it monotonically non-decreasing.
- `TranspositionTable::store_twin` also accepts a `work` value; existing base
  slots have their `work` maxed with the new value, and newly-created twin
  base slots carry the supplied `work`.
- `TranspositionTable::insert_new` was rewritten to implement the new
  replacement policy:
  - empty or stale slots are filled first;
  - if both slots are live in the current generation, the lower-priority
    existing slot is evicted and the new entry is always stored;
  - priority is `(live, solved, work, generation)` (all higher-is-better).

  An initial implementation sorted `[old0, old1, new]` and discarded the lowest-
  scoring entry.  That proved unsafe: a freshly-solved child entry could be
  discarded if two unrelated high-work entries already occupied its bucket, so
  the parent never saw the child's result and could loop.  Always storing the
  new entry fixes this while still preferring to evict cheap/unsolved/stale
  entries.

### `src/search/dfpn/children.rs`

- `select_from_children` now takes `previous_best_move` and
  `previous_best_child` hints from the TT summary.
- If the stored child is still valid and still the most-proving unsolved child,
  `selection_for_child` is used to return the result directly; it computes the
  parent `pn`/`dn` and the second-best unsolved child without a full argmin
  scan.
- Added a private `second_best_unsolved_excluding` helper.

### `src/search/dfpn/core.rs`

- Each `dfpn` call now records `child_evals_start` and stores
  `work = self.child_evals - child_evals_start` in the final `tt.store` call.
- `tt.store` also records `best_child`, computed by finding the index of
  `store_best_move` in the `children` vector.
- The previous `best_child`/`best_move` hints are read from `probe_summary`
  before the loop and passed into `select_from_children`.
- Terminal and depth-cutoff `tt.store` calls use `best_child = u8::MAX` and
  `work = 0`.

### Call site updates

- `src/search/dfpn/pv.rs`, `src/search/dfpn/simulate.rs`, and the TT/DF-PN
  test files were updated for the new `store` / `store_twin` signatures.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo test --release
$ cargo doc --no-deps
```

Results:

- `cargo fmt` completed with no changes after the final pass.
- `cargo clippy --all-targets` reported zero warnings.
- `cargo test` and `cargo test --release` passed all tests.
- `cargo doc --no-deps` built cleanly.

### `TtEntry` size

```text
$ cargo run --release --example tt_size
TtEntry size: 248
```

(`examples/tt_size.rs` was removed after the measurement.)  This is well under
the 512-byte cap tested by `search::tt::tests::tt_entry_size_is_reasonable` and
negligible relative to the default 64 MB table budget.

### Benchmark

```text
$ cargo run --release --example benchmark
```

```text
runs=10 timeout=5s epsilon=0.125 refine_shortest=false

| name | outcome | nodes | child_evals | mean (s) | min (s) | max (s) | pv_len |
|------|---------|------:|------------:|---------:|--------:|--------:|-------:|
| two_rook_mate | win | 6 | 35 | 0.000 | 0.000 | 0.000 | 3 |
| epsilon_mate | win | 533 | 11582 | 0.004 | 0.003 | 0.004 | 5 |
| promotion_transposition | win | 819 | 6601 | 0.002 | 0.001 | 0.002 | 15 |
| m26 | win | 299 | 2461 | 0.001 | 0.001 | 0.001 | 11 |
| opening_f2 | win | 658 | 13675 | 0.004 | 0.004 | 0.005 | 7 |
| rook_pawn_endgame | win | 714 | 5268 | 0.002 | 0.001 | 0.002 | 9 |
| m19 | draw | 791855 | 16407138 | 5.000 | 5.000 | 5.000 | 0 |
| startpos | draw | 651698 | 15873499 | 5.000 | 5.000 | 5.000 | 0 |
```

The six decisive positions sum to 3,029 nodes and 39,622 `child_evals`, which
is identical to the Plan 2 baseline for the same `epsilon = 0.125` settings.
This shows that the new fields and replacement policy do not introduce a
regression on the existing benchmark suite; the intended stability benefits are
most likely to appear on deeper, TT-bound searches where the same best child
persists across iterations.

### `fen1` and `fen2` regression checks

`fen2`:

```text
$ cargo run --release -- --fen \
    '6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26' \
    --no-refine-shortest --timeout 60
outcome: win
pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8
```

`fen1`:

```text
$ cargo run --release --example solve_no_refinement -- \
    '6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25'
outcome: Loss nodes: 1157
g8g7
b1b8
g7h7
b8h8
h7g7
h8h7
g7g8
h7g7
g8h8
g7g8
h8h7
g8g6
```

Both return the expected outcomes and PVs.  `fen1` with unbounded search
remains fast (1,157 nodes), while `fen1` under `solve_outcome` still times out
because of the `max_depth` bootstrap cliff — that is the target of Plan 4.

## Files changed

- `src/search/tt/entry.rs`
- `src/search/tt/table.rs`
- `src/search/dfpn/children.rs`
- `src/search/dfpn/core.rs`
- `src/search/dfpn/pv.rs`
- `src/search/dfpn/simulate.rs`
- `src/search/tt/tests.rs`
- `src/search/dfpn/tests.rs`
- `docs/plans/ultimattt/report3.md` (this report)
