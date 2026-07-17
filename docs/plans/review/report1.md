# Plan 1 Implementation Report

This report documents the implementation of `docs/plans/review/plan1.md`, which
addresses the no-legal-move terminal detection bug and related defensive fixes
identified in `docs/plans/review/review1.md` (sections 2.1, 3.4, and 3.5).

## Changes made

### `src/position.rs`

- Added `Position::legal_moves_with_state(moves, state)`.
  It populates a caller-supplied `StateInfo` with `Board::populate_state` and
  then calls `generate_legal_with_state`, giving callers both the move list and
  the checker state in one generation step.
- Reordered `Position::outcome` to evaluate commoner extinction before the
  50-move and two-piece draw rules:
  1. Own commoners gone -> `Loss`
  2. Opponent commoners gone -> `Win`
  3. `rule50 >= 100` -> `Draw`
  4. Only two pieces remain -> `Draw`

### `src/search/dfpn.rs`

- Replaced the empty-move-list branch in `dfpn`.
  After generating moves with `legal_moves_with_state`, an empty move list now
  checks `state.checkers`:
  - If the side to move's commoner is attacked (`!state.checkers.is_empty()`),
    the position is checkmate and is stored/returned as `Outcome::Loss`.
  - Otherwise the position is stalemate and is stored/returned as
    `Outcome::Draw`.
- Fixed `outcome_from_pn_dn`.
  It now only returns `Some(Outcome::Win)` for `(0, INF)`.  `(INF, 0)` returns
  `None` because that pair is shared by both `Loss` and `Draw`.  Documented
  that the `outcome` field is the source of truth when distinguishing
  `Loss` from `Draw`.

### `tests/test_plan1.rs` (new)

Added regression tests covering:

- The `7K/8/8/8/8/8/1Q6/k7 b - - 0 1` single-commoner checkmate returns
  `Outcome::Loss` for the side to move.
- A stalemate position (`7k/8/8/8/8/8/2q5/K7 w - - 0 1`) where the commoner is
  not attacked returns `Outcome::Draw`.
- `Position::outcome` on malformed FENs where `rule50 >= 100` but own
  commoners are gone still reports `Loss`.
- `Position::outcome` on malformed FENs where `rule50 >= 100` but the
  opponent's commoners are gone still reports `Win`.
- `outcome_from_pn_dn` correctly returns `None` for the ambiguous `(INF, 0)`
  pair and `Some(Outcome::Win)` only for `(0, INF)`.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
```

All passed:

- `cargo clippy --all-targets` is clean.
- `cargo test` passes all non-ignored tests, including the new Plan 1 tests.
- `cargo doc` builds without warnings.
- CLI checks:
  - `7K/8/8/8/8/8/1Q6/k7 b - - 0 1` reports `outcome: loss`.
  - `7k/8/8/8/8/8/2q5/K7 w - - 0 1` reports `outcome: draw`.

## Remaining concerns

Plan 1 intentionally scoped out the larger GHI/transposition issues noted in
`review1.md`:

- **GHI twin simulation** still does not carry the current search prefix into
  `simulate`, so a twin proven along a different path may be incorrectly reused.
- **Path-code encoding** still does not distinguish move type (normal move vs
  queen promotion, en-passant, castling), which can cause path-code collisions.
- `simulate` still accepts an empty move list as valid for `Outcome::Loss`
  without re-checking `pos.outcome()`.
- `solve_refined` remains a linear scan rather than a true binary search.
- `validate_pv` does not verify move legality or that the final outcome matches
  the reported result.

These are tracked for follow-up work and are not addressed by this plan.
