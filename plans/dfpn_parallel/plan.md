# Implementation Plan: Parallel Depth-First Proof-Number Search

## Goal

Replace the current exact minimax solver in `src/search/dfpn.rs` with a true
parallel DF-PN+ solver, following the design in
`plans/dfpn_parallel/research.md` (Kaneko, *Parallel Depth First Proof Number
Search*, AAAI-10). Keep the existing public API and the passing test suite from
`tests/test_inf.rs`.

The solver must still accept a FEN via `--fen`, determine the exact
`win`/`loss`/`draw` outcome, and print a principal variation for decisive
outcomes.

## What failed last time and how to fix it

`plans/basics/report.md` says the first DF-PN attempt did not converge for
draw-heavy atomic positions:

- The threshold-doubling loop kept returning lower-bound `(pn, dn)` pairs.
- The TT lower bounds caused recursive deadlocks around repeated draws.
- Standard DF-PN child-threshold formulas do not short-circuit the
  "both `pn` and `dn` are `INF`" draw condition well in the presence of cycles.

The current code works around this with exact retrograde minimax (stored
`Outcome` + a `path` set for repetitions). The new DF-PN must keep the same
correctness guarantees while using the DF-PN+ threshold search and Kaneko's
parallel algorithm. The concrete fixes are:

1. **Store a solved `outcome` flag in the transposition table.**
   - `pn`/`dn` are lower bounds when the outcome is unknown.
   - When the outcome is `Win` they are exact `(0, INF)`.
   - When the outcome is `Loss` or `Draw` they are exact `(INF, 0)`.
   - `Draw` and `Loss` are distinguished by the `outcome` field, not by the
     `pn`/`dn` pair.

2. **Do not store `(INF, INF)` for an unsolved node.**
   - A node with `outcome = None` and `pn = dn = INF` is an unknown frontier
     that has made no progress. Keep its frontier estimate `(1, 1)` instead of
     propagating a useless infinite lower bound.

3. **Treat `INF` thresholds as "unbounded" in the loop condition.**
   - Continue searching while
     `outcome == None && !check_stop() &&
      (th_pn == INF || pn < th_pn) &&
      (th_dn == INF || dn < th_dn)`.
   - This is the root cause of the previous deadlock: with `th = INF` and
     `pn = dn = INF`, the standard `pn >= th_pn || dn >= th_dn` test would stop
     before the node is actually solved.

4. **Keep per-thread repetition detection.**
   - Each thread has its own `path` set / ancestor stack.
   - If a child is already in the current thread's path, it is a repeated
     position and counts as a local `Draw` for that branch.
   - Do not put that transient result in the TT.

5. **Put `rule50` back in the position key.**
   - The `basics` report removed it to improve TT hit rate, but the 50-move
     draw is path-dependent. Add a `rule50` Zobrist key and include it in
     `Position::hash` so the same board with different clocks is distinct.

## High-level design

The new solver is a shared-memory multi-agent DF-PN+ search:

- A single `TranspositionTable` is shared by all worker threads.
- Each node/entry has an `active` counter (`T` in the paper), a `Mutex` for
  `(pn, dn, outcome, best_move, generation)`, and helper methods
  `mark`, `unmark`, `read`, `store`.
- A global `StopSet` + per-thread ancestor stack lets threads stop as soon as
  an ancestor is proven or disproven by another worker.
- Each worker gets its own `Position` clone and its own `Agent` state.
- Workers run the same recursive `or_node`/`and_node` routine, selecting the
  most-proving child by the virtual proof numbers `vpn`/`vdn`.

For the first implementation the `Eval` trait is `BasicEval`: frontier estimate
`H = (1, 1)` and zero edge costs. This is classic DF-PN. The `Eval` trait stays
in place so domain-specific DF-PN+ heuristics can be added later without
touching the search core.

## Data structures

### `Entry` (per transposition-table node)

