# Plan: Fix Shortest-Win Selection in DF-PN+ Solver

## Summary

`plans/dfpn/plan3.md` added depth tracking and used it to choose `best_move` once a node is *fully* solved. The canonical regression still passes, but the solver can still emit a much longer win when a shorter forced win exists. The new regression position is:

```text
6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28
```

Current output:

```text
outcome: win
pv: g8f8 c5c4 c3d5 c4c3 f3f4 c3d2 d6c5 d7d6 c5d6
```

A shorter forced win is available, e.g.:

```text
g8g7 e6f5 g7g6
```

(or `g8g7 c5c4 g7d7`, both mate in 2 for White).

This plan fixes the shortest-win selection by removing the `Win` early-out in `is_solved_by_children` and making `dfpn` continue refining a proven win until every sibling has been solved and the true minimum depth is known.

## Goal and scope

### Goal

For any `Win` node, the solver's `best_move` and stored `depth` must be the *shortest* forced win from the side to move. For `Loss`/`Draw` nodes the existing semantics (longest defense / any drawing line) remain.

### Non-goals

- Parallel search.
- New move-ordering heuristics (we continue to use the existing `StaticAtomicScorer`).
- Changing the GHI path-code logic or the epsilon-threshold trick.

## Root cause

1. `is_solved_by_children` returns `Outcome::Win` as soon as it sees *any* child that is a `Loss` for the child side (i.e. a win for the parent side).
2. `dfpn` immediately breaks the search loop when `select_children.solved_outcome` is `Some(Win)`.
3. Because `dfpn` only expands the single most-proving child, the first winning move that is proven becomes the stored `best_move`, even if an unexplored sibling is a much shorter win.
4. The `depth` field does track the minimum depth among *currently known* winning children, but a child that has never been searched has `depth = 0` and `outcome = None`, so the minimum is taken over an incomplete set.

In the regression position the move `g8f8` happens to be ordered before `g8g7` and is itself a win, so `dfpn` proves it, stores the long PV, and stops. The 3-ply win `g8g7` is never searched.

## Proposed fix

Make the solver refine a `Win` until all siblings are resolved. A `Win` is declared as soon as one losing child is found, but the search continues to find the shortest one. The algorithmic changes are:

1. `is_solved_by_children` returns a result as soon as a winning child is found, but it also reports whether *all* children have been resolved.
2. `dfpn` does **not** terminate on `solved_outcome == Some(Win)` while any child is still unsolved.
3. `select_children` picks the child to expand from the **unsolved** children only, skipping already-solved siblings.
4. The `pn`/`dn` threshold break is bypassed while the node is a proven win with unresolved siblings.

This is recursive: a child that itself is a `Win` will continue to refine its own siblings, so the shortest win is found throughout the PV.

## 4. Data structure changes

### 4.1 `TtEntry`

No change. `TtEntry.depth` already stores the plies to conversion for solved entries.

### 4.2 `ChildInfo`

No change. `outcome` is `None` for unsolved children and `Some` for solved children; `depth` is meaningful when `outcome` is `Some`.

### 4.3 `ChildSelection`

Add an `all_solved` flag and make `best_child` the unsolved child to expand. `best_move` is the solved best move (for storage), which may differ from `best_child.mv` while the node is a pending win.

```rust
struct ChildSelection {
    best_child: (Move, u64, u64, u64, u64), // child to expand; Move::NONE if none left
    second_child: (u64, u64),
    pn: u64,
    dn: u64,
    depth: u32,
    best_move: Move,                         // move to store (shortest win / longest defense)
    solved_outcome: Option<Outcome>,
    all_solved: bool,                         // NEW
    repetition_seen: bool,
}
```

### 4.4 `is_solved_by_children` return type

Change to return a 4-tuple including the "all solved" flag:

```rust
fn is_solved_by_children(
    children: &[ChildInfo],
    _is_or_node: bool,
) -> Option<(Outcome, u32, Move, bool)>
```

The `bool` is `true` only when every child has `outcome = Some(_)`.

## 5. Algorithm changes

### 5.1 `is_solved_by_children`

Keep the same traversal as today, but return the minimum-depth winning child as soon as one is found, with `all_solved = false` if any child is still `None`. Only return `Outcome::Loss` or `Outcome::Draw` when `all_solved` is `true`.

