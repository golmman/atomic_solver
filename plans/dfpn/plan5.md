# Plan: Full GHI Fix for DF-PN+ Solver

## Summary

This plan implements the full Kishimoto & Müller solution to the Graph-History Interaction (GHI) problem for the sequential `DF-PN+` solver in `src/search/dfpn.rs`. It replaces the current first-layer fix (`path` set returns `Draw` on a local cycle, `try_use_tt` trusts a single `path_code` match) with the complete base/twin transposition-table design and Kawano simulation described in `plans/dfpn/ghi.pdf` and `plans/dfpn/research_ghi.md`.

The main regression is the black-to-move position

```
6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27
```

which currently times out and returns `draw` even though `f7e6` leads to a short forced win for White (`g8g7 e6f5 g7g6` or `g8g7 c5c4 g7d7`). The timeout is caused by the same transposition being reused as a `Draw` in one search path while it is a `Win`/`Loss` in another, and by the `repetition_seen` flag being propagated from the whole child set instead of from the child that actually determines the result.

## Goal

1. Implement base and twin transposition-table entries.
2. Implement Kawano-style simulation to verify path-dependent results for new paths.
3. Keep path-independent proven/disproven results in the base entry.
4. Fix `repetition_seen` propagation so it reflects the `best_move` child only for `Win`/`Draw` nodes.
5. Ensure the regression FEN is solved within 60 seconds and returns `loss` (White wins) or a correct `draw` if `c5c4` is indeed a drawing resource.
6. Keep all existing tests passing.

## Non-goals

- Parallel search.
- New move-ordering heuristics (move ordering is out of scope; if GHI alone does not solve the FEN, move ordering will be handled in a follow-up plan).
- Changing the epsilon-threshold loop or the shortest-win refinement loop.

## Background

The current `TtEntry` holds a single `outcome`, a single `path_code`, and a `repetition_seen` flag. `try_use_tt` trusts an entry whenever `repetition_seen == false` or `path_code == entry.path_code`. This is incomplete because a position can be a draw by repetition in one path and a decisive win/loss in another. When a decisive result is later reached via a different path, the solver may incorrectly reuse a cached `Draw` (or vice versa) and either loop or stop too early.

The full fix from Kishimoto & Müller (AAAI-04) and summarized in `plans/dfpn/research_ghi.md` is:

- **Base entry**: stores `pn`/`dn` bounds for an unsolved position. If a result is proven without ever encountering a repetition, it is also stored in the base entry and is path-independent.
- **Twin entries**: stores each path-dependent proof/disproof keyed by a 64-bit path code (Zobrist XOR of the move sequence from the root).
- **Kawano simulation**: when a position is reached via a new path, a twin's cached proof tree is verified by following the stored `best_move` chain. If it succeeds, the result is reused and a new twin for the new path is stored. If it fails, the base entry's `pn`/`dn` bounds are used.

## Data structures

### `src/search/tt.rs`

Introduce a `TwinEntry` and change `TtEntry` to keep a small fixed-size twin list.

```rust
const MAX_TWINS: usize = 2;

#[derive(Clone, Copy, Debug, Default)]
struct TwinEntry {
    path_code: u64,
    outcome: Option<Outcome>, // None means empty
    best_move: Move,
    depth: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TtEntry {
    pub key: u64,
    pub valid: bool,

    // Base entry: bounds for unsolved nodes, or path-independent solved results.
    pub best_move: Move,
    pub outcome: Option<Outcome>,
    pub pn: u64,
    pub dn: u64,
    pub depth: u32,
    pub repetition_seen: bool,

    // Twin entries: path-dependent solved results.
    pub twins: [TwinEntry; MAX_TWINS],
}
```

`TtEntry` remains `Copy` so the existing `vec![[TtEntry::default(); 2]; buckets]` table allocation in `TranspositionTable::with_mb` still works. The `path_code` field is removed from the base entry; it now lives only inside `TwinEntry`.

`TranspositionTable` keeps its two-deep buckets but gains one helper:

```rust
pub fn store_twin(&mut self, key: u64, path_code: u64, outcome: Outcome, best_move: Move, depth: u32);
```

`store_twin` finds the existing bucket for `key`, adds/replaces a twin slot, and reinitializes the base `pn`/`dn` to `(1, 1)` with `outcome: None` and `repetition_seen: true` (per the paper).

### `src/search/dfpn.rs`

`try_use_tt` now returns a small result struct that carries the resolved `best_move` and `depth`:

```rust
struct Resolved {
    outcome: Outcome,
    best_move: Move,
    depth: u32,
    repetition_seen: bool,
}
```

`ChildInfo` remains unchanged except that `repetition_seen` is now the value for the *child that was actually used* for the result, not `any` child.

## Algorithm changes

