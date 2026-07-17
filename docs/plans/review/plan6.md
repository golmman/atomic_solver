# Plan 6: Strengthen PV validation

## Start

- Read `docs/plans/review/report5.md` to confirm `solve_refined` changes are
  stable and note any blockers before changing PV validation.

## Goal

`validate_pv` should verify that every move in the principal variation is legal,
that the final position's outcome matches the reported result, and that the PV
length is consistent with the reported depth.

## Background

The current `validate_pv` only replays moves and checks that the final position
is terminal. It does not check move legality, the correct outcome, or the depth.

## Implementation tasks

1. Change `validate_pv` to accept the expected `Outcome` and the expected
   depth/length (or `Option<u32>`) in addition to the `Position`.
2. Implement a robust replay:
   - Clone the position.
   - For each move, generate the legal moves at that step and confirm the move
     is in the list before playing it.
   - After all moves, check `current.outcome() == Some(expected)`.
   - Optionally assert the PV length equals the reported depth.
3. Update all call sites (`extract_pv_checked` and `solve_refined`) to pass the
   expected outcome.
4. If a PV fails validation, log/print a warning and fall back to the
   unvalidated `extract_pv` so the solver still returns a result.
5. Add unit tests in `src/search/dfpn.rs` for:
   - a valid winning PV,
   - an illegal move in a PV (should fail),
   - a PV that ends in the wrong terminal outcome (should fail).
6. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
7. Final task: write `docs/plans/review/report6.md` documenting the new
   validation logic and test coverage.

## File changes

- `src/search/dfpn.rs`

## Risks

- Move-legality checking by generating legal moves for every PV step is more
  expensive, but PVs are short and this is only done at the end of a solve.
- `MoveList` may not implement `contains`; implement a small helper if needed.

## Verification

- New unit tests pass.
- `cargo run -- --fen <win-FEN>` produces a validated PV.
