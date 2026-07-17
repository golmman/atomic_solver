# Plan 2: Include move type in the Zobrist path code

## Start

- Read `docs/plans/review/report1.md` to confirm that the terminal detection
  fixes landed and to note any blockers before changing the path encoding.

## Goal

Modify `zobrist::path_random` so that the path key encodes the full move type:
normal, castling, en passant, and promotion (with distinct promotion pieces).
Normal moves must not share a key with queen promotions.

## Background

`path_random` currently reads `mv.promotion_type()` and builds
`from + to * 64 + promotion * 64 * 64`. For normal moves `Move::make_move`
leaves promotion bits as `0`, which is interpreted as the first promotion piece
(queen), so a normal move and a queen-promotion move with the same from/to
squares collide. Castling and en passant are also not distinguished.

## Implementation tasks

1. Decide on a move-type index scheme. Example:
   - `0` = normal move (no promotion, no castling, no en passant)
   - `1` = castling
   - `2` = en passant
   - `3..` = promotion, offset by promotion piece index + 3

   This ensures normal moves and queen promotions have different indices.
2. Update `src/zobrist.rs`:
   - Increase `PATH_MOVE_NB` to `64 * 64 * (PieceType::NB + 3)` (or a fixed
     larger constant) so every `(from, to, move_kind)` triple has a unique key.
   - Rewrite `Zobrist::path_random` to compute the index from `from`, `to`, and
     the move kind, using `mv.is_castling()`, `mv.is_en_passant()`,
     `mv.is_promotion()`, and `mv.promotion_type()`.
3. Make sure `path_random` still XORs with a per-depth key. The depth index
   remains `depth % MAX_PATH_DEPTH`.
4. Add a unit test in `src/zobrist.rs` that verifies normal vs queen-promotion,
   castling, and en passant moves with the same from/to produce different path
   keys.
5. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
6. Final task: write `docs/plans/review/report2.md` summarizing the new
   encoding, the test coverage, and any key-collision audit.

## File changes

- `src/zobrist.rs`

## Risks

- Castling and en passant are rare in atomic chess but the path key must still be
  correct if they occur.
- Changing `PATH_MOVE_NB` increases static memory; keep it bounded (a few
  hundred KB is fine).

## Verification

- Unit tests for `path_random` collisions.
- `cargo test` passes.
