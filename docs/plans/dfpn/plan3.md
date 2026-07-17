# Plan: Fix PV Extraction / Principal-Variation Quality for DF-PN+ Solver

This plan fixes the problem described in `plans/dfpn/report_pv_issue.md`: the solver returns the correct game-theoretic outcome for a position but prints a short, suboptimal refutation instead of the longest defense (or shortest win). It also fixes a related correctness bug in `is_solved_by_children` for mixed `Win`/`Draw` child sets.

Parallelization and algorithmic changes to the core DF-PN+ search are out of scope.

## 1. Goal and scope

### Goal

After `solve()` finishes, the printed PV must be:

- For a `Win` node: the shortest forced win from the side to move.
- For a `Loss` node: the longest defense before the forced loss.
- For a `Draw` node: any drawing line (not a collapsed win/loss).

The current implementation reaches the correct outcome but selects `best_move` from static move ordering when all children have identical `(pn, dn)` pairs, so it cannot distinguish between a two-ply win and a four-ply win for the same parent.

### Non-goals

- Changing the DF-PN+ threshold algorithm, GHI handling, or epsilon trick.
- Parallel search.
- New heuristics or move ordering.

### Affected test positions

The canonical regression position is:

```text
rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3
```

Expected after the fix:

```text
outcome: loss
pv: d7d6 d5f7 e8d7 f7e7
```

The short `c7c5 d5d7` refutation is still a legal forced win, but it is not the principal variation because Black can defend longer with `d7d6`.

## 2. Root cause

1. `Outcome::to_pn_dn` collapses solved `Loss` and `Draw` to the same `(INF, 0)` pair.
2. `select_children`/`best_and_second` order children by virtual `pn` for OR nodes and virtual `dn` for AND nodes.
3. When every child is already solved, all children have the same `(0, INF)` or `(INF, 0)` pair from the parent node's perspective, so `best_and_second` falls back to the static move order produced by `sort_moves`.
4. `extract_pv` follows the stored `best_move` chain, so it produces whatever `sort_moves` happened to put first.
5. The solver has no concept of mate distance or "depth to conversion".
6. `is_solved_by_children` returns `Loss` for any set of fully resolved children that are not all `Draw`, even if a `Draw` child exists. If one child is a win and another is a draw, the player to move can choose the draw, so the parent should be `Draw`, not `Loss`.

## 3. Strategy: track depth in the transposition table

The smallest, cleanest fix is to add a `depth` field to solved transposition-table entries and propagate it when a node is resolved. This depth is the number of plies to terminal conversion under optimal play from both sides.