```rust
struct EntryData {
    pn: u64,
    dn: u64,
    outcome: Option<Outcome>,
    best_move: Move,
    generation: u32,
}

struct Entry {
    key: u64,
    active: AtomicU32,
    data: Mutex<EntryData>,
}
```

- `mark`: `active.fetch_add(1, Relaxed)`.
- `unmark`: `active.fetch_sub(1, Relaxed)`; if `outcome.is_some()`, notify the
  `StopSet`.
- `read`: lock `data`, return `(pn, dn, active, outcome)`.
- `store`: lock `data`. If the existing `outcome` is already `Some`, do not
  overwrite. If `outcome` is `None` and the new `(pn, dn)` is `(INF, INF)`
  for an unsolved node, do not write it. Otherwise write `pn`, `dn`,
  `outcome`, `best_move`, and `generation`.

### `TranspositionTable`

A sharded fixed-size or hash-map-backed table. For the first version a sharded
`Mutex<HashMap<u64, Arc<Entry>>>` is fine because it matches the paper's
`Lookup`/`Store` design and is easy to reason about. Production tuning can swap
it for a fixed-size sharded table with `parking_lot`.

- `with_mb(size)` computes the number of entries and the number of shards.
- `get_or_create(key, pn, dn) -> Arc<Entry>` returns the existing entry or
  inserts a new one initialized with `pn`/`dn` from the `Eval` heuristic.
- `get(key) -> Option<Arc<Entry>>` is used for PV extraction after the search.

### `StopSet`

```rust
struct StopSet {
    set: Mutex<HashSet<u64>>,
    epoch: AtomicUsize,
}
```

- `notify(key)`: insert `key` and bump `epoch`.
- `has_ancestor(stack) -> bool`: lock the set, check if any hash in the thread
  ancestor stack is present.
- `check_stop(agent)`: if `epoch` changed, reload and call `has_ancestor`.

### `SharedState` and `Agent`

```rust
struct SharedState {
    tt: Arc<TranspositionTable>,
    stop: StopSet,
    eval: Arc<dyn Eval>,
    move_scorer: Arc<dyn MoveScorer>,
    nodes: AtomicU64,
}

struct Agent {
    shared: Arc<SharedState>,
    tid: usize,
    path: HashSet<u64>,      // current thread's position set for repetitions
    stack: Vec<u64>,         // ancestor stack for StopSet
    last_epoch: usize,
}
```

The `Agent` does not own the `Position`; the worker thread owns the `Position`
and passes `&mut Position` to `or_node`/`and_node`.

### `Eval` and `BasicEval`

```rust
pub trait Eval: Send + Sync {
    fn h(&self, pos: &Position) -> (u64, u64);
    fn cost_pn(&self, pos: &Position, mv: Move, child: &Position) -> u64;
    fn cost_dn(&self, pos: &Position, mv: Move, child: &Position) -> u64;
}
```

`BasicEval` returns `(1, 1)` for `h` and `0` for both costs. This makes the
first version classic DF-PN.

## Algorithm

### `or_node` / `and_node` outline

Both are recursive functions that look like the skeleton in the research file,
with the addition of the `outcome` field and the solved / unknown distinction.

```rust
fn or_node(agent: &mut Agent, pos: &mut Position, th_pn: u64, th_dn: u64)
    -> (u64, u64, Option<Outcome>)
```

1. `if agent.check_stop() { return (0, 0, None); }`
2. If `pos.outcome()` is `Some`, convert it to `pn/dn` and return.
3. `key = pos.hash(); entry = agent.lookup_or_create(pos, key);`
   - If `entry.read().outcome` is `Some`, return the exact pair.
