# Research: possible improvements to `atomic-movegen` for `atomic_solver`

This note collects feedback from integrating `atomic-movegen = "1.0.0"` into the `atomic_solver` project. It is based on the published source in `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/atomic-movegen-1.0.0/` and on the current `Position` wrapper in `src/position.rs`.

## 1. The two questions asked

### 1.1 Is `atomic_movegen::attacks::init()` required and does it affect performance?

**No.** `attacks::init()` is a no-op in version 1.0.0.

`src/attacks.rs` calls `crate::magic::init()`:

```rust
pub fn init() {
    crate::magic::init();
}
```

`src/magic.rs` defines `init()` as an empty function:

```rust
pub(crate) fn init() {}
```

All attack tables (`KING_ATTACKS`, `KNIGHT_ATTACKS`, `PAWN_ATTACKS`, `BETWEEN_BB`, and the rook/bishop magic tables) are either `const` arrays or `static` arrays initialized at compile time. I verified this by running `generate_legal()` on the starting position without calling `attacks::init()`; it returns the expected 20 moves.

**Recommendation:** remove the public `init()` function, or mark it `#[deprecated]` and make it a no-op. The current API is misleading because example code and tests call it, but it does nothing and is not required for correctness or performance.

### 1.2 Should `atomic-movegen` track `rule50`?

**It already tracks `rule50`; it just does not expose it.**

`Board` has a private `rule50` field, updates it inside `do_move`, restores it in `undo_move`, and serializes it in `fen()`:

- `src/board.rs` line 157: `rule50: u16`
- `src/board.rs` lines 360–366: parsed from FEN
- `src/board.rs` line 449: written by `fen()`
- `src/board.rs` lines 681 & 826: saved and updated in `do_move`
- `src/board.rs` line 842: restored in `undo_move`

In `src/position.rs` we currently re-parse `rule50` from the FEN string and maintain our own `rule50` field because `Board` has no getter. This is duplicated logic.

**Recommendation:** expose `Board::rule50()` and `Board::game_ply()` (or `Board::halfmove_clock()` / `Board::fullmove_number()`). Then `Position` can delete its own `rule50` field and use `self.board.rule50()` instead. The `StateInfo` already carries the previous `rule50`, so undo semantics remain unchanged.

## 2. Other concrete improvements

### 2.1 Incremental Zobrist hash (largest performance win)

`Position` currently recomputes the Zobrist key from scratch after every `do_move` and `undo_move`:

```rust
// src/position.rs
self.zobrist = zobrist::hash(&self.board);
```

`zobrist::hash()` iterates over every occupied square and XORs piece/castling/ep/side keys, so it is `O(pieces)` per make/unmake. In a search that makes/unmakes millions of nodes this is expensive.

**Recommendation:** let `Board` maintain an incrementally updated `u64` hash and expose `Board::hash()`. The `StateInfo` can store the old hash so `undo_move` can restore it with a single assignment. The hash should include pieces, side, castling, and en-passant, but probably not `rule50` (we deliberately excluded `rule50` from `Position::hash()` because it makes the transposition table ineffective for repeated positions).

This would let `Position` drop its `zobrist` field and `zobrist::hash` recomputation entirely.

### 2.2 `generate_legal` recomputes `StateInfo` on every call

`src/movegen.rs` `generate_legal()` creates a fresh `StateInfo` and calls `board.populate_state()` for every call:

```rust
pub fn generate_legal(board: &Board, moves: &mut MoveList) {
    let mut state = StateInfo::new();
    board.populate_state(&mut state);
    ...
}
```

`populate_state()` computes `checkers`, `pinned`, and both commoner counts from scratch. In the search loop every node calls `pos.legal_moves()` (which calls `generate_legal`), so this work is repeated for every position. `do_move`/`undo_move` do not update a cached `StateInfo`.

**Recommendation:** add a `generate_legal_with_state(board: &Board, state: &StateInfo, moves: &mut MoveList)` variant that uses a caller-provided `StateInfo`. Longer term, `Board` could maintain an internal `StateInfo` and incrementally update it, although atomic blast effects make that more involved than in normal chess. Exposing `is_move_trivially_legal` (currently `pub(crate)`) is another small help.

### 2.3 `Move` and `MoveList` ergonomics

- `MoveList` has no `clear()` method and `set_len()` is `pub(crate)`. For search code that wants to reuse a `MoveList`, a `clear()` would be convenient.
- `Move` could expose simple helpers such as `is_castling()`, `is_promotion()`, `is_en_passant()`, and `Move::to_uci()` (the library encodes castling moves with `to` as the rook square, so a standard `e1g1`/`e1c1` UCI string is not trivial to get right). `Board::is_capture(m: Move)` would also be useful, since the current `Position` and `ordering.rs` duplicate the `is_capture` logic.

