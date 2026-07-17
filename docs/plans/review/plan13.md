# Plan 13: Add dedicated GHI regression tests

## Start

- Read `docs/plans/review/report12.md` to confirm the repetition-key separation
  is in place and `cargo test` passes.

## Goal

Create tests that exercise true cross-path reuse and path-dependent outcomes, so
future changes to GHI handling do not silently break it.

## Background

- The existing `try_use_tt` tests only cover the case where the current position
  is already in `self.path`; they do not cover a twin whose proof tree is
  reached through a different move order.
- There are no tests for repetition cycles where the halfmove clock changes, or
  for positions where the correct move at a transposition depends on the path.
- The GHI and repetition fixes in plans 10-12 need regression coverage.

## Implementation tasks

1. Create `tests/test_ghi.rs` (or extend `tests/test_review.rs`) with the
   following cases:
   - **Cross-path twin reuse:** solve a position from one move order that
     creates a twin, then reset and solve the same board from a different move
     order. Assert both runs agree on the outcome and, if decisive, produce a
     PV. Optionally assert that the second run reuses the twin via `twin_stats`
     or a test-only accessor.
   - **Repetition with changing rule50:** build a position with a cycle of
     reversible moves and verify the solver returns `Draw` (or at least does not
     return `Win`) when the cycle is forced.
   - **Path-dependent transposition:** construct a position where the same board
     can be reached by two move orders, and one path allows a winning move that
     the other does not because of repetition rights. Assert the solver reports
     the correct outcome for each starting move order.
2. Add a unit test in `src/search/tt.rs` for `find_result_for_path` and
   `best_result_for_path` when twins from multiple path codes are present.
3. If `simulate` exposes an internal helper or statistics, add a direct unit
   test that a twin stored for path A can be verified for path B when the proof
   tree is path-independent, and rejected when it is path-dependent.
4. Update `tests/test_epsilon.rs` (if present) to run one of the new cyclic
   positions with both `ε = 0.0` and `ε = 0.25`, ensuring the threshold fix does
   not regress GHI-sensitive positions.
5. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
6. Final task: write `docs/plans/review/report13.md` documenting the new
   regression suite, which tests are fast enough for the default suite, which are
   marked `#[ignore]`, and any solver behaviors that remain untested.

## File changes

- `tests/test_ghi.rs` (new, or `tests/test_review.rs`)
- `src/search/tt.rs` (unit tests)
- `tests/test_epsilon.rs` (optional cross-check)

## Risks

- Constructing atomic-chess FENs that exercise exact GHI corner cases is
  difficult. If manual FENs are not available, use `Position::do_move` sequences
  in the tests instead.
- Some GHI positions may be too deep or too cyclic for the default 5-second
  timeout. Mark them `#[ignore]` with a comment and run them in a dedicated CI
  job or release build.
- Tests that assert twin reuse rely on internal statistics; if those are not
  public, expose a minimal test-only accessor or use a wrapper.

## Verification

- `cargo test` passes.
- `cargo test --release -- --ignored` (or `cargo test --ignored`) passes the
  deeper GHI cases.
- `cargo run --release` on the selected FENs returns the expected outcomes.
