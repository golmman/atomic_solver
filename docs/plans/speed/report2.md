# Report: Plan 2 — Remove trait-object dispatch for move scoring

This report documents the application of `docs/plans/speed/plan2.md`.

## Changes applied

### `src/search/dfpn/mod.rs`

- Replaced the boxed trait object in `Search` with a concrete scorer:
  ```rust
  // before
  scorer: Box<dyn MoveScorer>,
  // after
  scorer: StaticAtomicScorer,
  ```
- Updated `Search::new` to construct `StaticAtomicScorer` directly instead of `Box::new(StaticAtomicScorer)`.
- Removed the now-unused `MoveScorer` import from `mod.rs`; the trait is still public in `src/search/ordering.rs` and is still imported by example code.

### `src/search/dfpn/history.rs`

- Added `use crate::search::ordering::MoveScorer;` so `sort_moves` can call the trait method `score` on the concrete `StaticAtomicScorer`.
- No other logic changed.

### Public API

`MoveScorer` and `StaticAtomicScorer` remain public in `src/search/ordering.rs`.  `examples/static_move_scores.rs` continues to use `MoveScorer::score` on `StaticAtomicScorer` directly and still compiles.

## Benchmarks

All timings are wall-clock seconds for `cargo run --release -- --fen ...` with the default 5-second timeout.  The "before" numbers are from the release-profile-tuned build produced by Plan 1 (`Box<dyn MoveScorer>` still in place); the "after" numbers are from the same build with the concrete scorer.  Each FEN was run 10 times and the first (cold-start) run is shown separately.

| FEN | Outcome / PV | Before (Plan 1 profile, boxed scorer) | After (concrete scorer) | Change |
|-----|--------------|--------------------------------------:|------------------------:|-------:|
| `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` | win, `f1f7 e8d8 g1g8` | cold 0.046, warm mean 0.017 | cold 0.042, warm mean 0.016 | within noise |
| `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` | win, `h5d5 d7d6 d5f7 e8d7 f7e7` | mean 1.579 | mean 1.566 | ~1% faster, within noise |
| `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` | win, `a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6` | mean 0.247 | mean 0.255 | ~3% slower, within noise |
| `4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19` (m19) | draw (5 s timeout) | mean 5.007 | mean 5.007 | unchanged (timeout-limited) |

The wall-clock differences are all within run-to-run noise.  The change does not materially speed up these sample positions.

## Verification

```text
$ cargo fmt
$ cargo clippy --release
$ cargo test --release
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"
$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
```

Results:

- `cargo fmt` completed with no remaining diffs.
- `cargo clippy --release` reports zero warnings.
- `cargo test --release` passes all tests.
- Outcomes and PVs for the sample FENs are identical before and after the change.
- `examples/static_move_scores.rs` builds and runs unchanged.

## Conclusion

The boxed `MoveScorer` trait object was removed from `Search` in favour of a concrete `StaticAtomicScorer`.  This is a safe cleanup that enables the compiler to inline `StaticAtomicScorer::score` directly into `sort_moves`, but on the sampled positions it did not produce a measurable wall-clock speed-up.  The largest remaining move-ordering overheads are likely the O(N log N) repeated scoring (to be addressed in Plan 5) and the static scorer's per-move work, not the dynamic-dispatch cost itself.