### 1. `try_use_tt` with base + twins + simulation

```rust
fn try_use_tt(
    &mut self,
    pos: &Position,
    entry: &TtEntry,
    max_depth: u32,
    path_code: u64,
) -> Option<Resolved> {
    // 1. Path-independent base result.
    if let Some(outcome) = entry.outcome
        && !entry.repetition_seen
        && entry.depth <= max_depth
    {
        return Some(Resolved {
            outcome,
            best_move: entry.best_move,
            depth: entry.depth,
            repetition_seen: false,
        });
    }

    // 2. Try existing twins for the current path.
    for twin in entry.twins.iter() {
        if twin.outcome.is_some() && twin.path_code == path_code && twin.depth <= max_depth {
            return Some(Resolved {
                outcome: twin.outcome.unwrap(),
                best_move: twin.best_move,
                depth: twin.depth,
                repetition_seen: true,
            });
        }
    }

    // 3. Kawano simulation: verify a twin from another path for the current path.
    for twin in entry.twins.iter() {
        if twin.outcome.is_none() || twin.depth > max_depth {
            continue;
        }
        let mut sim_pos = pos.clone();
        if self.simulate(&mut sim_pos, twin.path_code, twin.outcome.unwrap(), twin.best_move) {
            self.tt.store_twin(
                entry.key,
                path_code,
                twin.outcome.unwrap(),
                twin.best_move,
                twin.depth,
            );
            return Some(Resolved {
                outcome: twin.outcome.unwrap(),
                best_move: twin.best_move,
                depth: twin.depth,
                repetition_seen: true,
            });
        }
    }

    None
}
```

### 2. Kawano simulation

Simulation is a bounded, recursive traversal of the cached proof/disproof tree. It starts from the original twin's `path_code` and walks the `best_move` chain. For `Win`/`Draw` it follows the stored `best_move`; for `Loss` it expands all legal children.

```rust
fn simulate(
    &self,
    pos: &mut Position,
    path_code: u64,
    expected: Outcome,
    best_move: Move,
    sim_path: &mut HashSet<u64>,
    sim_stack: &mut Vec<u64>,
    sim_nodes: &mut u64,
) -> bool {
    if *sim_nodes >= SIM_MAX_NODES {
        return false;
    }
    *sim_nodes += 1;

    if let Some(outcome) = pos.outcome() {
        return outcome == expected;
    }

    let key = pos.hash();
    if !sim_path.insert(key) {
        return false;
    }
    sim_stack.push(key);

    if sim_stack.len() > SIM_MAX_DEPTH {
        sim_stack.pop();
        sim_path.remove(&key);
        return false;
    }

    let ok = match expected {
        Outcome::Win | Outcome::Draw => {
            if best_move == Move::NONE {
                false
            } else {
                pos.do_move(best_move);
                let child_path_code = path_code ^ zobrist::path_random(best_move, sim_stack.len());
                let child_expected = if expected == Outcome::Draw {
                    Outcome::Draw
                } else {
                    Outcome::Loss
                };
                let entry = self.tt.probe(pos.hash());
                let child_best = entry.and_then(|e| e.find_result_for_path(child_path_code, child_expected));
                let ok = child_best.map_or(false, |b| {
                    self.simulate(pos, child_path_code, child_expected, b.best_move, sim_path, sim_stack, sim_nodes)
                });
                pos.undo_move(best_move);
                ok
            }
        }
        Outcome::Loss => {
            let mut moves = MoveList::new();
            pos.legal_moves(&mut moves);
            let mut ok = true;
            for i in 0..moves.len() {
                let mv = moves[i];
                pos.do_move(mv);
                let child_path_code = path_code ^ zobrist::path_random(mv, sim_stack.len());
                let entry = self.tt.probe(pos.hash());
                let child_best = entry.and_then(|e| e.find_result_for_path(child_path_code, Outcome::Win));
                if !child_best.map_or(false, |b| {
                    self.simulate(pos, child_path_code, Outcome::Win, b.best_move, sim_path, sim_stack, sim_nodes)
                }) {
                    ok = false;
                }
                pos.undo_move(mv);
                if !ok {
                    break;
                }
            }
            ok
        }
    };

    sim_stack.pop();
    sim_path.remove(&key);
    ok
}
```

`find_result_for_path` on `TtEntry` returns a result that is either:

- the base entry if `outcome == expected`, `repetition_seen == false`, and `depth <= u32::MAX`; or
- a twin with `outcome == expected` and `path_code == path_code`.

Constants: `SIM_MAX_DEPTH = 1000`, `SIM_MAX_NODES = 1000` (tune if simulation fails too often on deep wins).

### 3. Storing results

`TranspositionTable::store` remains the main write path. Its logic is changed to:

