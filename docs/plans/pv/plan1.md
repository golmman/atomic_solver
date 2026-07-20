# Plan: Shortest-PV Fix for the DF-PN+ Solver

## Summary

This plan implements the fixes identified in `docs/plans/pv/analysis.md` for the reported FEN

```text
6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26
```

The solver currently returns an 11-plies PV that is suboptimal in two ways:

1. The overall win can be shortened from 11 to 7 half-moves.
2. The PV shows Black's weakest defense (`1...Kh7`) and White's slowest reply (`2.Rh8`); it should show Black's best defense (`1...Kf7`) and, after `1...Kh7`, the fast `2.Rg8!`.

The plan changes the `dfpn` search loop to keep looking for shorter winning children at OR nodes, propagates mate distance in the transposition table, replaces the binary refinement with an iterative-deepening search, and improves the static scorer for atomic restriction/check moves.

## Goal

1. Make `cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"` return a 7-plies PV starting with `b1b8 g8f7`.
2. Ensure that after `1.Rb8+ Kh7` the solver finds the 3-plies continuation `b8g8 c5c4 g8g6`.
3. Keep all existing tests passing.

## Non-goals

- Parallel search.
- Changes to `atomic-movegen` rules or move generation.
- A complete redesign of the transposition table (the existing base/twin design stays; only `best_move` selection and depth propagation change).

## Background

`docs/plans/pv/analysis.md` established:

- The `dfpn` loop in `src/search/dfpn/core.rs` breaks as soon as `select_children` returns a `Win` child, so the first proven winning line is stored as the `best_move` regardless of whether a shorter win exists.
- `is_solved_by_children` in `src/search/dfpn/selection.rs` already computes `parent_win_depth`/`parent_loss_depth`, but the early exit in `core.rs` means the selection often sees only the first solved child.
- `StaticAtomicScorer` in `src/search/ordering.rs` scores `b8h8` (`10650`) far above `b8g8` (`650`) because it only rewards direct attacks on the enemy commoner's square, not attacks on the squares adjacent to the commoner that create atomic check.
- `Search::solve_refined` in `src/search/dfpn/mod.rs` runs an unbounded search and then binary-searches `[1, best_depth]`. The midpoint `max_depth=6` is a Draw and requires an exhaustive 5-second search, so the default timeout is exhausted before `max_depth=7` (where the short win lives) is tried.

## Data structures

No new top-level data structures are required. The existing fields in `TtEntry`, `ChildInfo`, and `ChildSelection` already carry `depth` values. The following local additions are needed inside `dfpn`:

- A running `(best_win_move, best_win_depth)` pair for OR nodes.
- A running `(best_loss_move, best_loss_depth)` pair for AND nodes (or reuse `outcome_to_store_*` fields).

## Algorithm changes

### 1. Depth-aware `dfpn` loop

In `src/search/dfpn/core.rs`, change the main loop:

```rust
// Before
if let Some(solved) = selection.solved_outcome {
    outcome_to_store = Some(solved);
    ...
    if selection.all_solved { break; }
    if solved == Outcome::Win { break; } // <-- exits on first win
}
```

```rust
// After
if let Some(solved) = selection.solved_outcome {
    if solved == Outcome::Win {
        // OR node: keep the shortest known winning child.
        if selection.depth < best_win_depth {
            best_win_depth = selection.depth;
            best_win_move = selection.best_move;
            outcome_to_store = Some(solved);
            outcome_to_store_best_move = best_win_move;
            outcome_to_store_depth = best_win_depth;
            outcome_to_store_pn = selection.pn;
            outcome_to_store_dn = selection.dn;
        }
        if selection.all_solved {
            break;
        }
        // Continue if an unsolved child could improve best_win_depth.
        // (Use the unsolved children's pn/dn to bound their possible depth.)
    } else if solved == Outcome::Loss {
        // AND node: keep the longest known losing child.
        if selection.depth > best_loss_depth {
            best_loss_depth = selection.depth;
            best_loss_move = selection.best_move;
            outcome_to_store = Some(solved);
            outcome_to_store_best_move = best_loss_move;
            outcome_to_store_depth = best_loss_depth;
            outcome_to_store_pn = selection.pn;
            outcome_to_store_dn = selection.dn;
        }
        if selection.all_solved {
            break;
        }
    } else {
        // Draw or Loss/Draw with unresolved siblings.
        outcome_to_store = Some(solved);
        ...
        if selection.all_solved { break; }
    }
}
```

