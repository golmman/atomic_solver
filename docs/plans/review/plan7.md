# Plan 7: Add regression tests for review findings

## Start

- Read `docs/plans/review/report6.md` to confirm PV validation is stable and
  note any blockers before adding regression tests.

## Goal

Create a regression suite that covers the correctness gaps identified in the
review: single-commoner checkmate vs stalemate, transpositions with different
move types, promotion path-code collisions, and repeated-state twin reuse.

## Background

`cargo test` currently passes 49 tests, but none cover the issues from the
review.

## Implementation tasks

1. Create a new file `tests/test_review.rs` or extend `tests/test_inf.rs` with
   cases for:
   - Single-commoner checkmate:
     `7K/8/8/8/8/8/1Q6/k7 b - - 0 1` and a few variants with different line
     pieces.
   - Stalemate with no commoner under attack: a FEN where the side to move has
     no legal moves and is not in check; expected `Draw`.
   - Transpositions with different move types/promotions: positions where the
     same board can be reached by a normal move and by a queen promotion, and
     the solver returns the correct outcome for both paths.
   - Repeated states: positions with forced repetitions or cyclic defenses
     that stress `twins` and `simulate`.
2. Add `Position`-level unit tests in `src/position.rs` for `outcome` ordering
   and no-legal-move classification.
3. Add `zobrist` unit tests for path-code uniqueness (already partly in plan 2;
   expand here if needed).
4. For each test, assert both the `Outcome` and, where decisive, a non-empty PV.
5. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
6. Final task: write `docs/plans/review/report7.md` documenting the new
   regression suite and any cases that remain too slow for the default
   5-second timeout.

## File changes

- `tests/test_review.rs` (new or `tests/test_inf.rs`)
- `src/position.rs` (unit tests)
- `src/zobrist.rs` (tests, if needed)

## Risks

- Some regression positions may be too deep for the default 5-second timeout.
  For those, either increase the timeout in the test or mark them `#[ignore]`
  with a comment explaining they are stress tests.
- Constructing stalemate and transposition FENs for atomic chess requires care;
  validate with `Board::outcome()` or manual analysis.

## Verification

- `cargo test` passes (including new tests).
- The checkmate FEN no longer returns `Draw`.
