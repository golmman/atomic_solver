# Plan 1: Correct terminal and outcome classification

## Start

- Read `docs/plans/review/review1.md` (the source review that acts as the report
  for plan 0) to confirm the baseline state and the specific issues this plan
  addresses: section 2.1, section 3.4, and section 3.5.

## Goal

Fix the solver so that a position with no legal moves is classified as
`Outcome::Loss` when the side to move's commoner is attacked (checkmate) and as
`Outcome::Draw` when it is not (stalemate). While doing this, also fix the
defensive ordering of `Position::outcome` and remove the ambiguous
`outcome_from_pn_dn` mapping.

## Background

- `Position::outcome()` in `src/position.rs` does not consider the no-legal-moves
  case.
- `dfpn.rs` returns `Outcome::Draw` unconditionally when `moves.is_empty()`.
- `StateInfo::checkers` from `atomic_movegen` already tells us whether a
  commoner is attacked.
- `outcome_from_pn_dn` maps `(INF, 0)` to `Loss`, which is also the encoding for
  `Draw`.
- `Position::outcome` checks `rule50 >= 100` before own-commoner extinction,
  which is not defensive.

## Implementation tasks

1. Add `Position::legal_moves_with_state(&self, moves, state)` that calls
   `generate_legal_with_state` (or populates a `StateInfo` and reuses it) so the
   search can obtain both the move list and the checker state without generating
   moves twice.
2. In `dfpn.rs`, replace the empty-move-list branch with a check of
   `state.checkers`. If the side to move has no legal moves and is in check
   (`!state.checkers.is_empty()`), store and return `Outcome::Loss`; otherwise
   store and return `Outcome::Draw`.
3. Reorder `Position::outcome` to check own-commoner extinction and
   opponent-commoner extinction before the 50-move and two-piece draws.
4. Fix `outcome_from_pn_dn` so it returns `None` for `(INF, 0)` because that
   pair is shared by `Loss` and `Draw`. Document that `pn/dn` alone cannot
   distinguish `Loss` from `Draw`; the `outcome` field is the source of truth.
5. Add unit/integration tests for:
   - the `7K/8/8/8/8/8/1Q6/k7 b - - 0 1` checkmate (expected `Loss` for Black);
   - a stalemate position with no commoner under attack (expected `Draw`);
   - `Position::outcome` on malformed FENs where `rule50 >= 100` but own
     commoners are gone.
6. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
7. Final task: write `docs/plans/review/report1.md` documenting the implemented
   changes, test results, and any remaining concerns.

## File changes

- `src/position.rs`
- `src/search/dfpn.rs`
- `tests/test_inf.rs` or a new regression test file

## Risks

- `generate_legal_with_state` may have a different signature than
  `generate_legal`; verify the `atomic_movegen` 2.0.0 API first.
- Treating all `state.checkers != 0` positions with no legal moves as `Loss`
  assumes the game ends when any commoner is attacked, which matches the
  check-enforcing rules used by `atomic_movegen`.

## Verification

- `cargo test` passes.
- The checkmate FEN returns `Outcome::Loss`.
- The stalemate FEN returns `Outcome::Draw`.
