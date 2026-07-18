# Plan 17: Harden Shortest-PV Refinement, GHI Simulation, and Cleanup

## Start

- Read `docs/plans/review/report16.md` to confirm Plan 16 is stable and the test suite is green.
- Read `docs/plans/review/review3.md` sections 2.2, 3.1, 3.2, and recommendation 7.
- Read this file (`docs/plans/review/plan17.md`).

## Goal

Complete the remaining follow-up work from `review3.md`:

1. Validate and harden `solve_refined` shortest-PV behavior on transposition-heavy wins.
2. Strengthen GHI simulation or add regression tests that document its limitations.
3. Clean up duplicate CLI output and polish the public API.

## Background

- `solve_refined` uses a binary search over `[1, best_depth]`. After Plan 16, the `max_depth == 0` cutoff bug is fixed, but the interaction between depth-bounded search and transpositions may still produce non-minimal PVs.
- The GHI `simulate` function follows the stored proof tree using the twin's original `path_code` and `path_length`, seeded with the current search prefix. This is sound for finite proof trees and for cycles that are also present in the current prefix, but it does not fully handle the cross-path cyclic case from `research_ghi.md`.
- `main.rs` and `Search::print_pv_update` both print `outcome:` and `pv:`, causing duplicated final output.

## Implementation tasks

### Part 1: Shortest-PV refinement

1. Run `solve` with `refine_shortest(true)` on transposition-heavy wins:
   - `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1`
   - `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1`
   - The mate-in-two position from `tests/test_epsilon.rs`
2. For each, record returned PV length and compare to the known shortest length.
3. Add regression tests that assert the returned PV length for these positions.
4. If PVs are longer than optimal:
   - Investigate whether the remaining depth-bound TT interaction is the cause.
   - If a small fix resolves it (e.g., storing remaining depth with unsolved entries or disabling cross-bound reuse), implement it in `src/search/dfpn.rs` and `src/search/tt.rs`.
   - If the issue is inherent to the current design, document `solve_refined` as a best-effort refinement and add a note to `review3.md` or `AGENTS.md`.

### Part 2: GHI simulation

1. Decide on the approach for cross-path twin verification:
   - **Option A (pragmatic):** In `try_use_tt`, if `simulate` cannot verify a twin from another path (missing child twin or prefix mismatch), run a bounded fresh `dfpn` from the twin node under the current path with `max_depth = twin.depth`. Accept the twin only if the bounded search returns the same `outcome`.
   - **Option B (paper):** Implement the ancestor-set tracking required for full Kawano simulation across different paths.
2. Add regression tests in `tests/test_ghi.rs`:
   - A cross-path twin test where the same board is reached by two move orders and the available winning move depends on repetition rights (if such an atomic-chess position can be constructed).
   - A synthetic unit test that directly stores twin entries and calls `try_use_tt` / `simulate` to verify cross-path behavior.
3. If the current simulation is intentionally kept as an approximation, document the limitation in `docs/plans/dfpn/research_ghi.md` and `review3.md`.

### Part 3: Cleanup

1. Remove duplicate final `outcome:`/`pv:` output. Options:
   - Remove the final `print_pv_update` call in `solve_refined` and let `main.rs` print the final result.
   - Or, suppress the final `print_pv_update` when the result will be printed by the caller.
2. Confirm `Position::outcome_from_state` is `pub` and update `examples/` to use it when they already have a `MoveList` and `StateInfo`.
3. Add a CLI check that the output is not duplicated.

## File changes

- `src/search/dfpn.rs` (refinement, GHI simulation)
- `src/search/tt.rs` (if TT layout changes are needed)
- `src/main.rs` (CLI output cleanup)
- `src/position.rs` (visibility/API polish)
- `tests/test_plan5.rs` or `tests/test_review.rs` (shortest-PV tests)
- `tests/test_ghi.rs` (cross-path GHI tests)
- `docs/plans/dfpn/research_ghi.md` (if documenting limitations)

## Risks

- Shortest-PV refinement may require a more invasive TT redesign to be fully correct on transpositions.
- Cross-path GHI cases are difficult to construct for atomic chess; the regression tests may need to be synthetic.
- A fresh `dfpn` fallback in GHI simulation can be expensive; keep node and depth caps tight.
- Cleanup is low-risk but should be done last so it does not obscure functional changes.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo doc --no-deps
$ cargo test --release --test test_ghi -- --ignored
$ cargo test --release --test test_epsilon
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
```

The CLI runs must produce a single `outcome:`/`pv:` block each.

## Final task

Write `docs/plans/review/report17.md` documenting:

- The changes made for shortest-PV refinement, GHI simulation, and cleanup.
- The new regression tests and what they cover.
- Verification results (`cargo test`, `cargo clippy`, `cargo doc`, CLI output).
- Any remaining limitations and recommendations for future work.
