# Plan 2: Tighten the threshold formula and tune epsilon

## Goal

Reduce over-expansion of the most-proving child in DF-PN by using a tighter, more principled threshold formula and by finding a better default `epsilon` for atomic chess.

## Background

The `ultimattt` threshold for an OR node is:

```rust
delta = min(parent_phi, max(delta_2 + 1, delta_2 * (1.0 + epsilon)))
```

`atomic_solver` already implements the equivalent in `Search::epsilon_ceil`:

```rust
let scaled = (x as u128 * epsilon_num).div_ceil(epsilon_den as u128) as u64;
scaled.max(x.saturating_add(1)).min(INF)
```

The formula is correct. The remaining work is to verify it and tune the `DEFAULT_EPSILON` constant, which is currently `0.25`. The `ultimattt` paper recommends `1/8` (`0.125`) for DF-PN+ when the transposition table is large relative to the tree, and warns that larger epsilon can cause over-exploration of a single child. This matches the `fen1` behavior, where the `max_depth=8` search over-expands one subtree.

## Files to modify

- `src/search/dfpn/mod.rs` (`DEFAULT_EPSILON` constant)
- `src/main.rs` (optional: add `--epsilon <f64>` CLI argument)
- `src/search/dfpn/core.rs` (no change unless the formula is found to be wrong)
- `examples/benchmark.rs` (add an `--epsilon` flag for sweeps)

## Concrete changes

1. Verify the `epsilon_ceil` implementation against the `ultimattt` formula and the test cases in `core.rs`.
2. Lower `DEFAULT_EPSILON` from `0.25` to `0.125`.
3. Add an optional `--epsilon` CLI argument to `main.rs` so the value can be changed without recompiling.
4. Add an optional `--epsilon` flag to `examples/benchmark.rs` so the benchmark suite can be swept over multiple values.

## Verification

- Run `cargo test` to ensure the `epsilon_ceil` unit tests still pass.
- Run a sweep over `epsilon` values `0.0, 0.0625, 0.125, 0.25, 0.5` on `examples/benchmark.rs`.
- Choose the default that minimizes total node count across the benchmark suite without changing any outcomes.
- Verify `fen1` and `fen2` still return correct results.

## Risks / notes

- `epsilon = 0.0` reproduces classic `+1` DF-PN and is useful as a baseline, but may re-search the same child many times when the TT is small.
- `epsilon` above `0.5` can cause the search to explore one child too deeply before switching to siblings, which is the opposite of the desired effect.
- Outcomes must be identical for all valid `epsilon` values; if they differ, there is a bug in the threshold logic or `pn`/`dn` propagation.
