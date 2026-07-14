# DF-PN+ PV Quality / Principal-Variation Fix Report

## Summary

Implemented `plans/dfpn/plan3.md`: added a `depth` field to the transposition table and the child-evaluation pipeline, then used depth to choose the best move once a node is solved. This fixes the PV issue where the solver returned a short, suboptimal refutation instead of the longest defense (or shortest win). It also fixes the related bug in `is_solved_by_children` where a mixed `Win`/`Draw` child set was incorrectly reported as `Loss`.

## Files Changed

| File | What changed |
|------|--------------|
| `src/search/tt.rs` | Added `depth: u32` to `TtEntry` and `TranspositionTable::store`. The field is meaningful only when `outcome` is `Some`; unsolved entries store `0`. |
| `src/search/dfpn.rs` | Added `depth` to `ChildInfo` and `ChildSelection`. `evaluate_child` reads `entry.depth` for solved TT hits. `is_solved_by_children` now returns `(Outcome, depth, best_move)` and uses depth to pick the shortest win / longest defense / consistent draw. `select_children` uses the solved result for `best_move` and `depth`. `dfpn` stores `depth` and the solved `best_move` in the TT. Added unit tests for `is_solved_by_children`. |
| `tests/test_plan3.rs` | New regression test for the canonical position `rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3`, asserting the expected `Outcome::Loss` and PV `d7d6 d5f7 e8d7 f7e7`. |

## Algorithm Details

- **Depth semantics**: `TtEntry.depth` is the number of plies to terminal conversion under optimal play from the side to move at that node. Terminals and direct draws store `0`.
- **Child depth**: `evaluate_child` propagates `entry.depth` for solved TT hits and sets `0` for unsolved or terminal/repetition positions.
- **Solved-by-children**: `is_solved_by_children` now evaluates the child outcome set as a minimax:
  - A `Loss` child (for the child side) means the parent side to move can win. The parent is `Win` with `depth = 1 + min(child.depth)` over all winning children.
  - If no winning child but at least one `Draw` child, the parent is `Draw` with `depth = 1 + max(child.depth)` over drawing children.
  - If all children are `Win` for the child side, the parent is `Loss` with `depth = 1 + max(child.depth)` (longest defense).
  - Mixed `Win`/`Draw` with no `Loss` child is now reported as `Draw`, not `Loss`.
- **Win early-out**: A `Win` is reported as soon as a losing child is found, so the search does not waste time exploring sibling moves once a forced win is known. This avoids the 5-second timeout seen on trivial mating positions (e.g. `4R1K1` vs `4k3` with `Rxe8`). Depth is still minimized over all *currently known* winning children.
- **Best move selection**: `select_children` overrides `best_move` with the solved move when `is_solved_by_children` returns a result; otherwise it falls back to the expansion-order `best_child.mv`.
- **PV extraction**: `extract_pv` already had a `1000`-ply bound and a `HashSet` to stop cycles, so it needed no changes.

## Test Results

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
cargo doc
```

All pass. Selected output:

```
running 4 tests
test search::dfpn::tests::draw_picks_longest_draw_child ... ok
test search::dfpn::tests::loss_picks_longest_win_child ... ok
test search::dfpn::tests::unsolved_returns_none ... ok
test search::dfpn::tests::win_picks_shortest_loss_child ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 7 tests
test solve_no_white_pieces_black_win ... ok
test solve_no_white_pieces_loss ... ok
test solve_king_only_draw_white ... ok
test solve_king_only_draw_black ... ok
test solve_opposed_kings_draw ... ok
test solve_rook_mate_win ... ok
test solve_rook_mate_black_to_move_draw ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 10 tests
test mate_in_1_black_to_move ... ok
test mate_in_1_white_to_move ... ok
test mate_in_2_black_to_move ... ok
test only_two_kings_draw_black_to_move ... ok
test only_two_kings_draw_white_to_move ... ok
test win_with_exploded_black_king_black_to_move ... ok
test win_with_exploded_black_king_white_to_move ... ok
test mate_in_4_white_to_move ... ok
test mate_in_2_white_to_move ... ok
test mate_in_3_black_to_move ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 1 test
test longest_defense_pv ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Manual Verification

The canonical regression position now prints the longest defense:

```bash
cargo run -- --fen "rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3"
```

Output:

```text
outcome: loss
pv: d7d6 d5f7 e8d7 f7e7
```

A previously- problematic quick mate is also found correctly:

```bash
cargo run -- --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1"
```

Output:

```text
outcome: win
pv: e1e8
```

## Known Limitations / Future Work

- The `Win` early-out in `is_solved_by_children` guarantees a forced win but, for wins that are not immediate mate, does not guarantee the globally shortest win because the search stops at the first proven losing child. For the tested positions the chosen PV is already optimal, but a stricter shortest-win refinement could be added later.
- `outcome_from_pn_dn` is still unused; it cannot distinguish `Loss` from `Draw` because both collapse to `(INF, 0)`.
- `Depth` is stored as `u32`; for any realistic atomic-chess position this is effectively unbounded, and `saturating_add` is used when computing `1 + child.depth`.
