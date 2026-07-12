# Report: Building a Parallel Depth-First Proof-Number Search in Rust

This report is based on the paper **"Parallel Depth First Proof Number Search"** by Tomoyuki Kaneko (AAAI-10, `plans/dfpn_parallel/AAAI10-027.pdf`). The paper presents a shared-memory multi-agent parallelization of the depth-first proof-number search (DF-PN) algorithm. The goal is to provide a practical guide to implementing the algorithm in Rust, using the data structures and concurrency patterns that naturally map to the paper's design.

## 1. Source summary

- **Author / venue:** Tomoyuki Kaneko, AAAI-10.
- **Problem:** Solving large AND-OR tree problems (in the paper, shogi checkmate problems).
- **Algorithm:** DF-PN with enhancements (DF-PN+).
- **Parallelization:** Multiple independent agents each run the same DF-PN routine, sharing a single transposition table.
- **Key ideas for cooperation:**
  - Virtual proof numbers `vpn` and virtual disproof numbers `vdn` add a congestion term `T(n,c)` to a child's `pn`/`dn`.
  - `T(n,c)` is the number of agents currently working inside the subtree of child `c`.
  - `Mark`/`Unmark` track the set of active agents per node and maintain a per-thread ancestor stack.
  - When a node is proven or disproven, its Zobrist hash is published to a shared stop set; agents check their ancestor stack at safe points and unwind if an ancestor has been resolved.
  - The transposition table is protected with per-node locks; `Lookup` and `Store` are atomic with respect to other agents.

The paper reports roughly 3.6x speedup on 8 threads with <15% search overhead for large shogi checkmate problems.

## 2. DF-PN background

### 2.1 Proof and disproof numbers

For a node `n` we maintain a pair `(pn(n), dn(n))`:

- `pn(n)` estimates the difficulty of proving `n` (showing it is a win for the attacker).
- `dn(n)` estimates the difficulty of disproving `n`.

A frontier node is initialized with `(1, 1)` in pure DF-PN, or with heuristic estimates `Hpn`/`Hdn` in DF-PN+. A solved node is:

- **proven:** `(0, INF)`
- **disproven:** `(INF, 0)`

For nodes in between, `pn`/`dn` are computed from children assuming the search space is a tree:

```text
OR node:  pn = min child pn        dn = sum child dn
AND node: pn = sum child pn        dn = min child dn
```

### 2.2 Threshold-based search

DF-PN walks down the *most-proving* child using a pair of thresholds `(th_pn, th_dn)`. The idea is to expand the most-proving node until either `pn(n) >= th_pn` or `dn(n) >= th_dn`. The child thresholds are computed so that the parent threshold is reached as soon as the child has done enough work. For an OR node `n` with selected most-proving child `n1` and second-best `n2`:

```text
np = min(th_pn, pn(n2) + 1) - Costpn(n, n1)
nd = th_dn - dn(n) + dn(n1)
```

The `+1` on `pn(n2)` is the standard trick: when `n1` becomes no better than `n2`, the search switches to `n2`. `Costpn` and `Costdn` are edge penalties in DF-PN+; for plain DF-PN they are `0`. For an AND node the roles of `pn` and `dn` are symmetric.

## 3. DF-PN+ enhancements

DF-PN+ introduces two evaluation functions:

- `Hpn(pos)`, `Hdn(pos)`: initial estimates for frontier nodes, replacing `(1,1)`.
- `Costpn(parent, move, child)`, `Costdn(parent, move, child)`: edge penalties that persist until the node is solved.

The virtual numbers used to select the most-proving child in the parallel version are:

```text
OR node:  vpn(c) = pn(c) + Costpn(n,c) + T(n,c)        vdn(c) = dn(c) + Costdn(n,c)
AND node: vdn(c) = dn(c) + Costdn(n,c) + T(n,c)        vpn(c) = pn(c) + Costpn(n,c)
```

