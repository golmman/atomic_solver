# Plan 10: Centralize no-legal-move terminal detection in `Position`

## Start

- Read `docs/plans/review/report9.md` to confirm the `epsilon_ceil` fix is stable
  and the test suite is green before refactoring terminal detection.

## Goal

Add a single `Position` helper that classifies no-legal-move positions as
`Outcome::Loss` (checkmate) or `Outcome::Draw` (stalemate), and remove the
duplicated logic from `src/search/dfpn.rs` and `validate_pv`.

## Background

- `Position::outcome()` in `src/position.rs` handles commoner extinction,
  rule50, and the two-piece draw rule, but it does not detect checkmate or
  stalemate.
- `dfpn` and `validate_pv` both generate legal moves, inspect
  `StateInfo::checkers`, and return `Loss` or `Draw` independently.
- `simulate` relies on `pos.outcome()` and therefore cannot validate terminal
  checkmate or stalemate nodes.
- Centralizing the logic removes inconsistency and makes `simulate` simpler to
  fix.

## Implementation tasks

1. Add a private or public helper in `src/position.rs`, e.g.
   `Position::terminal_from_movegen(&self) -> Option<Outcome>`, that:
   - Creates a fresh `StateInfo`.
   - Generates legal moves with that state.
   - Returns `Some(Outcome::Loss)` if the move list is empty and
     `state.checkers` is non-empty.
   - Returns `Some(Outcome::Draw)` if the move list is empty and
     `state.checkers` is empty.
   - Returns `None` if legal moves exist.
2. Integrate the helper into `Position::outcome()` so that `outcome()` becomes the
   canonical terminal detector. The recommended order is:
   - Own commoner extinction -> `Loss`
   - Opponent commoner extinction -> `Win`
   - `rule50 >= 100` -> `Draw`
   - Two-piece draw -> `Draw`
   - No legal moves -> `Loss` or `Draw` via the helper

   If generating moves inside `outcome()` causes duplicate work in `dfpn`,
   provide an additional helper `outcome_from_state(&self, state, moves)` for
   callers that already have a move list.
3. Replace the empty-move-list branch in `src/search/dfpn.rs` (around lines
   367-386) with a call to the centralized terminal helper or `pos.outcome()`.
4. Replace the manual terminal-detection block in `validate_pv` (around lines
   279-294) with a call to `pos.outcome()`.
5. Ensure `simulate` uses the centralized terminal detection both at its top-level
   check and in the `Outcome::Loss` empty-move branch.
6. Add unit tests in `src/position.rs` for:
   - A checkmate position (`7K/8/8/8/8/8/1Q6/k7 b - - 0 1` -> `Loss` for Black)
   - A stalemate position with no commoner under attack -> `Draw`
   - A position with legal moves -> `None`
7. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
8. Final task: write `docs/plans/review/report10.md` documenting the centralized
   terminal helper, removed duplication, and test coverage.

## File changes

- `src/position.rs`
- `src/search/dfpn.rs`

## Risks

- Generating legal moves inside `outcome()` adds work if the caller immediately
  generates moves again. If profiling shows a problem, split the fast
  rule-based checks and the move-generation terminal check into two methods.
- `outcome()` keeps its `&self` signature, so all `StateInfo`/`MoveList` values
  must be local.

## Verification

- `cargo test` passes, including the new `Position` terminal tests.
- `cargo run -- --fen "7K/8/8/8/8/8/1Q6/k7 b - - 0 1"` returns `loss` for Black.
- A known stalemate FEN returns `draw`.
