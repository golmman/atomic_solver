# Plan 7 Implementation Report

This report documents the implementation of `docs/plans/review/plan7.md`, which
adds regression tests for the review findings and fixes a path-code collision bug
discovered while writing those tests.

## Changes made

### `tests/test_review.rs` (new)

- `queen_corner_mates_are_loss_for_black` — four single-commoner checkmate
  positions (black king in each corner with a white queen on the adjacent
  diagonal square).  Asserts `Outcome::Loss` for the side to move.
- `stalemate_with_no_commoner_under_attack_is_draw` — two stalemate positions
  where the side to move has a commoner but no legal moves and is not in check.
  Asserts `Outcome::Draw`.
- `two_rook_transposition_still_wins` — the two-rook mate where the rooks can
  be developed in either order, stressing move-order transpositions.
- `promotion_transposition_still_wins` — white pawns on a7 and b7 can both
  promote to queen in either order, leading to the same board state.  Asserts
  `Outcome::Win` and a reasonable node count.

### `src/position.rs`

- Added `#[cfg(test)] mod tests` with unit tests for `Position::outcome()`:
  - `outcome_prefers_own_commoner_extinction_over_rule50`
  - `outcome_prefers_opponent_extinction_over_rule50`
  - `no_legal_moves_is_not_decided_by_position_outcome` — confirms that a
    stalemate position has `outcome() == None` and an empty legal-move list,
    so the solver is responsible for classifying it as a draw.

### `src/zobrist.rs`

- Added `move_order_path_codes_differ_for_same_final_board`, a unit test that
  checks two promotion transpositions (`a7-a8Q` then `b7-b8Q` vs the reverse)
  do not have the same path code.
- **Bug fix:** the old `path_random` implementation computed
  `path_move_keys[move_index] ^ path_depth_keys[depth]`.  Because both the
  move and depth were XORed independently, the resulting path code was
  order-insensitive: swapping which move occurred at which depth left the total
  XOR unchanged.  The new implementation mixes `(move_index, depth)` into a
  single 64-bit value and applies one `SplitMix64` round, which is a bijection on
  `u64` and therefore gives a distinct key for every `(move, depth)` pair.  This
  makes path codes order-sensitive, so transpositions reached by different move
  orders no longer collide.

### `src/search/dfpn.rs`

- Strengthened `validate_pv` so it can validate terminal positions that are
  checkmate or stalemate.  `Position::outcome()` detects commoner extinction
  and rule50/two-piece draws, but not no-legal-move terminals, so the replay
  now falls back to `legal_moves_with_state` and classifies:
  - no legal moves + in check => `Outcome::Loss`
  - no legal moves + not in check => `Outcome::Draw`
- This fixed spurious validation warnings for immediate checkmates where the
  principal variation is empty and `Position::outcome()` returns `None`.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
```

All passed:

- `cargo clippy --all-targets` is clean.
- `cargo test` passes all tests, including the new `test_review` regression
  suite, the `position` unit tests, and the `zobrist` path-code test.
- `cargo doc` builds without warnings.

## Remaining concerns

- The `promotion_transposition_still_wins` test asserts `nodes < 20_000`; this
  is a coarse smoke test.  A tighter bound would require more tuning of the
  timeout and move ordering.
- Forced-repetition / cyclic-defense cases are still only partially covered.
  The `dfpn` simulation unit tests from earlier plans handle some repeated-state
  twin logic, but a dedicated end-to-end cyclic draw position could be added.