4. If `agent.path.contains(&key)`, return `(INF, 0, Some(Draw))`.
5. `entry.mark(); agent.path.insert(key); agent.stack.push(key);`
6. Re-read `entry`; if it became solved in the meantime, `unmark` and return.
7. Generate legal moves. If there are none, set `outcome = Draw`.
8. Otherwise loop:
   - `if agent.check_stop() { break; }`
   - Sort moves with the existing `MoveScorer`.
   - For each child move:
     - If the child key is in `agent.path`, treat it as `pn=INF, dn=0,
       outcome=Draw`.
     - Else `do_move`, get the child key, `lookup_or_create` the child entry,
       `undo_move`, and read the child's `(pn, dn, active, outcome)`.
     - Compute `vpn`/`vdn`:
       - `or_node`: `vpn = pn + cost_pn + active`, `vdn = dn + cost_dn`
       - `and_node`: `vdn = dn + cost_dn + active`, `vpn = pn + cost_pn`
     - Accumulate `pn`/`dn` and track `best` / `second`.
     - Track the outcome of the children: `or_node` needs a `Win` child or
       all children solved; `and_node` needs a `Loss` child or all children
       solved.
   - If `outcome` is determined (`Win` for `or_node`, `Loss`/`Draw`/`Win` for
     `and_node` based on minimax over the child `outcome`s), break.
   - If the virtual `pn`/`dn` have exceeded the finite thresholds, break with
     `outcome = None`.
   - Select the most-proving child, compute the child thresholds from the
     research paper formulas, `do_move` it, recurse to the opposite node type,
     `undo_move`, and `if agent.check_stop() { break; }`.
9. Determine the final `pn`/`dn` to store:
   - If `outcome` is `Some`, store `outcome.to_pn_dn()` and the winning/drawing
     `best_move`.
   - If `outcome` is `None`, store the virtual `pn`/`dn` and the most-proving
     `best_move`. If `pn == INF && dn == INF`, do not write it.
10. `agent.path.remove(&key); agent.stack.pop(); entry.unmark(...);` return.

The `and_node` is symmetric, swapping `pn`/`dn` and `min`/`sum` just as in the
research paper.

### Child thresholds (for `BasicEval` with zero costs)

For `or_node` with most-proving child `c1` and second-best child `c2`:

```text
np = min(th_pn, vpn(c2) + 1)
nd = th_dn - dn + vdn(c1)
```

For `and_node`:

```text
nd = min(th_dn, vdn(c2) + 1)
np = th_pn - pn + vpn(c1)
```

All arithmetic uses `saturating_add`/`saturating_sub` and caps values at `INF`.

### Parallel driver

```rust
pub struct Search { ... }

impl Search {
    pub fn new(tt_mb: usize) -> Self { ... }

    pub fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        let shared = Arc::new(SharedState { ... });
        let root = pos.clone();
        let threads = self.threads;

        let mut handles = Vec::with_capacity(threads);
        for tid in 0..threads {
            let shared = Arc::clone(&shared);
            let mut position = root.clone();
            let builder = std::thread::Builder::new()
                .stack_size(8 * 1024 * 1024);
            handles.push(builder.spawn(move || {
                let mut agent = Agent::new(shared, tid);
                or_node(&mut agent, &mut position, INF, INF);
            }).unwrap());
        }

        for h in handles { h.join().unwrap(); }

        let outcome = shared.tt.get(root.hash())
            .and_then(|e| e.read().outcome)
            .unwrap_or(Outcome::Draw);
        let pv = extract_pv(&shared.tt, &root);
        let nodes = shared.nodes.load(Ordering::Relaxed);
        (outcome, pv, nodes)
    }
}
```

Each worker starts at the root with `th = (INF, INF)`. The `INF` threshold is
handled by the unbounded loop condition above, so the search runs until the
root is actually solved.

### PV extraction

After the join, walk the `best_move` chain from the root `Position` using the
TT. Stop at a terminal position, a `NONE` move, or a repeated hash. Convert to
UCI with the existing `move_to_uci` helper.

## File-by-file changes

### `src/zobrist.rs`

- Add a `rule50_keys: [u64; 101]` table to the `Zobrist` struct.
- Change `hash` to `pub fn hash(board: &Board, rule50: u16) -> u64` and XOR in
  `rule50_keys[rule50.min(100) as usize]`.

