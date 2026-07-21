# Report: Plan 6 — Replace `HashSet<u64>` repetition path with a stack + linear search

This report documents the application of `docs/plans/speed/plan6.md`.

## Changes applied

### `src/search/dfpn/mod.rs`

- Removed the `path: HashSet<u64>` field from `Search`; `path_stack: Vec<u64>` is now the single authoritative path.
- Removed the `HashSet` import and all `path` initialisation/clearing code.
- Added small path helper methods on `Search`:
  ```rust
  pub(super) fn path_contains(&self, key: u64) -> bool {
      self.path_stack.contains(&key)
  }
  pub(super) fn path_push(&mut self, key: u64) {
      self.path_stack.push(key);
  }
  pub(super) fn path_pop(&mut self) {
      self.path_stack.pop();
  }
  ```

### `src/search/dfpn/core.rs`

- Replaced `if !self.path.insert(rep_key) { return Outcome::Draw; }` with `if self.path_contains(rep_key) { return Outcome::Draw; }`.
- The `path_push(rep_key)` call is kept **after** `sort_moves` so that `sort_moves` still sees `path_stack.len()` equal to the current depth (preserving the original killer-depth semantics).
- Replaced the paired `self.path_stack.pop(); self.path.remove(&rep_key);` cleanup with a single `self.path_pop();`.

### `src/search/dfpn/children.rs`

- Replaced `self.path.contains(&child_rep_key)` with `self.path_stack.contains(&child_rep_key)`.  `evaluate_child` is called after the current node has been pushed, so the stack already contains the parent and ancestors.

### `src/search/dfpn/simulate.rs`

- Removed the `sim_path: &mut HashSet<u64>` parameter from `simulate`; the simulation now uses a single `sim_stack: &mut Vec<u64>` for both membership and depth.
- `try_use_tt` now clones only `self.path_stack` into `sim_stack` (no HashSet clone).
- The simulation repetition check uses `sim_stack.contains(&rep_key)` and `sim_stack.push/pop`.

### `src/search/dfpn/tests.rs`

- Updated unit tests to push onto `search.path_stack` instead of `search.path`.
- Removed `sim_path` from `simulate` call sites.

## Why the search depth is preserved

The original code had an asymmetry:

- `path: HashSet<u64>` was used for membership and was updated **before** `sort_moves`.
- `path_stack: Vec<u64>` was used for `path_stack.len()` (killer depth) and was updated **after** `sort_moves`.

To keep `sort_moves` seeing the same depth value, the new code checks `path_contains` before `sort_moves` but only calls `path_push` afterwards.  This means the stack does not contain the current node while moves are being sorted, matching the original `path_stack.len()` value.

## Benchmarks

Wall-clock seconds for `cargo run --release -- --fen ...` with the default 5-second timeout.  The "before" numbers are from the Plan 5 build; the "after" numbers are from the same build with the stack-only path.

| FEN | Outcome / PV | Before (Plan 5) | After (stack-only path) | Change |
|-----|--------------|----------------:|------------------------:|-------:|
| `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` | win, `f1f7 e8d8 g1g8` | warm mean 0.016 | warm mean 0.016 | within noise |
| `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` | win, `h5d5 d7d6 d5f7 e8d7 f7e7` | mean 1.477 | mean 1.460 | ~1% faster |
| `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` | win, `a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6` | mean 0.236 | mean 0.240 | within noise |
| `4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19` (m19) | draw (timeout) | 5.006 | 5.007 | unchanged (timeout-limited) |

The speed-up on the longer decisive positions is small (around 1%) and within run-to-run variation on the shorter ones.  The main win is the removal of the separate `HashSet` allocation and hashing on every make/unmake, which is now measured as a small constant improvement on the larger positions.

## Deep-path / cyclic behaviour

The two ignored cyclic GHI regression tests were run explicitly to ensure the `Vec`-based linear search is still safe on repetition-heavy positions:

```text
$ cargo test --release cyclic_rook_position_does_not_claim_win -- --ignored
$ cargo test --release reversible_cycle_does_not_claim_win -- --ignored
```

Both passed (they run to the 5-second timeout and correctly do **not** claim a win).  The standard `test_repetition` suite and the `test_ghi` non-ignored transposition test also pass.  This confirms that the `O(depth)` linear search is competitive for the path lengths encountered in the test suite.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --release
$ cargo test --release cyclic_rook_position_does_not_claim_win -- --ignored
$ cargo test --release reversible_cycle_does_not_claim_win -- --ignored
$ cargo doc --no-deps
$ cargo run --release --example play_and_solve
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"
$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
```

Results:

- `cargo fmt` completed with no changes.
- `cargo clippy --all-targets` reports zero warnings.
- `cargo test --release` passes all tests.
- The two ignored cyclic GHI regression tests pass.
- `cargo doc --no-deps` builds cleanly.
- `examples/play_and_solve` runs and reports `outcome: Draw` for the default m19 move within the 5-second timeout.
- The sample FENs produce identical outcomes and PVs.

## Conclusion

The redundant `HashSet` path set was removed in favour of the existing `Vec<u64>` path stack.  This simplifies the code, removes per-node HashSet insert/remove overhead, and reduces allocations (especially in simulation, where a `HashSet` clone was removed).  The measured wall-clock improvement is modest, and all repetition/GHI tests continue to pass, including the cyclic regression positions.  If future positions produce much deeper paths, an `FxHashSet`-backed `BuildHasherDefault` could be reintroduced as a drop-in replacement for `path_stack`, but the `Vec` implementation is sufficient for the current workload.
