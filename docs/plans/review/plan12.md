# Plan 12: Separate TT and repetition keys

## Start

- Read `docs/plans/review/report11.md` to confirm the `simulate` changes are
  stable before changing the key used for repetition detection.

## Goal

Use a board-only Zobrist key (pieces, side, castling, en passant) for in-memory
repetition detection, while keeping the rule50-inclusive key for the
transposition table.

## Background

- `Position::hash()` currently returns the full key that includes the halfmove
  clock.
- `self.path` and `self.path_stack` in `src/search/dfpn.rs` use this same key,
  so a repeated board position with a different `rule50` counter is not
  detected as a repetition.
- The TT should keep `rule50` because the 50-move draw is path-dependent, but the
  repetition set must ignore it.

## Implementation tasks

1. Add `pub fn board_hash(board: &Board) -> u64` to `src/zobrist.rs` that
   returns `board.hash()` (the static board-only component). If `board.hash()`
   unexpectedly includes `rule50`, document the need to mask it out.
2. Add `pub fn repetition_key(&self) -> u64` to `Position` in
   `src/position.rs`, returning `self.board.hash()`. Keep `pub fn hash(&self) -> u64`
   returning the full `self.zobrist` for TT use.
3. In `src/search/dfpn.rs`:
   - Compute `tt_key = pos.hash()` and `rep_key = pos.repetition_key()` at the
     start of `dfpn`.
   - Use `tt_key` for all `self.tt.probe` and `self.tt.store` calls.
   - Use `rep_key` for `self.path.insert/remove` and store `rep_key` in
     `self.path_stack` (the stack length is still used for path-code depth
     indexing).
4. In `simulate`:
   - Use `rep_key` for `sim_path` insertion/removal and the
     `SIM_MAX_DEPTH`/`sim_stack.len()` depth check.
   - Use `tt_key = pos.hash()` for `self.tt.probe` child lookups.
5. In `extract_pv`:
   - Use `tt_key = current.hash()` for TT probe.
   - Use `rep_key = current.repetition_key()` for the `seen` cycle-detection
     set, so a board repeated with a higher `rule50` does not create an
     infinite-looking PV.
6. Add tests:
   - In `src/zobrist.rs` or `src/position.rs`, assert that two positions with
     the same board but different `rule50` have equal `repetition_key()` and
     different `hash()`.
   - In `tests/test_review.rs` or a new `tests/test_repetition.rs`, create a
     reversible move cycle and verify the solver returns `Draw` by repetition
     (or at least does not incorrectly claim `Win`).
7. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
8. Final task: write `docs/plans/review/report12.md` documenting the key
   separation, the new `Position` methods, test cases, and any observed change
   in solver behavior on cyclic positions.

## File changes

- `src/zobrist.rs`
- `src/position.rs`
- `src/search/dfpn.rs`

## Risks

- `board.hash()` must not include `rule50`; if it does, the repetition key is
  still wrong and needs further masking. Verify with a `rule50` delta test.
- Using `rep_key` in `path_stack` does not affect path-code depth indexing
  (which uses stack length), but any code comparing `path_stack` contents to
  `path` keys must be checked; currently there are none.
- `extract_pv` cycle detection now breaks on true board repetitions, which may
  shorten PVs in cyclic lines but prevents infinite extraction.

## Verification

- New `repetition_key` unit test passes.
- The reversible-cycle integration test returns `draw` or terminates without
  claiming a win.
- `cargo test` and `cargo clippy --all-targets` are clean.
