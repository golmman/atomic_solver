# Plan 6 Implementation Report

This report documents the implementation of `docs/plans/review/plan6.md`, which
strengthens the PV validation logic in `src/search/dfpn.rs`.

## Changes made

### `src/search/dfpn.rs`

- `validate_pv` now accepts the expected `Outcome` and an optional expected
  depth/length, in addition to the position and PV.
- The replay loop is now robust:
  - It clones the position.
  - For each move it generates the legal move list and verifies the PV move is
    contained in that list before playing it.  This catches illegal moves such as
    corrupted TT entries or malformed PVs.
  - After the last move it compares the terminal position's outcome to the
    expected value, taking the side-to-move perspective into account.  Because
    `Outcome` is from the player to move, the expected outcome is flipped when the
    PV length is odd: a root `Win` typically ends with the opponent to move and
    `Outcome::Loss`.
  - If an expected depth is supplied, the PV length is also checked.
- `extract_pv_checked` was updated to take the expected `Outcome` and optional
  depth and to print a warning when validation fails.
- All call sites were updated:
  - `solve` now validates the PV with the returned outcome and falls back to the
    unchecked `extract_pv` if validation fails.
  - `solve_refined` passes the full-depth outcome and, for the initial full
    search, the full-depth reported depth.  Intermediate binary-search probes use
    outcome-only validation.
- Added unit tests for `validate_pv`:
  - `validate_pv_accepts_valid_win` — a legal mate-in-one PV passes for `Win` and
    the correct depth, and fails for wrong depth or wrong outcome.
  - `validate_pv_rejects_illegal_move` — a PV containing a move not in the legal
    move list is rejected.
  - `validate_pv_rejects_wrong_terminal_outcome` — an empty PV for a drawn
    position is accepted for `Draw` but rejected for `Win`.

## Why the side-to-move adjustment is necessary

`Outcome` is defined from the perspective of the side to move.  After a winning
PV of odd length, the final position has the opponent to move and no commoners,
so `Position::outcome()` returns `Outcome::Loss`.  That is the correct terminal
result for the original player, so `validate_pv` compares against
`expected.flip()` for odd-length PVs.  Draws are symmetric, so they are
unaffected.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
```

All passed:

- `cargo clippy --all-targets` is clean.
- `cargo test` passes all tests, including the three new `validate_pv` unit tests.
- `cargo doc` builds without warnings.

## Remaining concerns

- `validate_pv` generates legal moves for every PV step, which is more expensive
  than the previous terminal-only check.  Because PVs are short and validation
  only runs at the end of a solve, the overhead is acceptable, but it could be
  measured on very long PVs.
- The validation warning goes to `stderr`.  A future logging integration could
  make this configurable.
