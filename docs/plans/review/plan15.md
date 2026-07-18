# Follow-up Plan: Work Packages after Review 3

Date: 2026-07-17  
Source: `docs/plans/review/review3.md`

This plan derives work packages from the recommendations in `review3.md` and proposes a safe order of implementation. The highest priority is fixing the terminal-detection ordering bug, because it already produces wrong results on simple FENs and propagates into search, PV validation, and GHI simulation.

---

## Proposed order of implementation

1. **Work Package A** — Fix `Position::outcome_from_state` terminal ordering.
2. **Work Package B** — Fix `extract_pv` path-code depth off-by-one.
3. **Work Package C** — Fix `max_depth == 0` cutoff storage.
4. **Work Package D** — Validate and harden shortest-PV refinement with transpositions.
5. **Work Package E** — Strengthen GHI simulation and regression tests.
6. **Work Package F** — CLI/output cleanup and minor API polish.

---

## Work Package A: Fix terminal-detection ordering

### Start

- `review3.md` section 2.1 and the confirmed failing FENs:
  - `7K/8/8/8/8/8/1Q6/k7 b - - 100 1` should be `loss`, currently `draw`.
  - `8/8/8/8/8/8/1K6/k7 b - - 0 1` should be `loss`, currently `draw`.

### Goal

Make `Position::outcome_from_state` evaluate no-legal-moves checkmate/stalemate before the 50-move and two-piece draw heuristics, so checkmate/stalemate always takes precedence.

### Implementation tasks

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
3. Add integration tests in `tests/test_review.rs` (or a new `tests/test_terminal_ordering.rs`) for the failing FENs.
4. Re-run the CLI on the two FENs above and confirm `outcome: loss`.

### File changes

- `src/position.rs`
- `tests/test_review.rs` (or new test file)

### Risks

- Very low. The helper is already used everywhere, so the fix is a single reordering.
- Must ensure the two-piece draw heuristic still fires for genuine two-commoner positions that are not checkmate/stalemate.

### Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo doc --no-deps
$ cargo run --release -- --fen "7K/8/8/8/8/8/1Q6/k7 b - - 100 1"
$ cargo run --release -- --fen "8/8/8/8/8/8/1K6/k7 b - - 0 1"
```

---

## Work Package B: Fix `extract_pv` path-code depth

### Start

- `review3.md` section 2.3.
- `dfpn` computes child path codes with depth `self.path_stack.len()` (1-indexed).
- `extract_pv` uses `pv.len()` (0-indexed).

### Goal

Make `extract_pv` use the same 1-indexed move depth as `dfpn`, so it can follow path-dependent twin entries.

### Implementation tasks

1. In `src/search/dfpn.rs`, change `extract_pv`:
   ```rust
   path_code ^= zobrist::path_random(mv, pv.len() + 1);
   ```
2. Add a unit or integration test that exercises a path-dependent twin through `extract_pv`:
   - Option 1: Make `extract_pv` `#[cfg(test)]` accessible and feed a `Search` with a manually stored twin, then assert the PV is non-empty and valid.
   - Option 2: Add an integration test that solves a position where the win is stored as a twin (because `repetition_seen` is true in the subtree) and asserts the returned PV is non-empty and passes `validate_pv`.
   - Option 3 (simpler): Add a `zobrist` test that builds the path code for a move sequence using `pv.len() + 1` and compares it to the path code built by `dfpn`'s path-code update loop.
3. Ensure all existing PV tests still pass and PV lengths are unchanged for path-independent wins.

### File changes

- `src/search/dfpn.rs`
- `tests/test_position.rs`, `tests/test_review.rs`, or `src/search/dfpn.rs` unit tests

### Risks

- Low. The change is a one-line off-by-one fix, but path codes are subtle.
- A wrong depth index will silently break twin PV retrieval, so a regression test is essential.

### Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo doc --no-deps
$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
```

---

## Work Package C: Fix `max_depth == 0` cutoff storage

### Start

- `review3.md` section 2.2.
- `dfpn` stores `Some(Outcome::Draw)` with `depth = 0` when `max_depth == 0`.
- `try_use_tt` reuses any base `Outcome::Draw` with `entry.depth <= max_depth`.

### Goal

Prevent a depth-bound cutoff from being treated as a proven draw in later searches with a larger remaining depth.

### Implementation tasks

1. Choose the minimal safe fix:
   - **Option 1 (recommended for this package):** In the `max_depth == 0` branch, return `Outcome::Draw` to the parent but do **not** store it in the transposition table. This avoids poisoning without changing the `TtEntry` layout.
   - **Option 2 (if Option 1 causes unacceptable re-search):** Add a `max_depth: u32` or `remaining_depth` field to `TtEntry` and store the remaining depth at which an unsolved/cutoff result is valid. Only reuse an unsolved entry when the current `max_depth` is equal to or less than the stored bound, with a clear policy for how bounds transfer across depths.
2. Implement the chosen fix.
3. Add a regression test in `tests/test_position.rs` or `tests/test_review.rs`:
   - Create a `Search` and call `search_depth` on a simple win with `max_depth = 0` (expect `Draw`).
   - Call `search_depth` again on the same `Search` with a larger `max_depth` (e.g., 3 or 4) and expect `Win` with a non-empty PV.
   - This should fail before the fix and pass after it.
4. If Option 2 is chosen, update `tt.rs` unit tests and the `TtEntry` size test.

### File changes

- `src/search/dfpn.rs` (and possibly `src/search/tt.rs` for Option 2)
- `tests/test_position.rs` or `tests/test_review.rs`

### Risks

- Option 1 is safe but may increase node counts in `search_depth` because cutoff results are not memoized.
- Option 2 is more invasive and may affect `TtEntry` size and `Copy` semantics.
- The fix must not break `solve_refined` binary search, which relies on `dfpn(max_depth)` being meaningful.

### Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo doc --no-deps
$ cargo test --release --test test_review
$ cargo run --example solve_depth_limited --release -- "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" 0
$ cargo run --example solve_depth_limited --release -- "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" 3
```

---

## Work Package D: Validate and harden shortest-PV refinement

### Start

- Completion of Work Package C.
- `review3.md` sections 2.2 and 3.2.

### Goal

Ensure `solve_refined` returns a shortest (or near-shortest) PV on transposition-heavy wins, and document any remaining limitations.

### Implementation tasks

1. After Work Package C, run `solve` with `refine_shortest(true)` on a set of transposition-heavy wins:
   - `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1`
   - `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1`
   - The mate-in-two position from `test_epsilon.rs`
2. For each, record returned PV length and compare to the known shortest length.
3. If PVs are consistently shortest, add regression tests that assert the PV length.
4. If PVs are longer than optimal:
   - Investigate whether the remaining depth-bound TT interaction is the cause.
   - If a small fix (e.g., clearing unsolved entries between `solve_refined` probes or storing remaining depth) resolves it, implement it.
   - If the issue is inherent to the current design, document `solve_refined` as a best-effort refinement and add an `--unrefined` or similar option to `main.rs`.
5. Add a regression test in `tests/test_plan5.rs` or `tests/test_review.rs` that asserts the shortest PV length for the two-rook mate.

### File changes

- `src/search/dfpn.rs` (if changes are needed)
- `tests/test_plan5.rs` or `tests/test_review.rs`
- `docs/plans/review/review3.md` or `AGENTS.md` (to document any limitations)

### Risks

- May require a more invasive TT redesign (remaining-depth field) to be fully correct.
- The existing `solve_refined` may already be good enough in practice; over-engineering should be avoided.

### Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo doc --no-deps
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
```

---

## Work Package E: Strengthen GHI simulation and regression tests

### Start

- Completion of Work Package A (terminal detection) and Work Package B (PV extraction).
- `review3.md` section 3.1.

### Goal

Make cross-path GHI reuse more robust and add regression tests that exercise the current simulation's limitations.

### Implementation tasks

1. Decide on the approach for cross-path twin verification:
   - **Option A (pragmatic):** Keep the current `simulate` but add a bounded fresh `dfpn` fallback in `try_use_tt`: if `simulate` cannot verify a twin (missing child twin or prefix mismatch), run a small `dfpn` from the twin node under the current path with `max_depth = twin.depth` and accept the twin only if the bounded search returns the same `outcome`.
   - **Option B (paper):** Implement the ancestor-set tracking required for full Kawano simulation across different paths.
2. Add regression tests in `tests/test_ghi.rs`:
   - A cross-path twin test where the same board is reached by two move orders and the winning move in one order is a repetition in the other (if such an atomic-chess position can be constructed).
   - A synthetic unit test that directly stores twin entries and calls `try_use_tt` / `simulate` to verify cross-path behavior.
3. If the current simulation is intentionally kept as an approximation, document the limitation in `docs/plans/dfpn/research_ghi.md` and `review3.md`.

### File changes

- `src/search/dfpn.rs`
- `tests/test_ghi.rs`
- `docs/plans/dfpn/research_ghi.md` (if needed)

### Risks

- Cross-path GHI cases are difficult to construct for atomic chess.
- A fresh `dfpn` fallback may be expensive; keep node/depth caps tight.
- Over-tight simulation can reject valid twins and hurt performance.

### Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo doc --no-deps
$ cargo test --release --test test_ghi -- --ignored
$ cargo run --example twin_stats --release
```

---

## Work Package F: CLI and API cleanup

### Start

- All higher-priority packages completed.
- `review3.md` recommendation 7.

### Goal

Remove duplicate CLI output and expose the move-list-aware terminal detector for callers that already have one.

### Implementation tasks

1. In `src/main.rs` and `src/search/dfpn.rs`, avoid printing the final `outcome:`/`pv:` twice. Options:
   - Remove the final `print_pv_update` call in `solve_refined` and let `main.rs` print the final result.
   - Or, have `print_pv_update` only print to `stderr` and suppress the final call when the result is about to be printed by `main.rs`.
2. Confirm `Position::outcome_from_state` is already `pub`; if not, make it `pub` and document it.
3. Update `examples/` that call `outcome()` to use `outcome_from_state` when they already have a `MoveList` and `StateInfo`.
4. Add a small regression test or CLI check that the output is not duplicated.

### File changes

- `src/main.rs`
- `src/search/dfpn.rs`
- `src/position.rs` (if visibility change)

### Risks

- Very low. Purely cosmetic and API polish.

### Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo doc --no-deps
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" | sort | uniq -c
```

---

## Cross-cutting concerns

- **Test coverage:** Each work package must add at least one regression test that fails before the fix and passes after it.
- **TT layout:** Work Packages C and E may require changes to `TtEntry`. If both need layout changes, do them together after Work Package C to avoid churn.
- **Performance:** Work Package C Option 1 may increase node counts. If that is unacceptable, prefer Option 2, but account for the extra `TtEntry` size in `tt_entry_size_is_reasonable`.
- **Documentation:** Update `review3.md` or `AGENTS.md` with any design decisions or remaining limitations (especially for GHI simulation and shortest-PV refinement).

---

## Acceptance criteria for the whole plan

- The two checkmate FENs from `review3.md` report `loss`.
- `extract_pv` returns a non-empty, valid PV for path-dependent twin wins (tested directly or via an integration test).
- `search_depth(0)` followed by `search_depth(3)` on a simple mate finds the win.
- Existing GHI and epsilon regression tests still pass.
- `cargo test --all-targets`, `cargo clippy --all-targets`, and `cargo doc --no-deps` are clean.
