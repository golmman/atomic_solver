# Plan 16: Fix `max_depth == 0` Cutoff Storage

## Start

- Read `docs/plans/review/report15.md` to confirm Plan 15 is stable and the test suite is green.
- Read `docs/plans/review/review3.md` section 2.2 for the full background.
- Read this file (`docs/plans/review/plan16.md`).

## Goal

Prevent `dfpn` from storing a depth-bound cutoff as a *proven* `Outcome::Draw` in the transposition table. A node that is not solved within the remaining depth budget must not be reused as a definite draw when the same node is reached later with a larger remaining depth.

## Background

When `dfpn` is called with `max_depth == 0` at a non-terminal node, it stores `Some(Outcome::Draw)` with `depth = 0`. `try_use_tt` then accepts any base `Outcome::Draw` whose `entry.depth <= max_depth`, so the cutoff is treated as a proven draw for all later searches with `max_depth > 0`. This is unsound for:

- `search_depth` called with increasing depth bounds on the same `Search`.
- `solve_refined`, where the same node can be reached via paths of different lengths within one depth-bounded probe.

## Implementation tasks

1. Choose the fix:
   - **Option 1 (recommended):** In the `max_depth == 0` branch in `src/search/dfpn.rs`, simply return `Outcome::Draw` to the parent without storing it in the TT. This is safe and minimally invasive.
   - **Option 2:** If Option 1 causes unacceptable re-search, add a `max_depth` or `remaining_depth` field to `TtEntry` and store the depth budget for which an unsolved/cutoff result is valid. Only reuse an unsolved entry when the current remaining depth does not exceed the stored budget. This affects `src/search/tt.rs` and may require updating the `TtEntry` size test.
2. Implement the chosen fix.
3. Add a regression test:
   - Create a `Search` and call `search_depth` on a simple winning position with `max_depth = 0` (expect `Draw`).
   - Call `search_depth` again on the same `Search` with `max_depth = 3` (or another small bound that is sufficient to find the win) and expect `Win` with a non-empty PV.
   - This test should fail before the fix and pass after it.
4. If Option 2 is chosen, update `src/search/tt.rs` unit tests and verify `TtEntry` size remains reasonable.
5. Run the full verification command set below.

## File changes

- `src/search/dfpn.rs`
- `src/search/tt.rs` (only if Option 2 is chosen)
- `tests/test_position.rs` or `tests/test_review.rs` (regression test)

## Risks

- Option 1 is safe but may increase node counts because cutoff results are not memoized.
- Option 2 is more invasive and may affect `TtEntry` size and `Copy` semantics.
- The fix must not break `solve_refined`, which relies on `dfpn(max_depth)` returning meaningful results.
- Be careful not to remove storage for genuine terminal draws; only the `max_depth == 0` cutoff branch should change.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo doc --no-deps
$ cargo run --example solve_depth_limited --release -- "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" 0
$ cargo run --example solve_depth_limited --release -- "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" 3
```

The second `solve_depth_limited` run must report a win.

## Final task

Write `docs/plans/review/report16.md` documenting:

- The chosen fix and why it was chosen.
- The new regression test and how it fails before the fix.
- Verification results (`cargo test`, `cargo clippy`, `cargo doc`, example output).
- Any performance observations and remaining concerns.
