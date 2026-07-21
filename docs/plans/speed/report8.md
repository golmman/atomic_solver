# Report: Plan 8 — Improve and tune move ordering

This report documents the application of `docs/plans/speed/plan8.md`.

## Changes applied

### `src/search/ordering.rs`

- Added `nearest_commoner_map(board, them) -> [i8; 64]` to compute the nearest
  enemy commoner distance for every square once per node, instead of scanning
  the enemy commoners for every `from`/`to` pair in `score`.
- Added `StaticAtomicScorer::score_with_map(board, m, state, nearest) -> i32`.
  This is the same atomic-chess move-ordering logic as the `MoveScorer::score`
  trait method but uses the precomputed distance map.
- Kept `impl MoveScorer for StaticAtomicScorer` as a compatibility wrapper that
  builds a fresh map and calls `score_with_map`.
- Removed the now-unused `nearest_commoner_dist` helper.

### `src/search/dfpn/history.rs`

- `sort_moves` now builds the nearest-commoner map once and calls
  `score_with_map` for every move.
- Removed the unused `MoveScorer` import.

### `examples/static_move_scores.rs`

- Updated the example to precompute the nearest-commoner map once and call
  `StaticAtomicScorer::score_with_map`, matching the new solver fast path.

## What was tried and reverted

A depth-scaled history bonus was implemented (`HISTORY_BONUS * depth * depth`,
capped at `HISTORY_MAX`) on the theory that moves which solve near the root
should receive a bigger history reward than deep flukes.  It compiled and all
outcome tests passed, but it caused a dramatic node-count regression on the
promotion-transposition position (`4k3/PP6/8/8/8/8/8/4K3 w - - 0 1`), which
went from ~0.22 s to the 5 s timeout, and `tests/test_review.rs` jumped from
~1.5 s to ~5 s.  The bonus was too large too quickly and caused the search to
chase historically successful but suboptimal moves.  The change was reverted;
`update_history` uses the original flat `HISTORY_BONUS` again.

## Benchmarks

Wall-clock seconds for `cargo run --release -- --fen ...` with the default
5-second timeout.  The "before" numbers are from the Plan 7 build; the "after"
numbers are from the Plan 8 build with the distance-map precomputation.

| FEN | Outcome / PV | Before (Plan 7) | After (Plan 8) | Change |
|-----|--------------|----------------:|---------------:|-------:|
| `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` | win, `f1f7 e8d8 g1g8` | warm mean 0.014 | warm mean 0.016 | within noise |
| `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` | win, `h5d5 d7d6 d5f7 e8d7 f7e7` | mean 1.443 | mean 1.452 | within noise |
| `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` | win, `a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6` | mean 0.218 | mean 0.224 | within noise |
| `4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19` (m19) | draw (timeout) | 5.006 | 5.007 | unchanged (timeout-limited) |

Because the scoring function itself is unchanged, the node counts for the
sample FENs are the same as in Plan 7; the only expected difference is a
reduction in per-node nearest-commoner recomputation.  That saving is small on
the tested positions (the distance scan is already cheap when the opponent has
one or two commoners), so the wall-clock numbers stay within run-to-run noise.

The `depth * depth` history experiment showed how sensitive the solver is to
dynamic history tuning: a poorly chosen scale can increase node counts by an
order of magnitude on TT-heavy positions.

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

- `cargo fmt` completed with no diffs.
- `cargo clippy --all-targets` reports zero warnings.
- `cargo test --release` passes all tests.
- `cargo doc --no-deps` builds cleanly.
- `examples/static_move_scores` prints the expected scores for the two-rook position.
- The sample FENs produce identical outcomes and PVs.

## Conclusion

The nearest-commoner distance map removes repeated per-move scanning of the
enemy commoners in the static scorer.  This is a clean, correctness-preserving
optimization that becomes more valuable when the opponent has several commoners
(e.g. through promotion).  The riskier history-scaling idea was tried and
reverted because it hurt node counts.  Further move-ordering work should focus
on safer, measurable improvements such as atomic-chess-specific capture blast
evaluation or a carefully tuned, smaller history bonus with regression tests on
the promotion-transposition and `test_review` suites.
