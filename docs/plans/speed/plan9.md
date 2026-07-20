# Plan 9: Incremental Zobrist hash update in `Position`

## Start

Read `docs/plans/speed/analysis.md`.  Inspect `src/position.rs`,
`src/zobrist.rs`, and the `atomic_movegen` `Board`/`StateInfo` API used by the
solver.  Determine whether `Board::hash()` is already maintained incrementally
or recomputed from pieces on every call.

## Goal

Avoid recomputing the full position hash on every `do_move` and `undo_move`.

## Background

`Position::do_move` and `Position::undo_move` currently call:

```rust
self.zobrist = zobrist::hash(&self.board, self.board.rule50());
```

where

```rust
pub fn hash(board: &Board, rule50: u16) -> u64 {
    board.hash() ^ z.rule50_keys[rule50.min(100) as usize]
}
```

<ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="67-78" /> <ref_snippet file="/workspace/atomic_solver/src/zobrist.rs" lines="101-115" />

`do_move` and `undo_move` are called for every child evaluation and every child
expansion, so `board.hash()` is on the hottest path.  If `Board::hash()` is
recomputed from pieces, maintaining the hash incrementally would be a large win.
If it is already incremental, only the rule50 XOR needs to be updated.

## Implementation tasks

1. **Investigate `Board::hash()`**.  Find the `atomic_movegen` source and check
   whether `hash()` reads an internally maintained key or recomputes it.
2. **If `Board::hash()` is incremental:**
   - Only recompute the rule50 Zobrist key when `rule50` changes.
   - Update `Position::do_move`/`undo_move` to do the minimal XOR update.
3. **If `Board::hash()` is not incremental:**
   - Add our own Zobrist piece keys to `src/zobrist.rs` for all piece/square
     combinations, side to move, castling rights and en-passant square.
   - Extend `Position` to update these keys incrementally when a move is made
     or undone, using the `StateInfo` returned by `Board::do_move` to learn which
     pieces were removed by an atomic blast and which pieces moved.
   - Keep `board.hash()` as a fallback and add tests that compare the
     incremental `Position::hash()` with `board.hash() ^ rule50_key` after every
     move in a random game.
4. Regardless of the approach, do not change the external `Position::hash()`
   value; it is used by the transposition table.

## File changes

- `src/position.rs`
- `src/zobrist.rs` (if new piece/side keys are needed)
- `tests/test_position.rs` or a new regression test for hash consistency

## Risks

- `StateInfo` may not expose all pieces removed by an atomic blast, making
  incremental updates from that struct impossible.
- If we maintain our own keys, they must match `Board::hash()` exactly, or we
  must stop using `board.hash()` entirely and reimplement the board hash.  The
  latter is a large change and should only be done if `Board::hash()` proves to
  be expensive.
- Mistakes in incremental hashing silently break transposition-table lookups,
  leading to wrong results or infinite loops.  Thorough property testing is
  essential.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
```

Add a new test that plays random legal games and asserts that
`Position::hash()` equals `zobrist::hash(&pos.board, pos.board.rule50())` after
every make/unmake pair.

## Final task

Write `docs/plans/speed/report9.md` stating whether `Board::hash()` was already
incremental, which strategy was chosen, and the measured speed impact.