```rust
fn is_solved_by_children(
    children: &[ChildInfo],
    _is_or_node: bool,
) -> Option<(Outcome, u32, Move, bool)> {
    let mut all_solved = true;
    let mut win_depth = u32::MAX;
    let mut win_mv = Move::NONE;
    let mut draw_depth = 0;
    let mut draw_mv = Move::NONE;
    let mut found_draw = false;
    let mut loss_depth = 0;
    let mut loss_mv = Move::NONE;

    for c in children {
        let d = c.depth.saturating_add(1);
        match c.outcome {
            None => {
                all_solved = false;
            }
            Some(Outcome::Loss) => {
                // A Loss child is a win for the parent.
                if d < win_depth {
                    win_depth = d;
                    win_mv = c.mv;
                }
            }
            Some(Outcome::Draw) => {
                if d > draw_depth {
                    draw_depth = d;
                    draw_mv = c.mv;
                }
                found_draw = true;
            }
            Some(Outcome::Win) => {
                if d > loss_depth {
                    loss_depth = d;
                    loss_mv = c.mv;
                }
            }
        }
    }

    // A win can be announced immediately, but the search must continue
    // to find the shortest one if children are still unsolved.
    if win_depth != u32::MAX {
        return Some((Outcome::Win, win_depth, win_mv, all_solved));
    }

    if all_solved {
        if found_draw {
            return Some((Outcome::Draw, draw_depth, draw_mv, true));
        }
        return Some((Outcome::Loss, loss_depth, loss_mv, true));
    }

    None
}
```

### 5.2 `select_children`

- Compute `pn` and `dn` from all children (unchanged).
- Call `is_solved_by_children` for the solved result and `best_move`.
- Pick `best_child` and `second_child` from **unsolved** children only, using the same `vpn`/`vdn` comparison as today. If every child is solved, `best_child.mv` is `Move::NONE`.
- Set `all_solved` from `is_solved_by_children`.

```rust
let solved = Self::is_solved_by_children(&children, is_or_node);

// best_child_for_expansion and second_child_for_expansion are computed
// over children with outcome == None.
let (best_idx, second_idx) = Self::best_and_second_unsolved(&children, is_or_node);
let best = best_idx.map(|i| &children[i]);
let second = second_idx.map(|i| &children[i]);

let best_child = best.map(|b| (b.mv, b.pn, b.dn, b.vpn, b.vdn))
    .unwrap_or((Move::NONE, INF, INF, INF, INF));
let second_child = second.map(|s| (s.pn, s.dn)).unwrap_or((INF, INF));

let best_move = if let Some((_, _, mv, _)) = solved {
    mv
} else {
    best_idx.map(|i| children[i].mv).unwrap_or(Move::NONE)
};

let depth = solved.map(|(_, d, _, _)| d).unwrap_or(0);
let all_solved = solved.map(|(_, _, _, all)| all).unwrap_or(false);

ChildSelection {
    best_child,
    second_child,
    pn,
    dn,
    depth,
    best_move,
    solved_outcome: solved.map(|(o, _, _, _)| o),
    all_solved,
    repetition_seen,
}
```

### 5.3 `best_and_second` becomes `best_and_second_unsolved`

The current `best_and_second` uses `vpn`/`vdn` for all children. Replace it with a helper that ignores children with `outcome.is_some()` (already resolved). If there are no unsolved children, return `(None, None)`.

The comparison is unchanged:

- OR node: minimize `vpn`.
- AND node: minimize `vdn`.

Tie-breaker: prefer the child with fewer `num_searched` (or keep the current static ordering if that field is not available). The existing `best_and_second` already falls through to static order on ties, which is acceptable.

### 5.4 `dfpn` loop

The loop now has two termination cases:

1. The node is fully solved (`solved_outcome.is_some() && all_solved`).
2. The `pn`/`dn` thresholds are reached and the node is **not** a pending win with unresolved siblings.

```rust
loop {
    if Instant::now() >= self.deadline {
        break;
    }

    let selection = self.select_children(pos, &moves, is_or_node);
    best_move = selection.best_move;
    pn = selection.pn;
    dn = selection.dn;
    depth = selection.depth;
    repetition_seen = selection.repetition_seen;

    if let Some(solved) = selection.solved_outcome {
        outcome_to_store = Some(solved);
        if selection.all_solved {
            break;
        }
        // Otherwise the node is a proven Win with siblings still unresolved.
        // Keep the current best_move/depth and continue refining.
    }

    if (th_pn != INF && pn >= th_pn) || (th_dn != INF && dn >= th_dn) {
        // A pending Win has pn = 0 / dn = INF.  We must keep refining even
        // though dn >= th_dn, otherwise we would stop at the first Win.
        if selection.solved_outcome != Some(Outcome::Win) || selection.all_solved {
            break;
        }
    }

    let (mv, child_pn, child_dn, _vpn, _vdn) = selection.best_child;
    if mv == Move::NONE {
        // No unsolved children left, but all_solved should have been true.
        break;
    }

    let (second_pn, second_dn) = selection.second_child;

    // Threshold computation stays the same as in plan3, but best_child and
    // second_child are now unsolved children.
    let (np, nd) = if is_or_node {
        let new_th_pn = std::cmp::min(th_pn, self.epsilon_ceil(second_pn));
        let new_th_dn = if th_dn == INF {
            INF
        } else {
            th_dn.saturating_sub(dn).saturating_add(child_dn)
        };
        (new_th_pn, new_th_dn)
    } else {
        let new_th_dn = std::cmp::min(th_dn, self.epsilon_ceil(second_dn));
        let new_th_pn = if th_pn == INF {
            INF
        } else {
            th_pn.saturating_sub(pn).saturating_add(child_pn)
        };
        (new_th_pn, new_th_dn)
    };

    pos.do_move(mv);
    self.path_code ^= zobrist::path_random(mv, self.path_stack.len());
    let _ = self.dfpn(pos, np, nd, !is_or_node);
    self.path_code ^= zobrist::path_random(mv, self.path_stack.len());
    pos.undo_move(mv);
}
```

