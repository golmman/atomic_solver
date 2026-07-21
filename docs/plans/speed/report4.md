# Report: Plan 4 — Avoid the double `tt.probe` in `dfpn`

This report documents the application of `docs/plans/speed/plan4.md`.

## Changes applied

### `src/search/dfpn/core.rs`

At the top of `dfpn` the same `tt_key` was previously probed twice:

1. Once to fetch an entry for `try_use_tt`.
2. A second time to extract `best_from_tt` when `try_use_tt` did not produce an early return.

The function now probes once and keeps the resulting `TtEntry` in a local `Option<TtEntry>`:

```rust
let tt_entry = self.tt.probe(tt_key).copied();
if let Some(entry) = tt_entry
    && let Some(resolved) =
        self.try_use_tt(pos, &entry, max_depth, self.path_code, path_length)
{
    return resolved.outcome;
}

if !self.path.insert(rep_key) {
    return Outcome::Draw;
}

let best_from_tt = tt_entry
    .and_then(|e| { /* best-move extraction */ })
    .unwrap_or(Move::NONE);
self.sort_moves(pos, &mut moves, best_from_tt);
```

`TtEntry` is `Copy`, so the local copy is used for both the `try_use_tt` check and the best-move extraction.  This removes the second `tt.probe(tt_key)` call and the second `TtEntry` copy on every non-early-return node.

## Why the local copy stays valid

`try_use_tt` can mutate the transposition table via `store_twin`, but it only calls `store_twin` immediately before returning `Some(...)`.  In that case `dfpn` returns early and `best_from_tt` is never used.  When `try_use_tt` returns `None` no mutation occurs, so the local `tt_entry` copy is still the current entry and its `best_move`/`best_result_for_path` data is safe to use for move ordering.

## Benchmarks

Wall-clock seconds for `cargo run --release -- --fen ...` with the default 5-second timeout.  The "before" numbers are from the Plan 3 build; the "after" numbers are from the same build with the single `tt.probe`.

| FEN | Outcome / PV | Before (Plan 3) | After (single probe) | Change |
|-----|--------------|----------------:|---------------------:|-------:|
| `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` | win, `f1f7 e8d8 g1g8` | warm mean 0.016 | warm mean 0.019 | within noise |
| `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` | win, `h5d5 d7d6 d5f7 e8d7 f7e7` | mean 1.576 | mean 1.538 | ~2% faster |
| `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` | win, `a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6` | mean 0.257 | mean 0.243 | ~5% faster |
| `4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19` (m19) | draw (timeout) | 5.006 | 5.007 | unchanged (timeout-limited) |

The two longer non-timeout positions (`epsilon` mate and promotion transposition) both improved by a few percent.  This is consistent with removing a repeated probe and `TtEntry` copy in the hot recursive loop.  The two-rook position is too fast for the difference to be distinguishable from noise.

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

- `cargo fmt` completed with no changes.
- `cargo clippy --all-targets` reports zero warnings.
- `cargo test --release` passes all tests.
- `cargo doc --no-deps` builds cleanly.
- The sample FENs produce identical outcomes and PVs.

## Conclusion

The double `tt.probe` was removed by keeping the first `TtEntry` copy in a local variable and deriving `best_from_tt` from it.  This is a safe refactor that reduces transposition-table probe overhead and removes one large `TtEntry` copy per node.  On the sampled positions the speed-up is modest (a few percent) but in the expected direction, and the test suite confirms no functional change.
