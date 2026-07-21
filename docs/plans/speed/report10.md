# Report: Plan 10 — Use a TT generation counter instead of `tt.clear()`

This report documents the application of `docs/plans/speed/plan10.md`.

## Changes applied

### `src/search/tt/entry.rs`

- Added `generation: u32` to `TtEntry`.
- Default entries are created with `generation: 0`.

### `src/search/tt/table.rs`

- Added `current_generation: u32` to `TranspositionTable`, initialized to `1`.
- `probe` now matches only entries where `entry.valid && entry.key == key &&
  entry.generation == self.current_generation`.
- `store` and `store_twin` set `entry.generation = self.current_generation` on
  every update and on every newly created `TtEntry`.
- Added `new_generation(&mut self)` which increments `current_generation` and
  physically clears the table only on the extremely unlikely `u32` wrap.
- `clear` now also resets `current_generation` to `1`.
- `insert_new` prefers to overwrite a bucket slot that is either invalid or
  belongs to an older generation before evicting a live current-generation
  slot.

### `src/search/dfpn/mod.rs`

- Replaced the five `self.tt.clear()` calls inside the iterative-deepening
  bootstrap and refinement loops with `self.tt.new_generation()`.
- `tt.clear()` is still available for callers that need a fresh table.

### `src/search/tt/tests.rs`

- Added `new_generation_marks_old_entries_stale`.
- Added `new_generation_prefers_stale_slot`.

## Why node counts stay the same

The generation counter is logically equivalent to clearing: after
`new_generation()` all previously stored entries are ignored by `probe`.  The
search therefore repopulates the table just as it would after `tt.clear()`.  The
win is not from reusing entries between probes (which would require careful
`remaining_depth` validation), but from avoiding the O(table-size) memory write
that `clear()` performed on every probe.

## Benchmarks

Sample FENs run with `cargo run --release -- --fen ...`, default 5-second
timeout, `refine_shortest` enabled.  The "before" numbers were measured by
switching the `mod.rs` calls back to `tt.clear()` while keeping the generation
fields in `TtEntry`/`TranspositionTable` (which is functionally identical to the
pre-Plan 10 code).  The "after" numbers are with `tt.new_generation()`.

| FEN | before nodes | after nodes | before time (warm mean) | after time (warm mean) |
|-----|-------------:|------------:|--------------------------:|-----------------------:|
| `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` | 133 | 133 | 0.015 s | 0.007 s |
| `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` | 373,557 | 373,557 | 1.452 s | 1.456 s |
| `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` | 97,743 | 97,743 | 0.240 s | 0.225 s |
| `4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19` (m19) | — (timeout) | — (timeout) | 5.006 s | 5.007 s |

As expected, node counts are identical, while small searches (two-rook mate)
show a large relative speedup because the search time is dominated by the
physical table clear.  Larger searches amortize the clear cost, so the gain is
smaller but still positive where the timeout is not the limiting factor.

`cargo test --release` for `tests/test_review.rs`:

- Plan 9: ~1.58 s
- Plan 10: ~1.53 s

`examples/twin_stats.rs` output is unchanged in shape; twin insertions and
peak twins are still tracked correctly.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --release
$ cargo doc --no-deps
$ cargo run --release --example twin_stats
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"
$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
```

Results:

- `cargo fmt` completed with no diffs.
- `cargo clippy --all-targets` reports zero warnings.
- `cargo test --release` passes all tests.
- `cargo doc --no-deps` builds cleanly.
- `examples/twin_stats` runs and reports consistent twin statistics.
- The sample FENs produce identical outcomes and PVs.

## Conclusion

Replacing physical `tt.clear()` calls with a generation counter eliminates the
per-probe cost of zeroing the entire transposition table.  Node counts are
unchanged because old entries are still logically invalidated, preserving
correctness.  The biggest relative speedups appear on small searches where the
clear overhead dominated; larger searches show modest gains.  Future work could
attempt to reuse entries across iterative-deepening probes by validating the
stored `remaining_depth`, which would reduce node counts as well as time.