The recursive call on an unsolved child will itself keep refining until all its siblings are resolved, so the shortest win depth propagates upward.

### 5.5 `dfpn` storage

At the end of `dfpn`, store the current best result. If the search timed out before `all_solved`, `outcome_to_store` may still be `Some(Win)` from a proven win, but `best_move` and `depth` are the best known so far. Store them as-is. This is safe: the result is correct, only the depth might be larger than the true shortest if time ran out.

```rust
self.tt.store(
    key,
    best_move,
    outcome_to_store,
    pn,
    dn,
    depth,
    old_path_code,
    repetition_seen,
);
outcome_to_store.unwrap_or(Outcome::Draw)
```

### 5.6 `extract_pv`

`extract_pv` follows the `best_move` chain. Once `best_move` is the shortest winning child, `extract_pv` will produce the shortest PV. No changes needed.

## 6. File changes

### `src/search/dfpn.rs`

- Update `is_solved_by_children` to return `(Outcome, u32, Move, bool)`.
- Rename `best_and_second` to `best_and_second_unsolved` and skip solved children.
- Update `ChildSelection` to include `all_solved` and make `best_child` the unsolved child to expand.
- Update `select_children` to use `best_and_second_unsolved` and set `all_solved`.
- Update `dfpn` loop to continue refining a pending `Win`.

### `src/search/tt.rs`

No changes.

### `src/position.rs`

No changes.

### `src/main.rs`

No changes.

## 7. Testing and verification

### 7.1 Regression test

Add `tests/test_plan4.rs`:

```rust
use atomic_movegen::types::{Move, Square};
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

#[test]
fn shortest_win_g8g7() {
    let mut pos =
        Position::from_fen("6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28").unwrap();
    let mut search = Search::new(64);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
    let pv_str: String = pv.iter().map(|&m| move_to_uci(m)).collect::<Vec<_>>().join(" ");
    let first = pv.first().copied().unwrap();
    assert_eq!(first, Move::make_move(Square::G8, Square::G7));
    // 3 plies = mate in 2 for White.  The exact black reply may vary
    // (e.g. c5c4 or e6f5), but the total length must be 3.
    assert_eq!(pv.len(), 3, "expected a 3-ply win, got: {pv_str}");
}
```

### 7.2 Existing tests

Run the full suite:

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
cargo doc
```

Ensure the plan-3 regression test still passes:

```bash
cargo test longest_defense_pv
```

### 7.3 Manual verification

```bash
cargo run -- --fen "6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28"
```

Expected output:

```text
outcome: win
pv: g8g7 e6f5 g7g6
```

(Or `g8g7 c5c4 g7d7` or another 3-ply line; the exact black defense depends on move ordering, but the PV length must be 3.)

Also verify:

```bash
cargo run -- --fen "rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3"
```

Still expected:

```text
outcome: loss
pv: d7d6 d5f7 e8d7 f7e7
```

### 7.4 Rook mate sanity check

```bash
cargo run -- --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1"
```

Expected:

```text
outcome: win
pv: e1e8
```

## 8. Risks and mitigations

- **Search blow-up from refining all siblings**: The solver now explores every child of a winning node, not just the first one. For a win that is deep and has many sibling moves, this is more work. However, the first proven win still gives an upper bound: the `dfpn` loop only explores siblings that are not yet known to be longer than the current best. If `pn`/`dn` thresholds cut off an unsolved sibling, it is still not fully resolved, but the current best win is correct and will be stored. The 5-second deadline prevents runaway searches.
- **Loop on solved children**: `best_and_second_unsolved` must skip solved children, otherwise the loop would repeatedly expand the same solved child.
- **Pending-win threshold bypass**: Skipping `dn >= th_dn` only for `solved_outcome == Some(Win) && !all_solved` ensures the loop continues for the shortest win, but `pn`/`dn` and the thresholds are still used to guide the child search.
- **Draw handling**: `is_solved_by_children` returns `Draw` only when `all_solved` is true. If a `Draw` child is found and no `Win` child is found, the parent is still `None` while other children are unsolved, which is correct: a sibling could still be a shorter win.
- **Depth overflow**: `u32` and `saturating_add` are still sufficient for any realistic atomic-chess mate.

## 9. Summary

1. `is_solved_by_children` returns `Win` early with `all_solved = false` so `dfpn` can keep refining.
2. `select_children` separates `best_move` (solved shortest win) from `best_child` (unsolved child to expand).
3. `best_and_second` is replaced by `best_and_second_unsolved` to avoid selecting already-solved children.
4. `dfpn` continues the loop while a `Win` has unsolved siblings and bypasses the `dn >= th_dn` break in that case.
5. Add regression test for the reported FEN and verify existing tests still pass.
