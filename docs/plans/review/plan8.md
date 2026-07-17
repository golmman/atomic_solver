# Plan 8: Expose `epsilon` at runtime

## Start

- Read `docs/plans/review/report7.md` to confirm the regression suite is in
  place and note any blockers before adding runtime epsilon configuration.

## Goal

Allow users to tune the DF-PN+ epsilon multiplier without recompiling. Add a
`Search::set_epsilon` method and a CLI flag.

## Background

`EPSILON` is a compile-time constant (`0.25`). The research notes and review
note that a `set_epsilon` method/runtime flag is a useful convenience for
tuning.

## Implementation tasks

1. Add a `pub fn set_epsilon(&mut self, epsilon: f64)` method to `Search` in
   `src/search/dfpn.rs`. Validate `epsilon > 0.0` and `epsilon < 1.0` (or
   another sensible range); use `0.25` as default.
2. Use `self.epsilon` in `epsilon_ceil` (already a field) and remove the unused
   compile-time constant if it is now dead code.
3. Add a `--epsilon <value>` command-line option to `src/main.rs`. Parse with
   `f64`; exit with a clear error if the value is out of range or not a number.
4. Add unit tests for `epsilon_ceil` with different epsilon values (e.g.
   `0.0`, `0.25`, `0.5`, `1.0`) and edge cases (`INF`, `0`, `1`).
5. Add an integration test that runs the CLI with `--epsilon` and verifies it
   still solves a simple position.
6. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
7. Final task: write `docs/plans/review/report8.md` documenting the runtime
   epsilon API, the accepted range, and the default behavior.

## File changes

- `src/search/dfpn.rs`
- `src/main.rs`
- `tests/` (integration test)

## Risks

- Very large epsilon values can overshoot thresholds and reduce search quality.
  Enforce a guard or document the recommended range.
- The CLI already has a simple argument parser; be careful not to break `--fen`.

## Verification

- `cargo run -- --epsilon 0.25 --fen <FEN>` works.
- `cargo run -- --epsilon 0.5 --fen <FEN>` works and returns the same outcome
  as the default.
- Out-of-range values produce a clear error.
