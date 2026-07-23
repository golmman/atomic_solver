# Report: Plan 2 — Tighten the threshold formula and tune epsilon

This report documents the implementation of `docs/plans/ultimattt/plan2.md`.

## Summary

`Search::epsilon_ceil` was already equivalent to the `ultimattt` threshold
formula, so no algorithm change was needed there.  The default `epsilon` was
lowered from `0.25` to `0.125` (the value recommended by the DF-PN+ literature)
and the CLI / benchmark defaults were updated to match.

## Changes applied

### `src/search/dfpn/mod.rs`

- `DEFAULT_EPSILON` changed from `0.25` to `0.125`.

### `src/main.rs`

- The default CLI `epsilon` value and the help text updated from `0.25` to
  `0.125`.  (`--epsilon` was already supported.)

### `examples/benchmark.rs`

- The default benchmark `epsilon` value updated from `0.25` to `0.125`.
  (`--epsilon` was already supported.)

## Epsilon sweep

Ran the benchmark suite over the requested epsilon values with
`--runs 1 --timeout 5`:

```text
$ for eps in 0.0 0.0625 0.125 0.25 0.5; do
      cargo run --release --example benchmark -- --runs 1 --timeout 5 --epsilon $eps
done
```

All decisive positions returned `win` for every epsilon; `m19` and `startpos`
hit the 5-second limit and returned `draw` for every epsilon, so outcomes were
unchanged.

### Total node counts across the full suite

| epsilon | total nodes | total child_evals |
|--------:|------------:|------------------:|
| 0.0     | 1,497,975   | 33,409,263        |
| 0.0625  | 1,519,098   | 33,838,223        |
| 0.125   | 1,483,488   | 33,358,943        |
| 0.25    | 1,471,863   | 33,518,948        |
| 0.5     | 1,469,680   | 33,665,542        |

The two timeout-limited positions dominate the totals and make the absolute
node counts noisy.  Restricting to the six decisive positions:

| epsilon | decisive nodes | decisive child_evals |
|--------:|---------------:|-----------------------:|
| 0.0     | 3,579          | 39,334                 |
| 0.0625  | 4,020          | 44,034                 |
| 0.125   | 3,029          | 39,622                 |
| 0.25    | 2,872          | 40,159                 |
| 0.5     | 3,788          | 49,928                 |

`0.0` and `0.25` trade the lead on different positions, while `0.125` is a
balanced middle ground and matches the `ultimattt` / DF-PN+ recommendation.  The
new default of `0.125` was chosen.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo test --release
$ cargo doc --no-deps
```

All clean and passing.

### `fen1` and `fen2`

`fen2`:

```text
$ cargo run --release -- --fen \
    '6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26' \
    --no-refine-shortest --timeout 60
outcome: win
pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8
```

`fen1`:

```text
$ cargo run --release --example solve_no_refinement -- \
    '6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25'
outcome: Loss nodes: 1157
g8g7
b1b8
g7h7
b8h8
h7g7
h8h7
g7g8
h7g7
g8h8
g7g8
h8h7
g8g6
```

Both return the same outcomes and PVs; only the exact node count varies with
epsilon (`fen1` moved from `1129` to `1157` nodes, still well within the same
fast range).

## Files changed

- `src/search/dfpn/mod.rs`
- `src/main.rs`
- `examples/benchmark.rs`
- `docs/plans/ultimattt/report2.md` (this report)