- If `outcome` is `Some` and `repetition_seen` is `true`: create a twin for the supplied `path_code` and reinitialize the base to `pn=1, dn=1, outcome=None, depth=0, repetition_seen=true, best_move=Move::NONE`.
- If `outcome` is `Some` and `repetition_seen` is `false`: store the path-independent result in the base entry and clear the twin list.
- If `outcome` is `None`: store the base `pn`/`dn` bounds, keep the existing twins, and set `repetition_seen` to the propagated flag.

`dfpn` calls `store` exactly as before, but the `path_code` it passes is only meaningful when `repetition_seen == true`.

### 4. `repetition_seen` propagation

The current `select_children` sets `repetition_seen = children.iter().any(|c| c.repetition_seen)`. This is too broad: a `Win` node is path-independent if its *winning* child is path-independent, even if other explored children are path-dependent.

After `is_solved_by_children` returns, update `select_children` to set `repetition_seen` based on the selected `best_move` and the solved outcome:

- `Win`: `repetition_seen` = `children[win_idx].repetition_seen`.
- `Draw`: `repetition_seen` = `children[draw_idx].repetition_seen` (prefer a path-independent `draw_mv` in `is_solved_by_children` when possible).
- `Loss`: `repetition_seen` = `any` child is path-dependent (all children are in the proof set).
- Not solved: `repetition_seen` = `any` child has seen a repetition.

Also, `is_solved_by_children` should prefer path-independent children of the required outcome when there are ties:

- Shortest `Loss` child with `repetition_seen == false` for `Win`.
- Longest `Draw` child with `repetition_seen == false` for `Draw`.
- Longest `Win` child with `repetition_seen == false` for `Loss`.

This keeps more results in the base entry and reduces the number of twin/simulation calls.

### 5. `evaluate_child`

`evaluate_child` uses the richer `try_use_tt` result. When `try_use_tt` returns `None`, the behavior is:

- If `entry.outcome` is `Some` (path-dependent or too deep), or `entry.depth > max_depth` for an unsolved entry, use `(pn, dn) = (1, 1)` and `outcome = None`.
- Otherwise use `entry.pn`/`entry.dn` as the lower bounds and `outcome = None`.

`ChildInfo.repetition_seen` is taken from the matched `Resolved` (base path-independent = `false`, twin = `true`) or from the unsolved entry's base `repetition_seen`.

### 6. `extract_pv` with path codes

`extract_pv` must follow the correct `best_move` for the current path. It recomputes the path code as it walks:

```rust
fn extract_pv(&self, pos: &Position) -> Vec<Move> {
    let mut pv = Vec::new();
    let mut seen = HashSet::new();
    let mut current = pos.clone();
    let mut path_code = 0u64;

    for _ in 0..1000 {
        let key = current.hash();
        if seen.contains(&key) || current.outcome().is_some() {
            break;
        }
        if let Some(entry) = self.tt.probe(key) {
            let resolved = entry.best_result_for_path(path_code);
            let (mv, outcome) = match resolved {
                Some((mv, Some(_))) if mv != Move::NONE => (mv, true),
                _ => break,
            };
            seen.insert(key);
            pv.push(mv);
            current.do_move(mv);
            path_code ^= zobrist::path_random(mv, pv.len());
        } else {
            break;
        }
    }
    pv
}
```

`best_result_for_path` on `TtEntry` returns the base entry's `best_move` if `outcome` is `Some` and `repetition_seen == false`; otherwise it returns the matching twin's `best_move` if there is one.

### 7. Root thresholds and base reinitialization

The paper initializes root thresholds to `(1, 1)` and reinitializes base `pn`/`dn` to `(1, 1)` whenever a twin proof/disproof is stored. The reinitialization is already described in the storage logic.

Root thresholds `(1, 1)` apply to the single-threshold `dfpn` variant used in the paper. Our solver runs `dfpn(pos, INF, INF, max_depth, is_or_node)` once and uses the iterative shortest-win refinement in `solve_refined`. The latter is already an iterative search with `tt.clear()` between calls, so the root-threshold change is not required. However, the `dfpn` recursion keeps the threshold propagation as-is.

## File changes

### `src/search/tt.rs`

- Add `TwinEntry`, change `TtEntry` fields.
- Update `TtEntry::default`.
- Update `TranspositionTable::store` to implement base/twin logic.
- Add `TranspositionTable::store_twin`.
- Add helper methods `TtEntry::find_result_for_path` and `TtEntry::best_result_for_path` (or keep them private).
- Keep `TranspositionTable::with_mb` and `probe`/`clear` unchanged.

### `src/search/dfpn.rs`

