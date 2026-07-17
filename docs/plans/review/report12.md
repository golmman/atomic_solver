# Plan 12 Implementation Report

This report documents the implementation of `docs/plans/review/plan12.md`, which
separates the transposition-table key from the repetition-detection key so that
positions with the same board but different `rule50` counters are correctly
recognised as repetitions.

## Changes made

### `src/zobrist.rs`

- Added `pub fn board_hash(board: &Board) -> u64` returning the board-only
  component (`board.hash()`). `zobrist::hash` still XORs the rule50 key on top
  of this for TT use.

### `src/position.rs`

- Added `pub fn repetition_key(&self) -> u64` returning the board-only
  `zobrist::board_hash(&self.board)`. `hash()` continues to return the full
  rule50-inclusive Zobrist key for the transposition table.

### `src/search/dfpn.rs`

- `dfpn` now computes two keys at entry:
  - `tt_key = pos.hash()` for all `self.tt.probe` and `self.tt.store` calls.
  - `rep_key = pos.repetition_key()` for `self.path.insert`,
    `self.path_stack.push`, and `self.path.remove`.
- `evaluate_child` uses `child_key = pos.hash()` for the TT and
  `child_rep_key = pos.repetition_key()` when checking `self.path` for a
  repeated board.
- `simulate` uses `rep_key` for `sim_path` / `sim_stack` and the child's
  `tt_key` for transposition-table probes.
- `extract_pv` uses `tt_key` for the TT and `rep_key` for the `seen` cycle set,
  so a board repeated with a higher `rule50` does not create an infinite PV.

### Tests

- `src/position.rs`: added `repetition_key_ignores_rule50`, which asserts that
  two positions with the same board but `rule50` 0 and 25 have equal
  `repetition_key()` and different `hash()`.
- `tests/test_repetition.rs`: new integration test file.
  - `rook_alone_does_not_claim_win_against_safe_king`: a rook cannot win
    against a lone king that has a 2x2 safe area, so the solver must not report
    `Outcome::Win`.
  - `reversible_cycle_keeps_repetition_key_and_stays_draw`: performs a
    reversible rook/king shuffle that returns to the same board with a higher
    `rule50`, asserts `repetition_key` is unchanged and `hash` is changed, and
    verifies the solver still does not claim a win from the repeated position.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
$ cargo test --release
```

All commands completed successfully.

## Behavioural notes

- The TT continues to use the full rule50-inclusive key, preserving the
  path-dependent 50-move draw behaviour.
- Repetition detection now ignores `rule50`, matching the rule that a position
  reached by reversible moves is a repetition regardless of how many reversible
  moves have been played.
- `extract_pv` now breaks PV extraction on true board repetitions, which may
  shorten PVs in cyclic lines but prevents runaway extraction.

## Remaining limitations

- The solver relies on `board.hash()` being board-only; if the underlying
  `atomic_movegen` representation ever folds `rule50` into `Board::hash`,
  `repetition_key` will need an explicit mask.
- The cyclic integration tests hit the 5-second timeout for the large search
  space, so they assert only that the solver does not incorrectly claim a win.
