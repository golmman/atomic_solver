# Report: Plan 7 — Stop copying `TtEntry` on every probe

This report documents the application of `docs/plans/speed/plan7.md`.

## Approach chosen

Option B was followed: add a small `TtSummary` struct and a `probe_best_move` helper so the search hot paths only copy the handful of base fields they need, not the full `TtEntry` (which includes eight `TwinEntry` slots).

## Changes applied

### `src/search/tt/entry.rs`

- Added `TtSummary`, a `Copy` struct containing only the base entry fields used by the DF-PN search: `best_move`, `outcome`, `pn`, `dn`, `depth`, `remaining_depth`, and `repetition_seen`.

### `src/search/tt/table.rs`

- Added `TranspositionTable::probe_summary(&self, key) -> Option<TtSummary>` to return the small base-field summary.
- Added `TranspositionTable::probe_best_move(&self, key, path_code) -> Option<Move>` to return the best move for a key/path without copying the twins.  It mirrors the old `TtEntry::best_result_for_path` logic, falling back to the unsolved base `best_move`.

### `src/search/tt/mod.rs`

- Re-exported `MAX_TWINS` so that `simulate.rs` can iterate over twin slots without a magic number.

### `src/search/dfpn/simulate.rs`

- `try_use_tt` now takes the 64-bit `key` instead of an owned/copied `&TtEntry`.
- It probes the table once for the path-independent base result and once per twin slot when simulating.  Each probe is a short-lived immutable borrow, so the later mutable `store_twin` call is not blocked.
- Removed `.copied()` from the two recursive `simulate` probes; they now use `self.tt.probe(...).and_then(|e| e.find_result_for_path(...))`.

### `src/search/dfpn/core.rs`

- Replaced `let tt_entry = self.tt.probe(tt_key).copied();` with a direct call to `self.try_use_tt(pos, tt_key, ...)`.
- Replaced the manual `best_result_for_path` extraction from a copied `TtEntry` with `self.tt.probe_best_move(tt_key, self.path_code)`.

### `src/search/dfpn/children.rs`

- `evaluate_child` no longer copies `TtEntry`.
- It first calls `try_use_tt(pos, child_key, ...)` and then falls back to `probe_summary(child_key)` for the unsolved base bounds.

### `src/search/dfpn/tests.rs`

- Updated `try_use_tt` call sites to pass `key` instead of `&entry`.

## Why `TtEntry: Copy` is kept

The transposition table is still initialised with `vec![[TtEntry::default(); 2]; buckets]`, which requires `TtEntry` to be `Copy` for the array-repeat expression.  The derive was left in place; only the *use* of that copy in the hot probe paths was removed.

## Benchmarks

Wall-clock seconds for `cargo run --release -- --fen ...` with the default 5-second timeout.  The "before" numbers are from the Plan 6 build; the "after" numbers are from the same build with the `TtEntry` copy eliminated.

| FEN | Outcome / PV | Before (Plan 6) | After (no full entry copy) | Change |
|-----|--------------|----------------:|---------------------------:|-------:|
| `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` | win, `f1f7 e8d8 g1g8` | warm mean 0.016 | warm mean 0.014 | within noise |
| `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` | win, `h5d5 d7d6 d5f7 e8d7 f7e7` | mean 1.460 | mean 1.443 | ~1% faster |
| `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` | win, `a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6` | mean 0.240 | mean 0.218 | ~9% faster |
| `4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19` (m19) | draw (timeout) | 5.007 | 5.006 | unchanged (timeout-limited) |

The biggest improvement is on the promotion-transposition position, which exercises the TT heavily and now sees roughly 9% less wall-clock time.  The two smaller positions are too fast for the difference to be reliably distinguished from noise.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --release
$ cargo doc --no-deps
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"
$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
```

Results:

- `cargo fmt` completed with no remaining diffs.
- `cargo clippy --all-targets` reports zero warnings.
- `cargo test --release` passes all tests.
- `cargo doc --no-deps` builds cleanly.
- The sample FENs produce identical outcomes and PVs.

## Conclusion

The hot paths no longer copy the full `TtEntry` on every probe.  A `TtSummary` carries the base fields needed by `evaluate_child`, and `probe_best_move` returns just the best move for `sort_moves` ordering.  The simulation path copies at most one `TwinEntry` at a time.  The measured speed-up is largest on TT-heavy positions, and all outcomes / PVs remain unchanged.