### 2.4 `game_ply` / FEN full-move counter semantics

`Board::do_move()` increments `game_ply` by one every move:

```rust
self.game_ply += 1;
```

`Board::fen()` then writes `game_ply` as if it were the FEN full-move counter. This is not FEN-compliant: the full-move counter increases only after Black's move. After a single White move the FEN should still read fullmove `1`, but `Board` would emit `2`.

**Recommendation:** rename or fix the field: either store true plies and compute `fullmove_number = (game_ply + 1) / 2` in `fen()`, or store the full-move counter and only increment it after Black's move.

### 2.5 `Piece::color()` on `NO_PIECE`

`Piece::color()` returns `Color::White` for `NO_PIECE` in release builds, even though `NO_PIECE` has no color:

```rust
pub fn color(self) -> Color {
    debug_assert!(self.0 != 0, "Piece::color called on NO_PIECE");
    if self.0 & 8 == 0 {
        Color::White
    } else {
        Color::Black
    }
}
```

`Piece::type_of()` panics on `NO_PIECE` in debug and, in release, causes an out-of-bounds access. This is a footgun for callers of `Board::piece_on()` who must remember to check `NO_PIECE` before calling `color()` or `type_of()`.

**Recommendation:** make `Piece::color()` and `Piece::type_of()` return `Option<Color>` / `Option<PieceType>`, or document that they panic on `NO_PIECE` in all build modes and make `type_of()` panic consistently.

### 2.6 `Board::legal()` and the "kings touching" rule

I initially misread this part of `Board::legal()`. In standard atomic chess, kings (commoners) **are allowed** to be on adjacent squares. When they touch, neither can be captured by the other (because the capture would destroy the capturing side's own commoner), and the touching commoner is effectively immune from being checked by other pieces as well.

`Board::legal()` in `src/board.rs` lines 1041–1052 implements this correctly:

```rust
let adjacent_enemy = them_commoners & attacks::king_attacks(ksq);
if adjacent_enemy.is_empty()
    && attackers_to(self, ksq, occupied, enemy_survivors) != Bitboard::EMPTY
{
    return false;
}
```

If an enemy commoner is adjacent to the new commoner square, the function does *not* return `false`, so the move is allowed. The `attackers_to()` check only runs when no enemy commoner is adjacent, which is exactly the right atomic behavior: a touching commoner is immune from other attacks, while a non-touching commoner must not be left under attack.

I had previously tested

```rust
let fen = "8/8/8/8/3k4/4K3/8/8 w - - 0 1";
```

and incorrectly expected `e3d3` and `e3e4` to be illegal. Those moves are actually legal in atomic because the white commoner on `e3` is allowed to move next to the black commoner on `d4`. The `generate_legal()` output matches that interpretation.

Two issues remain, but they are not correctness bugs:

- The documentation in `lib.rs` and the doc comment for `Board::legal()` states that the last commoner "cannot move next to an enemy commoner". That is misleading for standard atomic rules and should be corrected.
- `compute_checkers()` still labels an adjacent enemy commoner as a "checker" (see `src/board.rs` lines 599–607), which forces `is_move_trivially_legal()` to bail out to the full `legal()` check. This is a minor performance cost, not a correctness problem.

### 2.7 `Board::outcome()` / terminal helper

`Position` implements its own terminal detection by checking `rule50 >= 100` and `commoners` counts. Since `Board` already tracks `rule50` and `commoners`, the library could expose a `Board::outcome()` or `Board::is_terminal()` helper. This would remove the duplicate `Outcome` logic in `Position` and make `Board` more useful for searchers.

## 3. Recommendations in priority order

1. **Add an incremental Zobrist `Board::hash()`** — the biggest performance win for the solver.
2. **Expose `Board::rule50()` and `Board::game_ply()`** — removes duplication in `Position`.
3. **Provide `Move`/`MoveList` helper methods** (`clear`, `to_uci`, `is_capture` via `Board`, `is_castling`, etc.).
4. **Deprecate or remove `attacks::init()`** — it is a no-op and misleading.
5. **Fix the documentation for pseudo-royal adjacency** — the library code correctly allows kings to touch, but the doc comments say the opposite.
6. **Fix `Piece` `NO_PIECE` handling and FEN `game_ply` semantics** — lower priority but worth doing for a cleaner public API.
