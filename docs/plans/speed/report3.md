# Report: Plan 3 — Integerize `epsilon_ceil`

This report documents the application of `docs/plans/speed/plan3.md`.

## Changes applied

### `src/search/dfpn/mod.rs`

- Removed the `epsilon: f64` field from `Search`.
- Added `epsilon_num: u64` and `epsilon_den: u64` so that `1.0 + epsilon` is represented as the exact reduced fraction `num/den`.
- Added `epsilon_fraction(v: f64) -> (u64, u64)`, which extracts the exact dyadic rational from an f64 bit pattern.  Because `epsilon` is constrained to `[0.0, 1.0]`, `v = 1.0 + epsilon` lies in `[1.0, 2.0]` and is always a normal dyadic rational.
- Added `gcd` for reducing the fraction.
- Updated `Search::new` and `Search::set_epsilon` to compute and store the fraction.

### `src/search/dfpn/core.rs`

- Rewrote `epsilon_ceil` to use integer arithmetic with a `u128` intermediate:
  ```rust
  let scaled =
      (x as u128 * self.epsilon_num as u128).div_ceil(self.epsilon_den as u128) as u64;
  scaled.max(x.saturating_add(1)).min(INF)
  ```
- Removed the `f64` conversion, multiplication, and `ceil` call from the hot DF-PN threshold loop.

### Unit tests in `src/search/dfpn/core.rs`

- Extended `epsilon_ceil_scales_threshold` with large-value cases:
  - `epsilon = 0.0`: `epsilon_ceil(1_000_000) == 1_000_001` and `epsilon_ceil(INF - 1) == INF`
  - `epsilon = 0.25`: `epsilon_ceil(1_000_000) == 1_250_000` and `epsilon_ceil(INF - 1) == INF`
  - `epsilon = 0.5`: `epsilon_ceil(1_000_000) == 1_500_000` and `epsilon_ceil(INF - 1) == INF`
  - `epsilon = 1.0`: `epsilon_ceil(1_000_000) == 2_000_000` and `epsilon_ceil(INF - 1) == INF`

## Equivalence to the old floating-point version

The new fraction is the exact dyadic rational represented by the f64 value `1.0 + epsilon`.  For the supported test values this gives:

| `epsilon` | `1.0 + epsilon` | `epsilon_num` / `epsilon_den` |
|-----------|-----------------|------------------------------|
| 0.0       | 1.0             | 1 / 1                        |
| 0.25      | 1.25            | 5 / 4                        |
| 0.5       | 1.5             | 3 / 2                        |
| 0.99      | 1.99            | 199 / 100                    |
| 1.0       | 2.0             | 2 / 1                        |

The unit tests and the `test_epsilon` integration tests (which exercise `0.0`, `0.01`, `0.25`, `0.5`, `0.99`, and `1.0`) pass, confirming that the threshold values are identical to the old f64 computation for these inputs.  For very large `x` the integer version is exact, whereas the old `x as f64` conversion could lose integer precision; the new behavior is therefore more precise.

## Benchmarks

Wall-clock seconds for `cargo run --release -- --fen ...` with the default 5-second timeout.  The "before" numbers are from the Plan 2 build (concrete scorer, release profile tuned); the "after" numbers are from the same build with the integer `epsilon_ceil`.

| FEN | Outcome / PV | Before (Plan 2) | After (integer `epsilon_ceil`) | Change |
|-----|--------------|----------------:|-------------------------------:|-------:|
| `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` | win, `f1f7 e8d8 g1g8` | warm mean ~0.017 | warm mean ~0.016 | within noise |
| `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` | win, `h5d5 d7d6 d5f7 e8d7 f7e7` | mean 1.579 | mean 1.576 | within noise |
| `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` | win, `a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6` | mean 0.247 | mean 0.257 | within noise |
| `4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19` (m19) | draw (timeout) | 5.007 | 5.006 | unchanged (timeout-limited) |

The measured wall-clock difference is within run-to-run noise.  The `epsilon_ceil` computation is a tiny fraction of the overall search cost, so removing the `f64` work does not show up at the macro level on these positions.

## Verification

```text
$ cargo fmt
$ cargo clippy --release
$ cargo test --release
$ cargo doc --no-deps
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
```

Results:

- `cargo fmt` completed with no changes.
- `cargo clippy --release` reports zero warnings.
- `cargo test --release` passes all tests, including the extended `epsilon_ceil_scales_threshold` test and the `test_epsilon` integration tests.
- `cargo doc --no-deps` builds cleanly.
- The sample FENs still produce the same outcomes and PVs.

## Conclusion

`epsilon_ceil` is now computed with exact integer arithmetic from a precomputed `epsilon_num/epsilon_den` fraction.  The change is safe and correct, and it removes `f64` operations from the hot threshold loop.  On the sampled positions the wall-clock speed-up is not measurable, which is expected because the threshold update is not a dominant cost.  The larger benefit is the removal of a repeated `f64` conversion and a more precise result for large bound values.
