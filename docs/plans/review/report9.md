# Plan 9 Implementation Report

This report documents the implementation of `docs/plans/review/plan9.md`, which
fixes `epsilon_ceil` for `ε = 0.0` so it behaves like the original `+1` DF-PN
threshold.

## Changes made

### `src/search/dfpn.rs`

- Updated `epsilon_ceil` to enforce a minimum step of `x + 1` for finite `x`:

  ```rust
  fn epsilon_ceil(&self, x: u64) -> u64 {
      if x >= INF {
          return INF;
      }
      let scaled = (x as f64 * (1.0 + self.epsilon)).ceil() as u64;
      scaled.max(x.saturating_add(1)).min(INF)
  }
  ```

  This guarantees progress for every valid `ε`, because the child threshold is
  always strictly larger than the sibling bound.
- `ε = 0.0` now reproduces the classic `p2 + 1` / `d2 + 1` threshold exactly,
  instead of returning the same bound and potentially looping forever.
- The `set_epsilon` range check remains `[0.0, 1.0]`; `0.0` is strict `+1`
  DF-PN and `1.0` allows thresholds up to twice the sibling bound.
- Updated the `epsilon_ceil_scales_threshold` unit test:
  - For `ε = 0.0`, asserts `epsilon_ceil(x) == x + 1` for `0`, `1`, `5`, and
    `100`, and `INF` for `x >= INF`.
  - For `ε = 0.25`, asserts the multiplicative ceiling still holds for `1`,
    `10`, and `100`, with `0` now returning `1`.

### `tests/test_epsilon.rs`

- Added `solve_with_epsilon_full`, a helper that returns the outcome, PV (as UCI
  strings), and node count.
- Added `epsilon_zero_solves_mate_in_two`, an integration test for the FEN from
  `review2.md` (`rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3`).
  - Asserts `Outcome::Win` and a non-empty PV with `ε = 0.0`.
  - Ignored in debug builds (`#[cfg_attr(debug_assertions, ignore = "slow in debug builds")]`) so `cargo test` stays fast, and runs in release builds.

### `docs/plans/dfpn/research_epsilon.md`

- Updated the `epsilon_ceil` code snippet and description to reflect the `x + 1`
  minimum step.
- Documented that `ε = 0.0` is implemented as `x + 1`, not as pure multiplicative
  scaling, and that the valid range stays `[0.0, 1.0]`.
- Clarified the correctness verification step to note that `ε = 0.0` now
  reproduces the original `+1` threshold exactly.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
$ cargo test --release
```

All commands completed successfully. The `cargo test` run includes the updated
`epsilon_ceil_scales_threshold` unit test, and `cargo test --release` runs the
new `epsilon_zero_solves_mate_in_two` integration test.

CLI verification on the mate-in-two position:

```text
$ cargo run --release -- --epsilon 0.0 --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"
outcome: win
pv: h5d5 d7d6 d5f7 e8d7 f7e7
nodes: 369330
outcome: win
pv: h5d5 d7d6 d5f7 e8d7 f7e7

$ cargo run --release -- --epsilon 0.25 --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"
outcome: win
pv: h5d5 d7d6 d5f7 e8d7 f7e7
nodes: 372384
outcome: win
pv: h5d5 d7d6 d5f7 e8d7 f7e7
```

Both `ε = 0.0` and `ε = 0.25` return `outcome: win` with a non-empty PV. The
node counts are similar, confirming that the `+1` floor does not materially
change the search effort for this position.

## Performance observations

- The `x + 1` floor slightly raises thresholds for very small sibling bounds,
  which can reduce root re-searches at the beginning of the search.
- For `ε = 0.0` on the mate-in-two position, the solver completed well within
  the default 5-second timeout in release mode.
- The duplicate `outcome: win` / `pv:` output in the CLI is existing behavior
  (`refine_shortest` prints an update and `main` prints the final result) and
  was not modified by this plan.

## Remaining concerns

- None for this plan. The `ε = 0.0` path is now safe for any non-immediate-mate
  position, because the child threshold is always at least one unit larger than
  the sibling bound.
