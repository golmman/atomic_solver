# Plan 15: Fix Terminal-Detection Ordering and `extract_pv` Path-Code Depth

## Start

- Read `docs/plans/review/review3.md` sections 2.1 and 2.3.
- Read this file (`docs/plans/review/plan15.md`).
- Confirm the failing FENs from `review3.md` still reproduce before starting.

## Goal

Fix the two highest-priority correctness issues identified in `review3.md`:

1. `Position::outcome_from_state` must evaluate no-legal-moves checkmate/stalemate before the 50-move and two-piece draw heuristics.
2. `extract_pv` must use the same 1-indexed move depth as `dfpn` when recomputing path codes, so it can follow path-dependent twin entries.

Both are small, well-scoped code changes with clear regression tests.

## Background

- `review3.md` section 2.1: `outcome_from_state` currently checks `rule50 >= 100` and `occupied().count() == 2` before the `moves.is_empty()` branch. This misclassifies 50-move checkmates and two-piece checkmates as draws.
- `review3.md` section 2.3: `extract_pv` uses `zobrist::path_random(mv, pv.len())` (0-indexed) while `dfpn` uses `zobrist::path_random(mv, self.path_stack.len())` (1-indexed). Path-dependent twins are keyed by the 1-indexed code, so PV extraction fails for them.

## Implementation tasks

1. In `src/position.rs`, reorder `outcome_from_state`:
   1. Own commoners gone -> `Loss`
   2. Opponent commoners gone -> `Win`
   3. No legal moves and in check -> `Loss`
   4. No legal moves and not in check -> `Draw`
   5. `rule50 >= 100` -> `Draw`
   6. Only two pieces remain -> `Draw`
   7. Otherwise `None`
2. Add unit tests in `src/position.rs`:
   - 50-move checkmate returns `Loss`.
   - Two-piece adjacent-king checkmate returns `Loss`.
   - 50-move stalemate returns `Draw`.
3. Add integration tests for the failing FENs (e.g., in `tests/test_review.rs` or a new `tests/test_terminal_ordering.rs`).
4. In `src/search/dfpn.rs`, fix `extract_pv`:
   ```rust
   path_code ^= zobrist::path_random(mv, pv.len() + 1);
   ```
5. Add a regression test for `extract_pv` path-code depth. Options:
   - Add a `zobrist` test that builds the path code for a move sequence using 1-indexed depths and compares it to the path code built by `dfpn`'s update loop.
   - Add an integration test that solves a position known to produce a path-dependent twin and asserts the returned PV is non-empty and valid.
6. Run the full verification command set below.

## File changes

- `src/position.rs`
- `src/search/dfpn.rs`
- `tests/test_review.rs` or `tests/test_terminal_ordering.rs`
- `src/zobrist.rs` or `src/search/dfpn.rs` unit tests (for path-code regression test)

## Risks

- The terminal-ordering change is low-risk but affects every call site of `outcome()` and `outcome_from_state`.
- The `extract_pv` change is a one-line fix, but path-code arithmetic is subtle; a regression test is essential.
- Existing PV tests should continue to pass with unchanged PVs for path-independent wins.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo doc --no-deps
$ cargo run --release -- --fen "7K/8/8/8/8/8/1Q6/k7 b - - 100 1"
$ cargo run --release -- --fen "8/8/8/8/8/8/1K6/k7 b - - 0 1"
```

Both CLI runs must report `outcome: loss`.

## Final task

Write `docs/plans/review/report15.md` documenting:

- The exact changes made.
- The new regression tests and why they fail before the fix.
- Verification results (`cargo test`, `cargo clippy`, `cargo doc`, CLI output).
- Any remaining concerns or follow-up work.