`T(n,c)` is the congestion term (number of agents inside `c`'s subtree). In a pure single-agent DF-PN+ or DF-PN, `T(n,c) = 0`.

For an initial implementation, set `Hpn = Hdn = 1` and all costs to `0`. This is the classic DF-PN.

## 4. Parallel DF-PN+ outline

### 4.1 Driver

```text
ParallelCheckmateSearch(n) {
    parallel for each thread (tid)
        OrNodePar(n, INF, INF, tid)
    return (pn(n), dn(n)) stored in table
}
```

### 4.2 Agent loop

```text
OrNodePar(n, th_pn, th_dn, tid) {
    Mark(n, tid)
    while (true) {
        for each child c {
            lock(c)
            (pn(c), dn(c)) = Lookup(c) or Hpn(c)/Hdn(c)
            pn(c) += Costpn(n,c) + T(n,c)      // virtual pn
            dn(c) += Costdn(n,c)                // virtual dn
            unlock(c)
        }
        compute pn(n) and dn(n) from the virtual numbers
        if (pn(n) >= th_pn or dn(n) >= th_dn) break
        identify n1 (least vpn) and n2 (second least vpn)
        np = min(th_pn, pn(n2)+1) - Costpn(n,n1)
        nd = th_dn - dn(n) + dn(n1)
        AndNodePar(n1, np, nd, tid)
        if (stopped by other agents) break
    }
    lock(n)
    unless (n was (dis)proven by other agents) Store(n, pn(n), dn(n))
    unlock(n)
    Unmark(n, tid)
}
```

`AndNodePar` is defined symmetrically. The paper's only additions are:

- `lock`/`unlock` around `Lookup`/`Store`.
- `Mark(n, tid)` and `Unmark(n, tid)` (extended with thread id).
- `T(n,c)` added to `vpn`/`vdn`.
- `if (stopped by other agents) break` after a recursive call.
- `Unmark` checks `if (pn(n) == 0 or dn(n) == 0)` (in practice `is_solved`) and notifies other agents to stop.

### 4.3 `Mark`/`Unmark` and `T`

Each transposition table entry tracks how many agents are currently inside that node. `Mark(n)` increments that counter; `Unmark(n)` decrements it. The counter is `T(parent, n)` for the parent selecting a child. The same `Mark`/`Unmark` also pushes/pops the node's Zobrist hash onto a per-thread ancestor stack, used by the stop mechanism.

### 4.4 Stop mechanism

When a node is solved, its Zobrist hash is asynchronously published. Each agent tests the published hash against its current ancestor stack. If a hash on the stack has been published, that ancestor is solved and the current subtree is irrelevant, so the agent unwinds. Testing is done at the start of `OrNodePar`/`AndNodePar` and after each recursive call returns.

## 5. Rust implementation

This section provides a complete, `std`-only skeleton. The design intentionally mirrors the paper's `Lookup`/`Store`, `Mark`/`Unmark`, and `T`/`stop` mechanisms.

### 5.1 Constants and traits

```rust
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

const INF: u32 = u32::MAX / 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameValue {
    Win,
    Loss,
    Draw,
}

pub trait Position: Clone + Hash + Eq + Send + Sync {
    type Move: Copy;
    fn is_terminal(&self) -> Option<GameValue>;
    fn generate_moves(&self) -> Vec<Self::Move>;
    fn make_move(&self, mv: Self::Move) -> Self;
    fn hash(&self) -> u64;
}

pub trait Eval<P: Position>: Send + Sync {
    /// Initial (pn, dn) for a frontier node.
    fn h(&self, pos: &P) -> (u32, u32);
    /// Edge penalty for pn and dn.
    fn cost_pn(&self, pos: &P, mv: P::Move, child: &P) -> u32;
    fn cost_dn(&self, pos: &P, mv: P::Move, child: &P) -> u32;
}

/// Plain DF-PN: H=(1,1) and no costs.
pub struct BasicEval;

impl<P: Position> Eval<P> for BasicEval {
    fn h(&self, _pos: &P) -> (u32, u32) { (1, 1) }
    fn cost_pn(&self, _pos: &P, _mv: P::Move, _child: &P) -> u32 { 0 }
    fn cost_dn(&self, _pos: &P, _mv: P::Move, _child: &P) -> u32 { 0 }
}
```

### 5.2 Solved-state helper

```rust
fn is_solved(pn: u32, dn: u32) -> bool {
    pn == 0 || dn == 0 || pn >= INF || dn >= INF
}

fn terminal_numbers(value: GameValue) -> (u32, u32) {
    match value {
        GameValue::Win => (0, INF),
        GameValue::Loss | GameValue::Draw => (INF, 0),
    }
}
```

### 5.3 Transposition table entry

```rust
struct EntryData {
    pn: u32,
    dn: u32,
}

struct EntryInner {
    key: u64,
    data: RwLock<EntryData>,
    active: AtomicU32,
}

impl EntryInner {
    fn new(key: u64, pn: u32, dn: u32) -> Self {
        Self {
            key,
            data: RwLock::new(EntryData { pn, dn }),
            active: AtomicU32::new(0),
        }
    }

    fn mark(&self) {
        self.active.fetch_add(1, Ordering::Relaxed);
    }

    fn unmark(&self, pn: u32, dn: u32, stop: &StopSet) {
        self.active.fetch_sub(1, Ordering::Relaxed);
        if is_solved(pn, dn) {
            stop.notify(self.key);
        }
    }

    fn read(&self) -> (u32, u32, u32) {
        let d = self.data.read().unwrap();
        let active = self.active.load(Ordering::Relaxed);
        (d.pn, d.dn, active)
    }

    fn store(&self, pn: u32, dn: u32) {
        let mut d = self.data.write().unwrap();
        if is_solved(d.pn, d.dn) {
            return;
        }
        d.pn = pn;
        d.dn = dn;
    }
}
```

### 5.4 Transposition table

```rust
struct TranspositionTable {
    map: RwLock<HashMap<u64, Arc<EntryInner>>>,
}

impl TranspositionTable {
    fn new() -> Self {
        Self { map: RwLock::new(HashMap::new()) }
    }

    fn get(&self, key: u64) -> Option<Arc<EntryInner>> {
        self.map.read().unwrap().get(&key).cloned()
    }

    fn get_or_create(&self, key: u64, pn: u32, dn: u32) -> Arc<EntryInner> {
        if let Some(e) = self.get(key) {
            return e;
        }
        let mut map = self.map.write().unwrap();
        map.entry(key)
            .or_insert_with(|| Arc::new(EntryInner::new(key, pn, dn)))
            .clone()
    }
}
```

For production, replace the global `RwLock` with a sharded table (e.g. `parking_lot` `Mutex` per bucket, or `DashMap`) and cap the size with a fixed-size table and replacement policy.

### 5.5 Stop set

```rust
struct StopSet {
    set: Mutex<HashSet<u64>>,
    epoch: AtomicUsize,
}

impl StopSet {
    fn new() -> Self {
        Self {
            set: Mutex::new(HashSet::new()),
            epoch: AtomicUsize::new(0),
        }
    }

    fn notify(&self, key: u64) {
        self.set.lock().unwrap().insert(key);
        self.epoch.fetch_add(1, Ordering::Relaxed);
    }

    fn has_ancestor(&self, stack: &[u64]) -> bool {
        let set = self.set.lock().unwrap();
        stack.iter().any(|h| set.contains(h))
    }

    fn epoch(&self) -> usize {
        self.epoch.load(Ordering::Relaxed)
    }
}
```

### 5.6 Shared state and agent

```rust
struct SharedState<P: Position> {
    tt: TranspositionTable,
    stop: StopSet,
    eval: Arc<dyn Eval<P>>,
}

struct Agent<P: Position> {
    shared: Arc<SharedState<P>>,
    tid: usize,
    stack: Vec<u64>,
    last_epoch: usize,
}

impl<P: Position> Agent<P> {
    fn check_stop(&mut self) -> bool {
        let epoch = self.shared.stop.epoch();
        if epoch == self.last_epoch {
            return false;
        }
        self.last_epoch = epoch;
        self.shared.stop.has_ancestor(&self.stack)
    }

    fn lookup_or_create(&self, pos: &P, key: u64) -> Arc<EntryInner> {
        if let Some(e) = self.shared.tt.get(key) {
            return e;
        }
        let (h_pn, h_dn) = self.shared.eval.h(pos);
        self.shared.tt.get_or_create(key, h_pn, h_dn)
    }
}
```

### 5.7 `OrNode` and `AndNode`

```rust
fn or_node<P: Position>(agent: &mut Agent<P>, pos: &P, th_pn: u32, th_dn: u32) -> (u32, u32) {
    if agent.check_stop() {
        return (0, 0);
    }

    if let Some(value) = pos.is_terminal() {
        let (pn, dn) = terminal_numbers(value);
        let entry = agent.lookup_or_create(pos, pos.hash());
        entry.store(pn, dn);
        return (pn, dn);
    }

    let key = pos.hash();
    let entry = agent.lookup_or_create(pos, key);
    {
        let (pn, dn, _) = entry.read();
        if is_solved(pn, dn) {
            return (pn, dn);
        }
    }

    entry.mark();
    agent.stack.push(key);

    let (mut pn, mut dn) = (INF, 0);
    loop {
        if agent.check_stop() {
            break;
        }

        let moves = pos.generate_moves();
        if moves.is_empty() {
            // No legal moves and not terminal: treat as a disproof for the OR player.
            pn = INF;
            dn = 0;
            break;
        }

        let mut best = None;          // (vpn, index, vdn)
        let mut second_vpn = INF;
        let mut vdn_sum = 0u32;
        let mut children = Vec::with_capacity(moves.len());

        for (i, mv) in moves.iter().enumerate() {
            let child = pos.make_move(*mv);
            let child_key = child.hash();
            let child_entry = agent.lookup_or_create(&child, child_key);
            let (cpn, cdn, active) = child_entry.read();

            let cost_pn = agent.shared.eval.cost_pn(pos, *mv, &child);
            let cost_dn = agent.shared.eval.cost_dn(pos, *mv, &child);

            let vpn = cpn.saturating_add(cost_pn).saturating_add(active);
            let vdn = cdn.saturating_add(cost_dn);

            vdn_sum = vdn_sum.saturating_add(vdn);

            match best {
                None => best = Some((vpn, i, vdn)),
                Some((b, _, _)) if vpn < b => {
                    second_vpn = b;
                    best = Some((vpn, i, vdn));
                }
                Some((b, _, _)) if vpn < second_vpn => {
                    second_vpn = vpn;
                }
                _ => {}
            }

            children.push((*mv, child, child_entry, vpn, vdn));
        }

        let (best_vpn, best_idx, best_vdn) = best.unwrap_or((INF, 0, 0));
        pn = best_vpn;
        dn = vdn_sum;

        if is_solved(pn, dn) || pn >= th_pn || dn >= th_dn {
            break;
        }

        let (mv1, child1, _, _, _) = children.swap_remove(best_idx);
        let pn2 = if children.is_empty() { INF } else { second_vpn };

        let cost_pn = agent.shared.eval.cost_pn(pos, mv1, &child1);
        let cost_dn = agent.shared.eval.cost_dn(pos, mv1, &child1);

        let np = std::cmp::min(th_pn, pn2.saturating_add(1)).saturating_sub(cost_pn);
        let nd = th_dn.saturating_sub(dn).saturating_add(best_vdn);

        and_node(agent, &child1, np, nd);

        if agent.check_stop() {
            break;
        }
    }

    entry.store(pn, dn);
    agent.stack.pop();
    entry.unmark(pn, dn, &agent.shared.stop);

    (pn, dn)
}
```

`and_node` is the same logic with the roles of `pn` and `dn` swapped and `T` applied to `vdn` instead of `vpn`:

```rust
fn and_node<P: Position>(agent: &mut Agent<P>, pos: &P, th_pn: u32, th_dn: u32) -> (u32, u32) {
    if agent.check_stop() {
        return (0, 0);
    }

    if let Some(value) = pos.is_terminal() {
        let (pn, dn) = terminal_numbers(value);
        let entry = agent.lookup_or_create(pos, pos.hash());
        entry.store(pn, dn);
        return (pn, dn);
    }

    let key = pos.hash();
    let entry = agent.lookup_or_create(pos, key);
    {
        let (pn, dn, _) = entry.read();
        if is_solved(pn, dn) {
            return (pn, dn);
        }
    }

    entry.mark();
    agent.stack.push(key);

    let (mut pn, mut dn) = (0, INF);
    loop {
        if agent.check_stop() {
            break;
        }

        let moves = pos.generate_moves();
        if moves.is_empty() {
            // No legal moves for the AND player: the OR player has already won.
            pn = 0;
            dn = INF;
            break;
        }

        let mut best = None;          // (vdn, index, vpn)
        let mut second_vdn = INF;
        let mut vpn_sum = 0u32;
        let mut children = Vec::with_capacity(moves.len());

        for (i, mv) in moves.iter().enumerate() {
            let child = pos.make_move(*mv);
            let child_key = child.hash();
            let child_entry = agent.lookup_or_create(&child, child_key);
            let (cpn, cdn, active) = child_entry.read();

            let cost_pn = agent.shared.eval.cost_pn(pos, *mv, &child);
            let cost_dn = agent.shared.eval.cost_dn(pos, *mv, &child);

            let vdn = cdn.saturating_add(cost_dn).saturating_add(active);
            let vpn = cpn.saturating_add(cost_pn);

            vpn_sum = vpn_sum.saturating_add(vpn);

            match best {
                None => best = Some((vdn, i, vpn)),
                Some((b, _, _)) if vdn < b => {
                    second_vdn = b;
                    best = Some((vdn, i, vpn));
                }
                Some((b, _, _)) if vdn < second_vdn => {
                    second_vdn = vdn;
                }
                _ => {}
            }

            children.push((*mv, child, child_entry, vdn, vpn));
        }

        let (best_vdn, best_idx, best_vpn) = best.unwrap_or((INF, 0, 0));
        dn = best_vdn;
        pn = vpn_sum;

        if is_solved(pn, dn) || pn >= th_pn || dn >= th_dn {
            break;
        }

        let (mv1, child1, _, _, _) = children.swap_remove(best_idx);
        let dn2 = if children.is_empty() { INF } else { second_vdn };

        let cost_pn = agent.shared.eval.cost_pn(pos, mv1, &child1);
        let cost_dn = agent.shared.eval.cost_dn(pos, mv1, &child1);

        let nd = std::cmp::min(th_dn, dn2.saturating_add(1)).saturating_sub(cost_dn);
        let np = th_pn.saturating_sub(pn).saturating_add(best_vpn);

        or_node(agent, &child1, np, nd);

        if agent.check_stop() {
            break;
        }
    }

    entry.store(pn, dn);
    agent.stack.pop();
    entry.unmark(pn, dn, &agent.shared.stop);

    (pn, dn)
}
```

### 5.8 Parallel driver

```rust
pub struct ParallelSearch<P: Position> {
    shared: Arc<SharedState<P>>,
    threads: usize,
}

impl<P: Position + 'static> ParallelSearch<P> {
    pub fn new(eval: Arc<dyn Eval<P>>, threads: usize) -> Self {
        Self {
            shared: Arc::new(SharedState {
                tt: TranspositionTable::new(),
                stop: StopSet::new(),
                eval,
            }),
            threads,
        }
    }

    pub fn solve(&self, root: &P) -> (u32, u32) {
        let root = root.clone();
        let mut handles = Vec::with_capacity(self.threads);

        for tid in 0..self.threads {
            let shared = Arc::clone(&self.shared);
            let root = root.clone();
            handles.push(std::thread::spawn(move || {
                let mut agent = Agent {
                    shared,
                    tid,
                    stack: Vec::new(),
                    last_epoch: 0,
                };
                or_node(&mut agent, &root, INF, INF);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let key = root.hash();
        if let Some(e) = self.shared.tt.get(key) {
            let (pn, dn, _) = e.read();
            (pn, dn)
        } else {
            (INF, INF)
        }
    }
}

pub fn outcome_from(pn: u32, dn: u32) -> Option<GameValue> {
    if dn >= INF || pn == 0 {
        Some(GameValue::Win)
    } else if pn >= INF || dn == 0 {
        Some(GameValue::Loss)
    } else {
        None
    }
}
```

`Loss` is returned for either a loss or a draw when DF-PN is used from a single goal (win) perspective. For a full `win`/`loss`/`draw` solver, run the search from the perspective of each side or add a draw outcome to the `pn`/`dn` propagation (multi-outcome proof-number search, which is not covered in this paper).

## 6. Key correctness details

### 6.1 Sums and saturation

All additions are `saturating_add`. `INF` is `u32::MAX / 2` so `INF + 1` is still well below `u32::MAX` and `saturating_add` is cheap. Threshold arithmetic uses `saturating_sub` and `saturating_add` to avoid wrap-around.

### 6.2 Lock granularity

The code above uses `std::sync::RwLock` for the whole table map and `std::sync::RwLock` per entry for `pn`/`dn`. This is correct but not the fastest:

- A `get_or_create` is a write lock on the whole table.
- A `Store` is a write lock on one entry.
- A `Lookup` is a read lock on the table and then a read lock on the entry.

For a real solver, shard the table into many `Mutex<RwLock<...>>` or `DashMap` buckets, and use `parking_lot` locks. The paper reports lock overhead around 1%, so the design is fine once the lock is not per-table.

### 6.3 `Mark`/`Unmark` and `T` accuracy

`active` is incremented by `Mark` and decremented by `Unmark`. `T(parent, child)` is `child.active` at the moment the parent reads it. This is intentionally approximate: it only needs to spread agents across different children. If `active` is read while another agent is between `Mark` and `Unmark`, the value is slightly stale, but the search remains correct because `pn`/`dn` are still updated by `Store`.

### 6.4 Stop set lifetime

The `StopSet` grows as nodes are solved. It can be pruned by removing a key when its `active` counter reaches `0` in `EntryInner::unmark`. The paper does not specify this, but the report's code already avoids `active` going negative; the implementation can add `stop.remove(key)` when `active` reaches `0` if memory becomes an issue.

### 6.5 Trees versus DAGs

DF-PN assumes the search space is a tree. For game positions (chess, shogi, atomic-chess), the same board can be reached by different move orders, so the state space is a DAG. The paper warns that this causes overestimation of `pn`/`dn` and the `Graph History Interaction` (GHI) problem. Practical fixes:

1. Use a transposition table keyed by a Zobrist hash of the full board state.
2. For `pn`/`dn` propagation, sometimes use `max` instead of `sum` when a child is shared by multiple parents (Kishimoto 2005; 2010).
3. For repetitions, keep a `path` set and treat repeated positions as a draw/disproof.

The `plans/basics/report.md` in this repo already notes that plain DF-PN did not converge for draw-heavy atomic-chess positions. A robust implementation must propagate draws explicitly and tag `tt` entries with a `solved` flag and the solved `Outcome`.

### 6.6 Garbage collection

The sequential DF-PN uses garbage collection to discard sub-trees. The parallel paper's experiments ran with GC enabled but measured overheads without it. For a Rust implementation, the simplest approach is to not `delete` entries but use a fixed-size replacement table. A `generation` or `age` field per entry helps evict old entries when the table is full. When an entry is evicted, the next `Lookup` re-initializes it with `H`.

### 6.7 Stack size and recursion

`or_node` and `and_node` are mutually recursive. For very deep checkmate sequences (100+ plies), the default thread stack may overflow. Increase it per worker:

```rust
std::thread::Builder::new()
    .stack_size(8 * 1024 * 1024)
    .spawn(move || { ... })
```

For an iterative version, maintain an explicit work stack and an `InterpretedState` enum per frame. This is harder but avoids recursion limits.

## 7. Testing plan

1. **Unit tests for the arithmetic:**
   - `is_solved` and `terminal_numbers`.
   - `pn`/`dn` computation for artificial OR/AND nodes with known children.
   - Threshold formulas, including the `+1` child-switching case.

2. **Single-thread DF-PN first:**
   - Run with `threads = 1` and `BasicEval` on small positions.
   - Verify the same node count as a sequential reference.

3. **Multi-thread consistency:**
   - Run with `threads = 1, 2, 4, 8` on the same puzzle.
   - All runs should agree on `Win`/`Loss`/`Draw` (up to `draw` interpretation).
   - Node overhead should be small (the paper reports <15% for large problems).

4. **Scaling benchmarks:**
   - Measure wall time and node count per thread count.
   - Check that `T` is actually spreading agents (verify with `active` counters or logging).

5. **Edge cases:**
   - `terminal` position as root.
   - Positions with one legal move.
   - Repeated positions / cycles.
   - `draw` outcomes (treat as disproof for a win search).

## 8. Roadmap for a real solver

1. Start with the single-threaded `or_node`/`and_node` from the skeleton, using `BasicEval` and no `T`.
2. Add a robust `Position` wrapper around `atomic-movegen` `Board` with Zobrist hashing and terminal detection (`commoners`, `rule50`, checkmate).
3. Add `Entry` `solved` flag and `Outcome` to the transposition table, because the `basics` work showed that draw propagation is necessary for atomic-chess.
4. Add parallel workers, `Mark`/`Unmark`, `active` counters, and `T`.
5. Add the `StopSet` and ancestor-stack cutoff.
6. Replace `H`/`Cost` with domain-specific heuristics (DF-PN+).
7. Replace the global `RwLock` table with a sharded/fixed-size table.
8. Add `std::thread::Builder` with a larger stack or convert to an explicit stack if recursion is too deep.

## 9. References

- Kaneko, T. (2010). *Parallel Depth First Proof Number Search*. AAAI-10.
- Nagai, A. (2001). *DF-PN Algorithm for Searching AND/OR Trees and Its Applications*. Ph.D. dissertation, University of Tokyo.
- Allis, L. V., van der Meulen, M., & van den Herik, H. J. (1994). *Proof-number search*. Artificial Intelligence, 66, 91-124.
- Kishimoto, A. (2005). *Correct and Efficient Search Algorithms in the Presence of Repetitions*. Ph.D. dissertation, University of Alberta.
- Nagai, A., & Imai, H. (1999). *Application of df-pn+ to Othello endgames*. Game Programming Workshop in Japan '99.