- `Win` for the player to move: `depth = 1 + min(child.depth)` over all children that are a win for the player (i.e. child outcome from the parent's perspective is `Win`). This gives the *shortest* win.
- `Loss` for the player to move: `depth = 1 + max(child.depth)` over all children that are a loss for the player. This gives the *longest* defense.
- `Draw`: `depth = 1 + max(child.depth)` over all drawing children, or `0` for a direct terminal draw. For our purposes `0` is safe because the PV only needs a consistent draw, not a shortest draw.

When a node is solved, the `best_move` is set to the child that achieves the selected depth, not the child that happened to come first in static ordering.

This is cheaper and more robust than a post-solution re-evaluation of children.

## 4. Data structure changes

### 4.1 `TtEntry`

Add a `depth` field. The field is only meaningful when `outcome` is `Some(_)`; for unsolved entries it is `0`.

```rust
pub struct TtEntry {
    pub key: u64,
    pub best_move: Move,
    pub outcome: Option<Outcome>,
    pub pn: u64,
    pub dn: u64,
    pub depth: u32,      // NEW: plies to conversion when solved
    pub path_code: u64,
    pub repetition_seen: bool,
    pub valid: bool,
}
```

Update `Default` and `store` accordingly.

### 4.2 `ChildInfo`

Add `depth` and `child_is_or` is no longer needed inside `ChildInfo`.

```rust
struct ChildInfo {
    mv: Move,
    pn: u64,
    dn: u64,
    vpn: u64,
    vdn: u64,
    outcome: Option<Outcome>,
    depth: u32,          // NEW: child depth when solved, 0 otherwise
    repetition_seen: bool,
}
```

### 4.3 `ChildSelection`

Add `depth` and `best_move` semantics remain the same.

```rust
struct ChildSelection {
    best_child: (Move, u64, u64, u64, u64),
    second_child: (u64, u64),
    pn: u64,
    dn: u64,
    depth: u32,          // NEW: solved depth when known
    best_move: Move,
    solved_outcome: Option<Outcome>,
    repetition_seen: bool,
}
```

## 5. Algorithm changes

### 5.1 Terminal and draw nodes

When a node is terminal via `pos.outcome()` or `moves.is_empty()`, store `depth = 0`.

```rust
// in dfpn, terminal position path
self.tt.store(
    key,
    Move::NONE,
    Some(outcome),
    pn,
    dn,
    0,               // depth
    self.path_code,
    false,
);
```

### 5.2 `evaluate_child`

`evaluate_child` must read `depth` from the TT when a child is solved.

For terminal positions:

```rust
ChildInfo {
    mv,
    pn,
    dn,
    vpn: pn,
    vdn: dn,
    outcome: Some(outcome),
    depth: 0,
    repetition_seen: false,
}
```

For a solved TT hit:

```rust
ChildInfo {
    mv,
    pn,
    dn,
    vpn: pn,
    vdn: dn,
    outcome: Some(outcome),
    depth: entry.depth,
    repetition_seen: entry.repetition_seen,
}
```

For unsolved children, `depth = 0`.

### 5.3 `is_solved_by_children`

Rewrite `is_solved_by_children` to be aware of the node type and of depth.

```rust
fn is_solved_by_children(
    children: &[ChildInfo],
    is_or_node: bool,
) -> Option<(Outcome, u32, Move)> {
    // OR node (attacker to move): one Win child is enough.
    if is_or_node {
        let mut best_win: Option<(u32, Move)> = None;
        let mut all_draw = true;
        let mut any_loss_or_draw = false;
        for c in children {
            match c.outcome {
                None => return None,
                Some(Outcome::Win) => {
                    any_loss_or_draw = true;
                    let candidate = c.depth.saturating_add(1);
                    if best_win.map_or(true, |(d, _)| candidate < d) {
                        best_win = Some((candidate, c.mv));
                    }
                }
                Some(Outcome::Draw) => {
                    all_draw = false;
                    any_loss_or_draw = true;
                }
                Some(Outcome::Loss) => {
                    all_draw = false;
                    any_loss_or_draw = true;
                }
            }
        }

        if let Some((depth, mv)) = best_win {
            return Some((Outcome::Win, depth, mv));
        }

        if all_draw {
            return Some((Outcome::Draw, 0, Move::NONE));
        }

        // All children are Loss/Draw and there is no Win -> Loss for attacker.
        // Pick the longest defense (max depth) for best_move.
        let mut best_depth = 0;
        let mut best_mv = Move::NONE;
        for c in children {
            if c.outcome != Some(Outcome::Draw) {
                let d = c.depth.saturating_add(1);
                if d > best_depth {
                    best_depth = d;
                    best_mv = c.mv;
                }
            }
        }
        return Some((Outcome::Loss, best_depth, best_mv));
    }

    // AND node (defender to move): all children must be Win for the attacker.
    let mut all_win = true;
    let mut any_draw = false;
    let mut best_depth = u32::MAX;
    let mut best_mv = Move::NONE;
    for c in children {
        match c.outcome {
            None => return None,
            Some(Outcome::Win) => {
                any_draw = true; // Win for attacker is good for attacker; from defender side
            }
            Some(Outcome::Draw) => {
                all_win = false;
                any_draw = true;
            }
            Some(Outcome::Loss) => {
                all_win = false;
            }
        }
    }

    if all_win {
        for c in children {
            if c.outcome == Some(Outcome::Win) {
                let d = c.depth.saturating_add(1);
                if d < best_depth {
                    best_depth = d;
                    best_mv = c.mv;
                }
            }
        }
        return Some((Outcome::Win, best_depth, best_mv));
    }

    if any_draw {
        return Some((Outcome::Draw, 0, Move::NONE));
    }

    // All children are Loss for attacker -> Loss for attacker (Win for defender).
    let mut worst_depth = 0;
    let mut best_defense = Move::NONE;
    for c in children {
        if c.outcome == Some(Outcome::Loss) {
            let d = c.depth.saturating_add(1);
            if d > worst_depth {
                worst_depth = d;
                best_defense = c.mv;
            }
        }
    }
    Some((Outcome::Loss, worst_depth, best_defense))
}
```

**Important correction in `is_solved_by_children`**: if a node has only `Win` and `Draw` children and no `Loss` child, and the side to move could choose the `Draw`, the result is `Draw`, not `Loss`.

Because the `outcome` field in `ChildInfo` is always from the **child's** perspective, the parent outcome must be derived relative to the parent side:

- OR node (attacker to move):
  - Any child `Win` (from child's perspective) means the attacker wins on their move -> parent `Win`.
  - All children `Loss` (or `Draw` with no `Win` child) -> parent `Loss` or `Draw`.
  - If any `Draw` child and no `Win` child, parent is `Draw` (attacker cannot force a win).
  - Otherwise parent `Loss`.
- AND node (defender to move):
  - All children `Loss` (from child's perspective) means defender escaped on every reply -> parent `Loss`.
  - Any child `Draw` and no `Win` child -> parent `Draw`.
  - Any child `Win` (attacker can still win) -> parent `Win`.

The sketch above keeps the same idea; the actual implementation in the repo may need to be checked for the exact OR/AND outcome mapping used in the rest of the search. The key points are:

1. Return `(Outcome, depth, best_move)` so the caller can store the mate depth.
2. Never return `Loss` when a `Draw` child is available and no `Win` child is forced.
3. Pick the child that minimizes depth for `Win` and maximizes depth for `Loss`.

### 5.4 `select_children` and `best_move` update

`select_children` should continue to use virtual `pn`/`dn` for unsolved child ordering, but when a node is fully solved it should use depth to pick `best_move`.

Pseudo-change:

```rust
let solved = Self::is_solved_by_children(&children, is_or_node);

let best_move = if let Some((_, depth, mv)) = solved {
    mv
} else {
    children[best_idx].mv
};

let depth = solved.map(|(_, d, _)| d).unwrap_or(0);

ChildSelection {
    best_child,
    second_child,
    pn,
    dn,
    depth,
    best_move,
    solved_outcome: solved.map(|(o, _, _)| o),
    repetition_seen,
}
```

This means the solver's "best_move" for a solved node no longer depends on static move ordering.

### 5.5 `dfpn` storing the solved result

At the end of `dfpn`, use `selection.depth` and `selection.best_move` when storing the solved result.

```rust
self.tt.store(
    key,
    best_move,
    outcome_to_store,
    pn,
    dn,
    selection.depth,
    old_path_code,
    repetition_seen,
);
```

`best_move` should come from the solved selection, not from `selection.best_child.0`. If `solved_outcome` is `None`, the node is not solved, and `best_move` and `depth` are still stored for PV purposes but `outcome` is `None`.

### 5.6 `extract_pv`

`extract_pv` already follows the `best_move` chain. It will automatically produce the correct line once `best_move` is chosen by depth. Add a safety bound to avoid infinite loops in case of a corrupted best-move chain:

```rust
fn extract_pv(&self, pos: &Position) -> Vec<Move> {
    const MAX_PV_LEN: usize = 1000;
    let mut pv = Vec::new();
    let mut seen = HashSet::new();
    let mut current = pos.clone();
    for _ in 0..MAX_PV_LEN {
        let key = current.hash();
        if seen.contains(&key) {
            break;
        }
        if current.outcome().is_some() {
            break;
        }
        if let Some(entry) = self.tt.probe(key) {
            if entry.best_move == Move::NONE {
                break;
            }
            seen.insert(key);
            pv.push(entry.best_move);
            current.do_move(entry.best_move);
        } else {
            break;
        }
    }
    pv
}
```

This is already implemented in `plan2`; verify the `MAX_PV_LEN` constant is present.

## 6. File changes

### `src/search/tt.rs`

- Add `depth: u32` to `TtEntry`.
- Update `TtEntry::default`.
- Update `store` signature to accept `depth` and persist it.

### `src/search/dfpn.rs`

- Add `depth` to `ChildInfo` and `ChildSelection`.
- Update `evaluate_child` to read `entry.depth` from the TT for solved children.
- Update `select_children` to return depth and use `is_solved_by_children` to choose `best_move` when solved.
- Update `is_solved_by_children` to return `(Outcome, u32, Move)` and correctly handle `Win`/`Draw`/`Loss` combinations.
- Update `dfpn` to store `selection.depth` and the solved `best_move`.
- Do not change the threshold loop, epsilon, or GHI logic.

### `src/position.rs`

- No changes required for this fix unless `Outcome` helpers are needed. The `depth` concept is owned by the search/TT layer.

### `src/main.rs`

- No changes.

## 7. Testing and verification

### 7.1 Regression test

Add the position from the report to the integration tests:

```rust
#[test]
fn longest_defense_pv() {
    let mut search = Search::new(64);
    let mut pos = Position::from_fen("rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3").unwrap();
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Loss);
    assert_eq!(pv, vec![
        Move::from_str("d7d6").unwrap(),
        Move::from_str("d5f7").unwrap(),
        Move::from_str("e8d7").unwrap(),
        Move::from_str("f7e7").unwrap(),
    ]);
}
```

If `Move::from_str` is not available, construct moves with the existing `Move::new` or `Move::from` helpers.

### 7.2 Existing tests

Ensure these continue to pass:

- `tests/test_inf.rs` basic positions.
- `cargo test`.
- `cargo clippy`.
- `cargo fmt`.
- `cargo doc`.

### 7.3 Manual verification

Run:

```bash
cargo run -- --fen "rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3"
```

Expected output:

```text
outcome: loss
pv: d7d6 d5f7 e8d7 f7e7
```

Also verify the other positions from `plan2.md` section 6.2 produce correct outcomes and plausible PVs.

## 8. Risks and mitigations

- **Depth overflow**: `u32` is large enough for any realistic atomic-chess mate. Use `saturating_add` when computing `1 + child.depth`.
- **Depth for unsolved entries**: `TtEntry::depth` is only read when `outcome` is `Some`. If `outcome` is `None`, depth is ignored.
- **TT replacement**: `TranspositionTable::store` overwrites unconditionally. If a deeper but equally solved entry is probed later, its depth is still correct because the overwrite is deterministic. If the old `best_move` was better (shorter win / longer loss), ensure the new depth is recomputed before storage, not carried over from the old entry.
- **Correctness of mixed Win/Draw**: the new `is_solved_by_children` must be carefully reviewed. If a node has children `Win` and `Draw` from the child's perspective, the parent should be `Draw` if the player to move can choose the draw and the parent cannot force a win. Add targeted unit tests for `is_solved_by_children`.
- **GHI/twin interaction**: adding `depth` does not change the GHI path-code logic, but a solved entry with `repetition_seen` should still be trusted only if the path code matches. `depth` is stored along with the solved result and is path-dependent in the same way as `outcome`.

## 9. Summary

1. Add `depth: u32` to `TtEntry`, `ChildInfo`, and `ChildSelection`.
2. Propagate depth from terminal nodes (`0`) and solved TT children.
3. Change `is_solved_by_children` to return `(Outcome, depth, best_move)` and correctly treat mixed `Win`/`Draw` children as `Draw` when appropriate.
4. Use depth to select `best_move` in `select_children` when the node is fully solved.
5. Store `depth` and `best_move` in `dfpn`.
6. Verify `extract_pv` produces the longest defense for `Loss` and the shortest win for `Win`.
7. Add regression test and run `cargo fmt`, `cargo clippy`, `cargo test`, `cargo doc`.
