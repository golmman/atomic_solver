# Update report: `atomic-movegen` 2.0.0

## Summary

Updated the solver's dependency from `atomic-movegen = "1.0.0"` to the latest published release, `atomic-movegen = "2.0.0"` (12 July 2026, crates.io). The maintainer integrated the feedback in `plans/movegen/feedback.md` into this release. This report lists which suggestions were adopted, how the solver was adapted, and the verification results.

## Feedback integration

| # | Feedback | Status in 2.0.0 |
|---|----------|-----------------|
| 1 | **Incremental Zobrist `Board::hash()`** | Implemented. `Board` now stores a `u64` hash, updates it incrementally on `do_move`/`move_piece`/`remove_piece`/`place_piece`, and restores it on `undo_move` via `state.hash`. `Board::hash()` is public, excludes `rule50`, and returns `u64`. |
| 2 | **`Board::rule50()` and `Board::game_ply()` getters** | Implemented. `Board` exposes `rule50()`, `game_ply()`, and a new `fullmove_number()` helper. `Board::fen()` now outputs the correct full-move counter. |
| 3 | **`generate_legal_with_state`** | Implemented. `movegen::generate_legal_with_state(board, state, moves)` is public. `StateInfo` now includes `commoners_count`, `them_commoners_count`, `checkers`, `pinned`, and `hash`. `generate_legal()` itself still populates a fresh `StateInfo` internally and delegates to the `with_state` variant. |
| 4 | **`Move` / `MoveList` helpers** | Implemented. `Move` gained `is_castling()`, `is_promotion()`, `is_en_passant()`, and `to_uci()`. `MoveList` gained `clear()`. `Board::is_capture(m: Move)` was added. |
| 5 | **`Board::outcome()` / `Board::is_terminal()`** | Implemented. `Board::outcome()` returns `Option<Outcome>` from the side-to-move perspective and considers commoner extinction, the 50-move rule, and stalemate. `is_terminal()` is a thin wrapper. |
| 6 | **Deprecate or remove `attacks::init()`** | Implemented. `attacks::init()` was removed; attack tables are precomputed at compile time. |
| 7 | **Documentation correction for pseudo-royal adjacency** | Implemented. The `Board::legal()` docs now state that touching an enemy commoner is allowed and does not count as an attack. |
| 8.1 | **FEN full-move counter** | Fixed. `Board::fen()` writes `fullmove_number()` and a `fullmove_number()` getter is exposed. |
| 8.2 | **`Piece::color()` / `Piece::type_of()` safety** | Changed. Both now return `Option<Color>` and `Option<PieceType>`, returning `None` for `NO_PIECE`. |

## Code changes in `atomic_solver`

- `Cargo.toml` / `Cargo.lock`: bumped `atomic-movegen` to `2.0.0`.
- `src/main.rs`: removed the no-op `attacks::init()` call.
- `src/position.rs`:
  - Removed the duplicated `rule50` field; the solver now uses `Board::rule50()`.
  - `do_move` / `undo_move` now rely on the board's own `rule50` tracking and use `zobrist::hash(&board, board.rule50())`.
  - Replaced manual capture detection with `Board::is_capture(m)`.
- `src/zobrist.rs`:
  - Removed the local piece/castling/en-passant Zobrist recomputation.
  - `zobrist::hash()` is now `board.hash() ^ rule50_key`, leveraging the incremental hash from `atomic-movegen` while still mixing in `rule50` for transposition-table correctness.
- `src/notation.rs`: replaced the custom UCI formatter with `Move::to_uci()`.
- `src/search/ordering.rs`:
  - Updated to handle `Piece::color()` / `Piece::type_of()` returning `Option`.
  - Replaced manual `is_capture` and `MoveType` checks with `Board::is_capture(m)`, `Move::is_promotion()`, and `Move::is_en_passant()`.

## Verification

All project quality checks pass after the update:

```text
$ cargo check
    Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo clippy
    Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo test
    All unit/integration tests pass (including solver plan tests and position tests).

$ cargo doc --no-deps
    Documentation generated successfully.
```

## Notes

- `Board::hash()` intentionally excludes `rule50`, which is the correct behavior for a transposition key. The solver's `zobrist::hash()` mixes the board hash with a `rule50` key, so positions with different half-move clocks are still distinct in the transposition table.
- `Board::outcome()` is available and would provide a built-in terminal check, but the hot `Position::outcome()` path in the DFPN search still uses the lighter commoner/`rule50`/two-piece check to avoid the extra `generate_legal()` call inside `Board::outcome()`. The `Board::outcome()` method is available for any future use or for testing.
- `generate_legal_with_state` is not used directly yet; the current `Position::legal_moves()` continues to call `generate_legal`, which now internally uses the pre-populated `StateInfo` path. The `with_state` variant can be adopted later if the solver caches a `StateInfo` across the search loop.

## Conclusion

`atomic-movegen` 2.0.0 addresses all the prioritized feedback items. The update removes boilerplate, uses the incremental Zobrist hash, and makes the public API safer (`Option` piece accessors) and more complete. The solver compiles cleanly, passes `clippy`, and all tests pass.
