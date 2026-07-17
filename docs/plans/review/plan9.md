# Plan 9: Fix `epsilon_ceil` for `ε = 0.0`

## Start

- Read `docs/plans/review/report8.md` to confirm the runtime epsilon API and the
  existing tests are stable before changing the threshold calculation.

## Goal

Make `ε = 0.0` behave like the original DF-PN `+1` threshold, so it does not
thrash and time out on any position that is not an immediate mate.

## Background

- `epsilon_ceil` in `src/search/dfpn.rs` currently returns
  `ceil(x * (1.0 + ε))`.
- For `ε = 0.0` this is `ceil(x * 1.0) = x`, so the child threshold is set to the
  sibling bound instead of one unit above it.
- DF-PN relies on a strictly larger child threshold to guarantee progress; with
  `ε = 0.0` the search can re-enter the same child with the same threshold
  indefinitely.
- `research_epsilon.md` states that `ε = 0.0` reproduces the classic `p2 + 1`
  threshold; the implementation needs to match that intent.

## Implementation tasks

1. Modify `epsilon_ceil` in `src/search/dfpn.rs` so that for finite `x` it
   returns at least `x.saturating_add(1)`. For example:

   ```rust
   let scaled = (x as f64 * (1.0 + self.epsilon)).ceil() as u64;
   scaled.max(x.saturating_add(1)).min(INF)
   ```

   Keep the existing `INF` guard unchanged.
2. Keep `Search::set_epsilon` and the CLI accepting the range `[0.0, 1.0]`;
   `0.0` now means strict `+1` DF-PN and `1.0` allows thresholds up to `2x`.
3. Extend the unit tests for `epsilon_ceil`:
   - For `ε = 0.0` assert `epsilon_ceil(x) == x + 1` for `x = 0, 1, 5, 100`,
     and `INF` for `x >= INF`.
   - For `ε = 0.25` assert the expected multiplicative ceiling still holds.
4. Add an integration test that runs the mate-in-two FEN from `review2.md`
   (`rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3`) with
   `ε = 0.0` and asserts `Outcome::Win` and a non-empty PV within the default
   timeout. Mark it `#[ignore]` or run in release if it is too slow for debug.
5. Update `docs/plans/dfpn/research_epsilon.md` to clarify that `ε = 0.0` is
   implemented as `x + 1`, not as pure multiplicative scaling, and that the valid
   range remains `[0.0, 1.0]`.
6. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
7. Final task: write `docs/plans/review/report9.md` documenting the corrected
   threshold semantics, test results, and any performance observations.

## File changes

- `src/search/dfpn.rs`
- `tests/test_epsilon.rs` (or a new test in `tests/test_review.rs`)
- `docs/plans/dfpn/research_epsilon.md`

## Risks

- `max(x + 1, scaled)` always gives at least a one-unit step, which preserves
  DF-PN progress for all `ε`.
- For `ε > 0` and very small `x`, `x + 1` may dominate the multiplicative step.
  This is safe and may reduce root re-searches.
- The `ε = 0.0` integration test may need a longer timeout in debug builds.

## Verification

- `cargo test` passes, including the new `epsilon_ceil` unit tests.
- `cargo run --release -- --epsilon 0.0 --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"` returns `outcome: win` with a PV.
- `cargo run --release -- --epsilon 0.25` on the same FEN still returns `win` quickly.