Key rules:

- At an **OR** node (White trying to win), a `Win` outcome is not final until either all children are solved or the remaining unsolved children cannot beat the current `best_win_depth`. The stored `best_move` is the child with the smallest `1 + child.depth`.
- At an **AND** node (Black trying to survive), a `Loss` outcome is not final until all children are solved. The stored `best_move` is the child with the largest `1 + child.depth` (Black's most resistant defense).
- The existing threshold breaks `pn >= th_pn` / `dn >= th_dn` still apply and can cut the search short at non-root nodes. The refinement phase (see below) uses depth bounds to tighten these thresholds.

### 2. `is_solved_by_children` consistency

In `src/search/dfpn/selection.rs`, ensure the depth arithmetic is explicit:

- `Win` (parent finds a `Loss` child): `parent_win_depth = 1 + min(child.depth)` over all solved `Loss` children seen so far.
- `Loss` (parent finds all children `Win`): `parent_loss_depth = 1 + max(child.depth)` over all solved `Win` children.
- If a mix of `Win` and `Draw` children is fully solved and no `Loss` child exists, the parent is `Draw`. This is a separate correctness bug also noted in `docs/plans/dfpn/report_pv_issue.md` and should be fixed here because the depth selection cannot be correct otherwise.

Add a unit test in `src/search/dfpn/selection.rs` that exercises mixed `Win`/`Draw` children returning `Draw` and mixed `Win` depths returning the longest `Loss`.

### 3. Replace binary refinement with iterative deepening

In `src/search/dfpn/mod.rs`:

1. Extend the bootstrap so it records the smallest `max_depth` that returned a decisive outcome (`success_depth`) and the largest `max_depth` that returned `Draw` (`fail_depth`).
2. Change `solve_refined` to:
   - Start from `success_depth` and search downward: `max_depth = success_depth - 1, success_depth - 2, ..., fail_depth + 1`.
   - **Do not** clear the transposition table or history/killer tables between probes. Instead, let the previous probe's results guide the next probe. Clear only the `path`/`path_stack`/`path_code`.
   - For each probe, call `dfpn(pos, INF, INF, max_depth, true)` and extract a PV.
   - Track the shortest PV for which `outcome == best_outcome`. If a probe times out, keep the shortest PV found so far.
   - Stop when `max_depth` reaches `fail_depth + 1` or when the timeout is exceeded.
3. Remove the unbounded `dfpn` call at the start of `solve_refined`; the bootstrap already provides an upper bound. If the bootstrap did not find a decisive result, fall back to the current unbounded search.

This avoids the expensive first probe at `max_depth = 6`. In this position the bootstrap will typically find a win at `8`; the first refinement probe will be `max_depth = 7`, which solves in a few hundred nodes and returns the 7-plies PV. Only if time remains does the solver try `max_depth = 6` to prove minimality.

### 4. Improve `StaticAtomicScorer` for atomic checks

In `src/search/ordering.rs`, extend the threat detection:

```rust
// Existing branch: direct attack on a commoner.
if (attack_bb & board.commoners(them)) != Bitboard::EMPTY {
    if state.them_commoners_count == 1 { score += SCORE_THREAT_LAST; }
    else { score += SCORE_THREAT; }
}

// New branch: attack on a square adjacent to the last enemy commoner.
if state.them_commoners_count == 1 {
    let them_commoners = board.commoners(them);
    // There is only one, so we can pop it.
    let enemy_king_sq = them_commoners.pop_lsb();
    let near_king = attacks::king_attacks(enemy_king_sq);
    if (attack_bb & near_king) != Bitboard::EMPTY {
        score += SCORE_ATOMIC_CHECK; // e.g. 9_000, below SCORE_THREAT_LAST but above captures
    }
}
```

Constants:

```rust
const SCORE_ATOMIC_CHECK: i32 = 9_000;
```

This makes `b8g8` after `1.Rb8+ Kh7` score comparably to `b8h8`, because the rook on `g8` attacks `f8` and `h8`, both adjacent to the Black king on `h7`. The search will then explore the faster `b8g8` continuation before the slower `b8h8` line.

### 5. `extract_pv` depth consistency

In `src/search/dfpn/pv.rs`:

- `extract_pv_internal` already follows `best_result_for_path`. Once `best_move` is depth-optimal, this will produce the right PV.
- Add a debug-mode assertion (or warning) in `extract_pv_checked` that the extracted PV length equals the stored root `depth` when `expected_depth` is `Some(depth)`. This catches cases where the stored `best_move` and `depth` disagree.
- Keep `validate_pv` as the source of truth for final correctness.

## File changes

### `src/search/dfpn/core.rs`

- Add `best_win_depth`, `best_win_move`, `best_loss_depth`, `best_loss_move` locals.
- Modify the `if let Some(solved) = selection.solved_outcome` block as described in section 1.
- Update the final `tt.store` call to use the running best move/depth when an outcome is known.

### `src/search/dfpn/selection.rs`

- Ensure `is_solved_by_children` returns `Draw` when fully solved children are a mix of `Win` and `Draw` and no `Loss` child exists.
- Add a unit test for the mixed `Win`/`Draw` case.

### `src/search/dfpn/mod.rs`

- Record `success_depth` and `fail_depth` in the bootstrap loop.
- Rewrite `solve_refined` as an iterative-deepening downward search from `success_depth` to `fail_depth + 1`.
- Do not clear the transposition table between refinement probes; clear only path state.

### `src/search/ordering.rs`

- Add `SCORE_ATOMIC_CHECK`.
- Add the adjacent-commoner threat branch in `StaticAtomicScorer::score`.

### `src/search/dfpn/pv.rs`

- Add an assertion/warning in `extract_pv_checked` for PV length vs. stored root depth.

### `tests/`

- Add `tests/test_pv.rs` (or extend `tests/test_plan6.rs`) with:
  - `m27_shortest_pv`: for `6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26`, assert `outcome == Win`, `pv.len() == 7`, and the first two moves are `b1b8 g8f7`.
  - `m27_kh7_fast_win`: for `1R6/3p3c/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7C w - - 2 27`, assert `pv` equals `b8g8 c5c4 g8g6`.
- Re-evaluate the ignored tests in `tests/test_plan6.rs` after the changes.

## Testing and verification

1. Run the full test suite:

   ```bash
   cargo fmt
   cargo clippy --all-targets
   cargo test
   cargo doc
   ```

2. Manual checks:

   ```bash
   cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"
   # Expected:
   # outcome: win
   # pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8

   cargo run --release --example solve_depth_limited -- \
       "1R6/3p3c/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7C w - - 2 27" 3
   # Expected:
   # outcome: Win
   # pv: b8g8 c5c4 g8g6
   ```

3. Performance baseline: ensure `m27_white_wins` in `tests/test_plan6.rs` still completes within the 5-second timeout and the new `m27_shortest_pv` test passes.

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Continuing the search after the first `Win` could increase node counts on some positions. | Only do full expansion when `refine_shortest` is enabled. The existing `max_ply` and timeout limits still apply. |
| Iterative deepening with TT reuse could retain stale `best_move` entries from a previous depth. | Store `remaining_depth` explicitly and let `try_use_tt` reject results whose `remaining_depth` or `depth` is incompatible with the current `max_depth`. |
| `SCORE_ATOMIC_CHECK` changes move ordering globally and could slow down other positions. | Keep it below `SCORE_THREAT_LAST` and `SCORE_WINNING_CAPTURE` so tactical captures and direct checks still come first. Run the full test suite to detect regressions. |
| Proving the lower bound (`max_depth = 6`) may still be expensive. | The iterative deepening order tries the cheap upper-bound probes first; if the timeout is hit, the solver returns the shortest PV found so far instead of the long unbounded PV. |

## Success criteria

- `cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"` returns the 7-plies PV.
- `cargo test` passes with no new warnings from `clippy`.
- The new `m27_shortest_pv` and `m27_kh7_fast_win` regression tests pass.
- No previously passing test now times out.
