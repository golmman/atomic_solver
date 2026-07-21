# Report: Plan 9 — Incremental Zobrist hash update in `Position`

This report documents the application of `docs/plans/speed/plan9.md`.

## Investigation

`atomic_movegen::board::Board` already maintains its Zobrist hash incrementally.
In `atomic-movegen-2.0.0/src/board.rs`:

- `Board::recompute_hash` builds the hash from the piece array, side to move,
  castling rights and en-passant square.
- `Board::from_fen` calls `recompute_hash` once at construction time.
- `Board::do_move`, `move_piece`, `remove_piece`, `place_piece` and
  `update_hash_state_transition` all XOR the incremental `self.hash` field.
- `Board::hash()` simply returns `self.hash`.

Therefore `Position::do_move` / `undo_move` were never recomputing a full
piece-level hash; they were only recomputing the rule50 component each call.

## Changes applied

### `src/zobrist.rs`

- Replaced the runtime `OnceLock<Zobrist>` table with a `const` array
  `RULE50_KEYS: [u64; 101]` generated at compile time.
- Added `rule50_key(rule50: u16) -> u64` so callers can retrieve a key without
  going through the full `hash` wrapper.
- Turned `path_random` into a free function using the existing `splitmix64`
  round.
- Removed the `Zobrist` struct and the `std::sync::OnceLock` dependency.

The generated keys are identical to the previous runtime-generated sequence:
both use the same `SplitMix64` seed (`0x9e37_79b9_7f4a_7c15`) and mixing
function.

### `src/position.rs`

- Updated `do_move` and `undo_move` to compute `self.zobrist` as
  `self.board.hash() ^ zobrist::rule50_key(self.board.rule50())`, reflecting
  that only the rule50 component changes after `Board` has updated its own
  incremental hash.

### `tests/test_position.rs`

- Added `incremental_hash_matches_full_hash_in_random_game`, which plays a
  100-move deterministic random game from the starting position and asserts
  that `Position::hash()` equals `zobrist::hash(&pos.board, pos.board.rule50())`
  after every `do_move` and every `undo_move`.

## Benchmarks

Wall-clock seconds for `cargo run --release -- --fen ...` with the default
5-second timeout.

| FEN | Outcome / PV | Before (Plan 8) | After (Plan 9) |
|-----|--------------|----------------:|---------------:|
| `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` | win, `f1f7 e8d8 g1g8` | warm mean 0.016 | warm mean 0.015 |
| `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` | win, `h5d5 d7d6 d5f7 e8d7 f7e7` | mean 1.452 | mean 1.471 |
| `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` | win, `a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6` | mean 0.224 | mean 0.223 |
| `4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19` (m19) | draw (timeout) | 5.007 | 5.006 |

The wall-clock differences are within run-to-run noise.  The optimization removes
a small `OnceLock` lookup and a function call on every `do_move` / `undo_move`,
but `Board::hash()` was already incremental, so the per-node work was already
tiny.  The main value is cleaner code and the guarantee that `Position::hash`
now uses a compile-time constant rule50 key table.

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

- `cargo fmt` completed with no diffs.
- `cargo clippy --all-targets` reports zero warnings.
- `cargo test --release` passes all tests.
- `cargo doc --no-deps` builds cleanly.
- The sample FENs produce identical outcomes and PVs.
- The new `incremental_hash_matches_full_hash_in_random_game` test passes.

## Conclusion

`Board::hash()` was already incremental.  The remaining per-node overhead was
the rule50 key lookup, which is now a direct array access on a compile-time
constant table.  The measured speed impact is within noise, but the hash path is
slightly simpler and no longer depends on `OnceLock`.
