# Report: Plan 5 — Cache per-node move scores

This report documents the application of `docs/plans/speed/plan5.md`.

## Changes applied

### `src/search/dfpn/history.rs`

`sort_moves` previously called `self.scorer.score(...)` inside the `slice.sort_by` comparator, so every move was scored `O(N log N)` times per node.  The function now:

1. Builds a `Vec<(Move, i32)>` by scoring each move exactly once, adding history and killer bonuses.
2. Sorts that vector by descending score.
3. If a `best_from_tt` move is present, it is rotated to the front (preserving the original behavior).
4. Writes the reordered moves back into `moves.as_mut_slice()`.

The `StaticAtomicScorer` logic in `src/search/ordering.rs` was not changed.

## Why move ordering stays stable

- The score for each move is computed from the same inputs as before: `scorer.score(...) + history + killer_bonus`.
- `sort_by_key` with `std::cmp::Reverse(score)` produces a descending sort, identical in ordering to the previous `sort_by` comparator.
- `best_from_tt` is still rotated to index 0 after the sort, so the TT-best move gets the same priority.
- The rewritten moves are placed back into the original `MoveList` in the new order.

## Benchmarks

Wall-clock seconds for `cargo run --release -- --fen ...` with the default 5-second timeout.  The "before" numbers are from the Plan 4 build; the "after" numbers are from the same build with cached move scores.

| FEN | Outcome / PV | Before (Plan 4) | After (cached scores) | Change |
|-----|--------------|----------------:|----------------------:|-------:|
| `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` | win, `f1f7 e8d8 g1g8` | warm mean 0.019 | warm mean 0.016 | within noise |
| `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` | win, `h5d5 d7d6 d5f7 e8d7 f7e7` | mean 1.538 | mean 1.477 | ~4% faster |
| `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` | win, `a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6` | mean 0.243 | mean 0.236 | ~3% faster |
| `4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19` (m19) | draw (timeout) | 5.007 | 5.006 | unchanged (timeout-limited) |

The two longer decisive positions both improved by a few percent, which matches the expected saving from eliminating repeated `score` calls.  The two-rook position is too small for the timing difference to be distinguished from noise.

The `test_review` integration suite also ran slightly faster in release (`~1.49 s` vs `~1.56 s` in the Plan 4 run), consistent with the reduced per-node sorting cost.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --release
$ cargo doc --no-deps
$ cargo run --release --example static_move_scores -- "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"
$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
```

Results:

- `cargo fmt` completed with no changes.
- `cargo clippy --all-targets` reports zero warnings.
- `cargo test --release` passes all tests.
- `cargo doc --no-deps` builds cleanly.
- `examples/static_move_scores.rs` still prints the expected scores for the two-rook position.
- The sample FENs produce identical outcomes and PVs.

## Conclusion

Caching per-node move scores removes the `O(N log N)` repeated scoring overhead in `sort_moves`.  The measured speed-up on the longer sample positions is a few percent, with no change to move ordering or solver output.  This is a safe, worthwhile cleanup that is a prerequisite for any further tuning of the static scorer.
