# Plan 2 Implementation Report

This report documents the implementation of `docs/plans/review/plan2.md`, which
fixes the Zobrist path-code encoding so that move type is included in the key.

## Changes made

### `src/zobrist.rs`

- Added a private `move_kind` helper that maps a [`Move`] to a move-kind index:
  - `0` = normal move (no promotion, castling, or en passant)
  - `1` = castling
  - `2` = en passant
  - `3..6` = promotion, offset by the index of the promotion piece in
    `PROMOTION_PIECES` (queen = 3, rook = 4, bishop = 5, knight = 6)

  This guarantees that a normal move and a queen promotion with the same
  `from`/`to` squares no longer share a path key, and that castling and
  en-passant moves are distinguished as well.
- Increased `PATH_MOVE_NB` from `64 * 64 * PieceType::NB` to
  `64 * 64 * (PieceType::NB + 3)`, providing 9 move-kind slots.  The largest
  used index is 6 (knight promotion), so the array is comfortably sized.
- Rewrote `Zobrist::path_random` to use `move_kind(mv)` instead of
  `mv.promotion_type() as u8 as usize` when computing the move index.
- Added a `#[cfg(test)]` module with unit tests verifying that the following
  move pairs, when sharing the same `from`/`to` squares, produce different
  path keys:
  - normal move vs queen promotion
  - all four promotion piece types (queen, rook, bishop, knight) vs each other
  - normal move vs castling
  - normal move vs en passant

## Encoding details

The path key is still computed as:

```text
path_move_keys[move_index] ^ path_depth_keys[depth % MAX_PATH_DEPTH]
```

where `move_index = from + to * 64 + kind * 64 * 64`.

`from` and `to` are the 0..63 square indices; `kind` is the 0..6 move-kind code
above.  With `PATH_MOVE_NB = 64 * 64 * (6 + 3) = 36_864`, the largest possible
`move_index` is `63 + 63 * 64 + 6 * 64 * 64 = 28_671`, well within bounds.
The static memory usage for `path_move_keys` is `36_864 * 8 = 294_912` bytes,
about 288 KiB.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
```

All passed:

- `cargo clippy --all-targets` is clean.
- `cargo test` passes all tests, including the 4 new `zobrist::tests`.
- `cargo doc` builds without warnings.

## Key-collision audit

With the new scheme, the `(from, to, kind)` triple is encoded directly into the
index passed to the per-triple random key table.  This is collision-free for
valid moves because:

- `from`, `to`, and `kind` are decomposed into a single flat index using a
  mixed-radix formula (`from + to * 64 + kind * 64 * 64`).
- Each distinct triple maps to a distinct slot in `path_move_keys`.
- The table is sized to cover all possible triples with `kind < 9`.

The remaining risk is the XOR with the per-depth key: two different
`(from, to, kind)` triples at different depths could theoretically collide if
`path_move_keys[i] ^ path_depth_keys[d1] == path_move_keys[j] ^ path_depth_keys[d2]`.
This is a 64-bit random XOR collision and is vanishingly unlikely for the
search depths used.  The encoding itself no longer introduces structural
collisions for moves that differ only by move type.

## Remaining concerns

Plan 2 only changed the path-code encoding.  It does **not** address the larger
GHI/transposition issues from `review1.md`:

- `simulate` still verifies twins against the twin's own path rather than the
  current search prefix.
- `simulate` still accepts an empty move list as valid for `Outcome::Loss`.
- `solve_refined` remains a linear scan and `validate_pv` remains weak.

These remain for follow-up work.