### `src/position.rs`

- Update `do_move`, `undo_move`, and `from_fen` to call `zobrist::hash` with the
  current `rule50`.
- Keep `Position::hash()` returning `self.zobrist`.
- Optionally make `outcome()` detect stalemate (no legal moves) as `Draw`;
  otherwise `or_node`/`and_node` handle it.
- Update `Outcome::to_pn_dn` so `Draw` maps to `(INF, 0)` and keep the
  `outcome` field in `Entry` as the source of truth.

### `src/search/tt.rs`

- Replace the current `TtEntry`/`TranspositionTable` with the thread-safe
  `Entry`/`TranspositionTable` described above.
- Keep `with_mb` for sizing and add `get`/`get_or_create`.

### `src/search/dfpn.rs`

- Replace the minimax `Search` with `ParallelSearch` + `Agent` + `or_node`/
  `and_node`.
- Keep the public `Search` name and `Search::new`/`solve` signature so `main.rs`
  and the tests do not change.
- Keep `INF = 1 << 60` (or use `zobrist::INF`) and `outcome_from_pn_dn` if
  useful, but the authoritative solved result is `Entry.outcome`.

### `src/search/ordering.rs`

- Keep the `MoveScorer` trait and `StaticAtomicScorer`.
- Make `MoveScorer` extend `Send + Sync` so it can live in `SharedState`.
- Use `MoveScorer` inside `or_node` and `and_node` to sort the move list once
  per node.

### `src/main.rs`

- No changes needed if the `Search` API is preserved.

### `tests/test_inf.rs`

- No changes expected; all existing tests must still pass.

## Testing and verification

Run after every meaningful change:

```bash
cargo fmt
cargo clippy
cargo test
cargo doc
```

Specific tests to add and run:

1. **Unit tests in `src/search/dfpn.rs`:**
   - `BasicEval` returns `(1, 1)` and zero costs.
   - `Outcome`/`Entry` mapping: `Win` = `(0, INF)`, `Loss`/`Draw` = `(INF, 0)`.
   - Threshold arithmetic with `saturating_add`/`saturating_sub`.
   - `or_node`/`and_node` outcome propagation on synthetic children.

2. **Single-thread first:**
   - Set `threads = 1` and run `tests/test_inf.rs`.
   - Verify the rook mate, king-only, opposed-kings, and no-pieces cases.

3. **Multi-thread consistency:**
   - Run the same positions with `1, 2, 4, 8` threads.
   - All runs must return the same `Win`/`Loss`/`Draw`.

4. **Edge cases:**
   - Terminal position as root.
   - Stalemate with no legal moves.
   - Repeated positions (both `path` and 50-move draw).
   - Positions with `rule50` near 100 to check the key fix.

5. **Sanity check on the parallel overhead:**
   - With `threads > 1`, confirm that the `active` counters and `StopSet` are
     exercised (small log or debug counters).

## Notes and risks

- **Correctness over speed.** Do not leave `rule50` out of the key just to
  improve TT hit rate. If the hit-rate regression is large, address it later
  with a dedicated `draw-clock` aware entry or a separate key.
- **Do not store `(INF, INF)` for unsolved nodes.** This is the main mechanism
  that prevents the old deadlock.
- **Outcome is the source of truth.** `pn`/`dn` are search bounds; `Entry.outcome`
  decides whether a node is solved and whether it is `Win`/`Loss`/`Draw`.
- **Thread safety.** `Position` must be `Send` and `Clone` so each worker gets
  its own copy. No mutable `Position` is shared.
- **Stack depth.** The recursive `or_node`/`and_node` can be very deep. Use
  `std::thread::Builder::new().stack_size(8 * 1024 * 1024)` or larger for each
  worker.
- **Future DF-PN+ tuning.** Once the basic solver works, non-zero `cost`/`h`
  functions and better move ordering can be plugged into the existing `Eval`
  trait without changing the parallel search.