- Add `Resolved` struct.
- Add `SIM_MAX_DEPTH` and `SIM_MAX_NODES` constants.
- Rewrite `try_use_tt` to use base/twins and simulation.
- Add `simulate` and `find_result_for_path` helpers.
- Update `evaluate_child` to use `Resolved` and `TtEntry` fields.
- Update `select_children` and `is_solved_by_children` for correct `repetition_seen` propagation and path-independent child preference.
- Update `extract_pv` to track `path_code` and pick the correct `best_move` from base/twin.
- Update `dfpn` store calls: the signature is the same, but the `path_code` argument is now the twin key for `repetition_seen` results.

### `src/zobrist.rs`

No changes. `path_random(mv, depth)` is already deterministic and order-sensitive.

### `src/position.rs`

No changes. `Outcome::flip` already exists.

### `src/main.rs`

No changes.

## Testing and verification

### Existing tests

Run the full suite before and after:

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo doc
```

Ensure these keep passing:

- `tests/test_inf.rs`
- `tests/test_plan2.rs`
- `tests/test_plan3.rs`
- `tests/test_plan4.rs`
- `src/search/dfpn.rs` unit tests

### New regression test

Add `tests/test_plan5.rs`:

```rust
use atomic_movegen::types::{Move, Square};
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

#[test]
fn black_root_report4_fen() {
    let mut pos =
        Position::from_fen("6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27").unwrap();
    let mut search = Search::new(64);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    // After f7e6, White has a forced win, so Black is lost.
    assert_eq!(outcome, Outcome::Loss, "expected black to lose, got {outcome:?}");
    let first = pv.first().copied().unwrap();
    assert_eq!(first, Move::make_move(Square::F7, Square::E6));
}

#[test]
fn white_child_f7e6_short_win() {
    let mut pos =
        Position::from_fen("6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28").unwrap();
    let mut search = Search::new(64);
    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win);
    let first = pv.first().copied().unwrap();
    assert_eq!(first, Move::make_move(Square::G8, Square::G7));
    assert_eq!(pv.len(), 3, "expected a 3-ply win");
}
```

If the result of the black-to-move FEN is actually a `draw` because `c5c4` is a valid drawing resource, the first assertion should be changed to `Outcome::Draw` and the test should verify the solver returns within the timeout and prints a consistent PV.

### Manual verification

```bash
cargo run --release -- --fen "6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27"
cargo run --release -- --fen "6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28"
cargo run --release -- --fen "rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3"
```

All three should return within 60 seconds and produce sensible PVs for `Win`/`Loss` outcomes.

## Risks and mitigations

- **Simulation correctness**: `simulate` must follow the cached proof tree exactly. If it misses a child or uses the wrong `path_code`, it may return `false` and cause unnecessary re-search, or return `true` and corrupt the result. Mitigation: unit-test `simulate` on known wins/losses/draws and on positions with repeated transpositions.
- **Twin overflow**: `MAX_TWINS = 2` may be too small for positions with many distinct paths to the same node. If simulation fails because the right twin was evicted, the solver still falls back to the base `pn`/`dn` bounds. If observed, increase `MAX_TWINS` or add an LRU eviction policy.
- **Performance overhead**: Each simulation allocates a `Position` clone and a `HashSet`. For a node with many twins this could be slow. Mitigation: bound `SIM_MAX_NODES` and `SIM_MAX_DEPTH`; if overhead is too high, reduce `MAX_TWINS` or only simulate `Win`/`Loss` twins (not `Draw`).
- **Move ordering still a bottleneck**: The `f7e6` child is an AND node with `is_or_node = false` and `vdn` ties for all unsolved children, so it falls back to static ordering. If GHI fixes the false draws but the solver still does not reach `g8g7` within 60 seconds, move ordering will be handled in a separate plan.
- **TT size**: adding `TwinEntry` increases `TtEntry` size. For the default 64 MB table this is still acceptable, but verify with a `cargo` release run on the 60-second benchmark.
- **`Draw` simulation**: `Draw` results from local repetitions are not trustworthy on new paths. The plan handles them by simulation; if a `Draw` twin fails simulation, the solver re-searches, which is safe. Terminal draws are stored in the base entry with `repetition_seen: false`.

## Summary

1. Replace `TtEntry` with a base + twin design.
2. Add `TranspositionTable::store_twin` and reinitialize base `pn`/`dn` to `(1, 1)` on twin storage.
3. Implement `simulate` to verify path-dependent `Win`/`Loss`/`Draw` results for new paths.
4. Rewrite `try_use_tt` to use base, exact twin, or simulated twin.
5. Fix `repetition_seen` propagation and `best_move` selection to prefer path-independent children.
6. Update `evaluate_child` and `extract_pv` to work with the new `TtEntry` layout.
7. Add regression tests for the failing FEN and the `f7e6` white-to-move child.
8. Verify with `cargo fmt`, `cargo clippy`, `cargo test`, `cargo doc`, and manual release runs.
