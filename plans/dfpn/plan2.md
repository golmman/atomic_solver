# Plan: Sequential DF-PN+ Solver for Atomic Chess

This is an implementation plan for replacing the current `src/search/dfpn.rs` minimax/retrograde solver with a true sequential depth-first proof-number search (`DF-PN+`), incorporating the **GHI fix** (Kishimoto & Müller, AAAI-04) and the **epsilon trick** (Pawlewicz & Lew, 2006). Parallelization is **explicitly out of scope** for this iteration.

The solver must find the exact game-theoretic outcome (`win`/`loss`/`draw`) for atomic chess, print a principal variation for decisive outcomes, and **stop unconditionally after 5 seconds**.

## 1. Background and chosen algorithm

### 1.1 DF-PN / DF-PN+

From the literature in `plans/dfpn/parallel.pdf` and `plans/dfpn/epsilon.pdf`:

- An OR node is the attacker to move. It tries to prove the position.
- An AND node is the defender to move. It tries to disprove the position.
- For a node `n`:
  - `pn(n)` estimates the difficulty of *proving* `n` (attacker win).
  - `dn(n)` estimates the difficulty of *disproving* `n` (no forced win).
- For unsolved nodes:
  - `OR node`: `pn = min child pn`, `dn = sum child dn`.
  - `AND node`: `pn = sum child pn`, `dn = min child dn`.
- Solved nodes:
  - `Win` (proven): `pn = 0`, `dn = INF`.
  - `Loss`/`Draw` (disproven): `pn = INF`, `dn = 0`.

DF-PN traverses the most-proving child using a pair of thresholds `(th_pn, th_dn)`. For an OR node, the child thresholds are computed from the two best children `n1` (best, least `pn`) and `n2` (second best). For plain DF-PN:

```text
np = min(th_pn, pn(n2) + 1) - Costpn(n, n1)
nd = th_dn - dn(n) + dn(n1)
```

For an AND node, the roles of `pn` and `dn` are symmetric:

```text
nd = min(th_dn, dn(n2) + 1) - Costdn(n, n1)
np = th_pn - pn(n) + pn(n1)
```

`DF-PN+` extends this with:

- `Hpn(pos)`, `Hdn(pos)`: initial frontier estimates (replace `(1,1)`).
- `Costpn(parent, move, child)`, `Costdn(parent, move, child)`: edge penalties that persist until the node is solved.

For the first implementation, `H = (1, 1)` and `Cost = 0` for both. This is classic DF-PN and can be extended later.

### 1.2 Epsilon trick

From `plans/dfpn/epsilon.pdf`:

Standard DF-PN sets the child `pn` threshold to `pn(n2) + 1` (or `dn(n2) + 1` in an AND node). When the transposition table is small relative to the search tree, the same child is revisited many times, and the tree must be rebuilt repeatedly. The epsilon trick uses a multiplicative factor instead of an additive one, reducing the number of recursive calls from `O(threshold)` to `O(log threshold)`.

The child thresholds for an OR node become:

```text
np = min(th_pn, ceil(pn(n2) * (1 + epsilon))) - Costpn(n, n1)
nd = th_dn - dn(n) + dn(n1)
```

For an AND node:

```text
nd = min(th_dn, ceil(dn(n2) * (1 + epsilon))) - Costdn(n, n1)
np = th_pn - pn(n) + pn(n1)
```

Choose `epsilon = 1/4` as the initial default. The paper found this value effective for DF-PN. All arithmetic is capped at `INF` and uses `saturating_add`/`saturating_sub`.

### 1.3 GHI fix

From `plans/dfpn/ghi.pdf` and `plans/dfpn/research_ghi.md`:

The transposition table is keyed by position. In a cyclic graph, a proven or disproven result may depend on the path. The Kishimoto & Müller fix:

1. **Path encoding**: a 64-bit Zobrist-style hash of the move sequence from the root to the node.
2. **Base and twin entries**: an entry has a `base` part for unsolved bounds and a list of `twin` parts for path-dependent proven/disproven results.
3. **Kawano simulation**: when a position is reached via a new path, a twin's proof/disproof is verified by a short simulation. If it succeeds, the result is reused and a new twin for the new path is stored.
4. **Reducing simulation calls**: if a node is proven/disproven without a repetition, store the result directly in the `base` entry (path-independent). Only create twins when a repetition was involved.
5. **DF-PN-specific changes**: reinitialize the `base` entry to `(1, 1)` when a twin proof/disproof is stored, and initialize the root thresholds to `(1, 1)`.

## 2. Goal and scope

### Goal

Replace the `Search` in `src/search/dfpn.rs` with a sequential DF-PN+ solver that:

1. Returns exact `Outcome` (`Win`, `Loss`, `Draw`) for the side to move at the root.
2. Uses a transposition table of proof/disproof numbers and a `solved` flag.
3. Implements the epsilon trick.
4. Implements the GHI fix.
5. Stops after 5 seconds.
6. Preserves the public API `Search::new(tt_mb)` and `Search::solve(&mut pos) -> (Outcome, Vec<Move>, u64)`.
7. Works for the test positions listed in section 7.

### Non-goals

- Parallelization (this is plan `plan2.md`; parallelization is deferred).
- Game-specific DF-PN+ heuristics beyond `H = (1, 1)` and `Cost = 0`.
- Opening books or endgame tablebases.

## 3. Data structures

### 3.1 `Outcome` mapping

`Outcome` already exists in `src/position.rs`. For DF-PN, the mapping is:

- `Win` -> `(pn, dn) = (0, INF)`
- `Loss` -> `(pn, dn) = (INF, 0)`
- `Draw` -> `(pn, dn) = (INF, 0)` (disproven for the attacker, but the `outcome` field distinguishes `Loss` from `Draw`)

This means `Loss` and `Draw` collapse to the same `pn`/`dn` pair, but the `outcome` field is the source of truth. This is acceptable because the search is from the attacker's perspective and a draw is a failed proof.

### 3.2 `TranspositionTable` entry

Add a `Twin` concept and an `outcome` field.

```rust
struct TtEntry {
    pub key: u64,
    pub best_move: Move,
    pub outcome: Option<Outcome>,
    pub pn: u64,
    pub dn: u64,
    pub generation: u32,
    pub depth: u32,
    pub valid: bool,
    pub path_code: u64,
    pub repetition_seen: bool,
    pub twins: Vec<TwinEntry>, // only for solved, path-dependent results
}

struct TwinEntry {
    pub path_code: u64,
    pub outcome: Outcome,
    pub best_move: Move,
}
```

For simplicity, the first implementation can use a single `TtEntry` with the `outcome` field and `path_code`. If a result is stored and `repetition_seen` is true, it is treated as a path-dependent twin; otherwise, the base entry stores the result. The `twins` list can be added later if needed.

### 3.3 `Search` struct

```rust
pub struct Search {
    tt: TranspositionTable,
    path: HashSet<u64>,          // current search path, for cycle detection
    path_stack: Vec<u64>,        // ancestor stack for GHI
    path_code: u64,              // Zobrist path signature
    nodes: u64,
    start: Instant,
    deadline: Instant,
    epsilon: f64,
    scorer: Box<dyn MoveScorer>,
}
```

- `tt`: transposition table.
- `path`: positions currently on the recursion stack (per search).
- `path_stack`: stack of hashes for GHI and stop checks.
- `path_code`: incremental Zobrist hash of the move sequence.
- `deadline`: `Instant::now() + Duration::from_secs(5)`.
- `epsilon`: `0.25`.

## 4. Algorithm

### 4.1 High-level search loop

```rust
pub fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
    self.nodes = 0;
    self.start = Instant::now();
    self.deadline = self.start + Duration::from_secs(5);
    self.path.clear();
    self.path_stack.clear();
    self.path_code = 0;

    let outcome = self.dfpn(pos, INF, INF);
    let pv = self.extract_pv(pos);
    (outcome, pv, self.nodes)
}
```

`dfpn` is a single recursive function that handles both OR and AND nodes. At the root, the side to move is the attacker, so the root is an OR node. The recursion flips node type each ply.

### 4.2 `dfpn(pos, th_pn, th_dn, is_or_node)`

```rust
fn dfpn(&mut self, pos: &mut Position, th_pn: u64, th_dn: u64, is_or_node: bool) -> Outcome {
    if Instant::now() >= self.deadline {
        return Outcome::Draw; // timeout: unknown, return safe default
    }

    self.nodes += 1;

    if let Some(outcome) = pos.outcome() {
        self.tt.store_terminal(pos.hash(), outcome, self.path_code);
        return outcome;
    }

    let key = pos.hash();

    if let Some(entry) = self.tt.probe(key) {
        if let Some(outcome) = self.try_use_tt(entry, self.path_code) {
            return outcome;
        }
    }

    if !self.path.insert(key) {
        return Outcome::Draw; // local repetition
    }

    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);

    if moves.is_empty() {
        self.path.remove(&key);
        self.tt.store_terminal(key, Outcome::Draw, self.path_code);
        return Outcome::Draw;
    }

    self.sort_moves(pos, &mut moves, self.tt.probe(key).map(|e| e.best_move).unwrap_or(Move::NONE));

    self.path_stack.push(key);
    let old_path_code = self.path_code;

    let outcome = loop {
        if Instant::now() >= self.deadline {
            break Outcome::Draw;
        }

        // Compute child pn/dn
        let (best_child, second_child, pn, dn, mut best_move) =
            self.select_children(pos, &moves, is_or_node);

        let solved = self.is_solved_by_children(&moves, is_or_node);
        if let Some(o) = solved {
            // update best_move to the winning/drawing move if available
            break o;
        }

        if (th_pn != INF && pn >= th_pn) || (th_dn != INF && dn >= th_dn) {
            break Outcome::Draw; // unknown, search bound exceeded
        }

        let (mv, child_pn, child_dn, _vpn, _vdn) = best_child;
        let (second_pn, second_dn) = second_child;

        let (np, nd) = if is_or_node {
            let new_th_pn = std::cmp::min(th_pn, self.epsilon_ceil(second_pn)) - cost_pn(mv);
            let new_th_dn = th_dn.saturating_sub(dn).saturating_add(child_dn);
            (new_th_pn, new_th_dn)
        } else {
            let new_th_dn = std::cmp::min(th_dn, self.epsilon_ceil(second_dn)) - cost_dn(mv);
            let new_th_pn = th_pn.saturating_sub(pn).saturating_add(child_pn);
            (new_th_pn, new_th_dn)
        };

        pos.do_move(mv);
        self.path_code ^= path_random(mv, self.path_stack.len());
        let child_outcome = self.dfpn(pos, np, nd, !is_or_node);
        self.path_code ^= path_random(mv, self.path_stack.len());
        pos.undo_move(mv);
    };

    self.path_stack.pop();
    self.path.remove(&key);
    self.path_code = old_path_code;

    self.tt.store(key, best_move, outcome, pn, dn, self.path_code, repetition_seen);
    outcome
}
```

### 4.3 Epsilon helper

```rust
fn epsilon_ceil(&self, x: u64) -> u64 {
    if x >= INF {
        INF
    } else {
        let scaled = (x as f64 * (1.0 + self.epsilon)).ceil() as u64;
        scaled.min(INF)
    }
}
```

### 4.4 Child selection

For an OR node, children are sorted by `vpn` (virtual proof number). For an AND node, by `vdn` (virtual disproof number). The virtual numbers are:

```rust
// OR node
vpn = child_pn + cost_pn(child)
vdn = child_dn + cost_dn(child)

// AND node
vpn = child_pn + cost_pn(child)
vdn = child_dn + cost_dn(child)
```

The `T` (congestion) term from the parallel paper is 0 because there is only one thread.

`select_children` returns:

- `best_child`: `(move, child_pn, child_dn, vpn, vdn)` for the most-proving child.
- `second_child`: the virtual `pn`/`dn` of the second-best child.
- `pn`, `dn`: the computed parent `pn`/`dn` from the virtual children.
- `best_move`: the move that proves/disproves the parent if already determined, or the most-proving move for continuation.

The `best_child` is the child with the smallest `vpn` (OR) or `vdn` (AND). The `second_child` is the second smallest. The `pn` and `dn` of the parent are computed from the *current* values of all children.

### 4.5 GHI handling

```rust
fn try_use_tt(&mut self, entry: &TtEntry, path_code: u64) -> Option<Outcome> {
    if let Some(outcome) = entry.outcome {
        if entry.path_code == path_code || !entry.repetition_seen {
            // exact match or path-independent result
            return Some(outcome);
        }

        // Try to simulate the proof/disproof for this path
        if self.simulate(entry, path_code) {
            return Some(outcome);
        }
    }
    None
}
```

`simulate` is a small DF-PN search that follows the `best_move` chain recorded in the entry and verifies the proof/disproof under the new path. If it succeeds, a new twin entry (or the current entry) is updated with the new `path_code`. If it fails, the entry is treated as unsolved and the base `pn`/`dn` bounds are used.

For the first pass, a simplified GHI policy can be used:

1. Include `rule50` in the Zobrist key so positions with different draw clocks are distinct.
2. Use the per-search `path` set for cycle detection.
3. Do not store `Outcome` for nodes that were reached through a cycle; only store the result when `repetition_seen` is false.
4. If a `TtEntry` has a solved `outcome` and `repetition_seen` is false, trust it.
5. If `repetition_seen` is true, do a simulation or, for the first implementation, re-search the node.

### 4.6 Timeout behavior

If the 5-second deadline is reached before the root is solved, the search returns `Outcome::Draw` (unknown). This is a safe default for a mate-search perspective because a draw is not a win. The exact `Outcome` is returned only when the root is fully proven or disproven.

## 5. File changes

### `src/zobrist.rs`

- Add `rule50` to the hash.
- Add path-random keys for GHI path encoding: a table `PATH_RANDOM[move_index][depth]` of `u64` values.
- `path_random(mv, depth)` returns the Zobrist key for a move at a given depth.

### `src/position.rs`

- Update `hash()` to include `rule50`.
- Keep `Outcome` and its `to_pn_dn` and `flip` methods.
- Add `Outcome::is_solved_for_attacker()` if needed.

### `src/search/tt.rs`

- Extend `TtEntry` with `pn`, `dn`, `outcome`, `path_code`, `repetition_seen`, and `twins`.
- Add `store(key, best_move, outcome, pn, dn, path_code, repetition_seen)`.
- Add `probe(key) -> Option<&TtEntry>`.
- Reinitialize `pn`/`dn` to `(1, 1)` when a path-dependent solved result is stored.

### `src/search/dfpn.rs`

- Replace minimax `Search` with DF-PN+ `Search`.
- Implement `dfpn`, `select_children`, `epsilon_ceil`, `try_use_tt`, `simulate`, `extract_pv`.
- Keep `outcome_from_pn_dn` for compatibility.
- Add `EPSILON` constant (`0.25`).

### `src/search/ordering.rs`

- Keep `MoveScorer` and `StaticAtomicScorer`.
- Use `MoveScorer` to sort moves in `dfpn`.
- Generate `move_index` for `path_random` (e.g., `from_sq * 64 + to_sq + promotion * 64 * 64`).

### `src/main.rs`

- No changes needed if the `Search` API is preserved.
- Optionally add a `--time` flag, but default to 5 seconds.

## 6. Testing and verification

### 6.1 Existing tests

Keep `tests/test_inf.rs` passing:

- `4k3/8/8/8/8/8/8/4R1K1 w - - 0 1` -> Win
- `4k3/8/8/8/8/8/8/4R1K1 b - - 0 1` -> Draw
- `4k3/8/8/8/8/8/8/4K3 w - - 0 1` -> Draw
- `4k3/8/8/8/8/8/8/4K3 b - - 0 1` -> Draw
- `8/8/8/8/4k3/8/4K3/8 w - - 0 1` -> Draw
- `4k3/8/8/8/8/8/8/8 w - - 0 1` -> Loss
- `4k3/8/8/8/8/8/8/8 b - - 0 1` -> Win

### 6.2 New test positions

Add these positions to `tests/test_inf.rs` or a new `tests/test_dfpn.rs`:

**Mate in 4:**

```text
rnbqkbnr/ppppp1pp/5p2/8/8/4P3/PPPP1PPP/RNBQKBNR w KQkq - 0 2
```

**Mate in 3:**

```text
rnbqkbnr/ppppp1pp/5p2/7Q/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 2
rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3
```

**Mate in 2:**

```text
rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3
rnbqkbnr/ppp1p2p/3p1pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 4
```

**Mate in 1:**

```text
rnbqkbnr/ppp1pQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 4
rnbq1bnr/pppkpQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR w KQ - 2 5
```

**Win for white with exploded black king:**

```text
rnb3nr/ppp4p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR b KQ - 0 5
```

**Draw - only two kings remain:**

```text
4k3/8/8/8/8/8/8/4K3 w - - 0 1
```

### 6.3 Verification commands

After implementation, run:

```bash
cargo fmt
cargo clippy
cargo test
cargo doc
```

Also run `cargo run -- --fen <FEN>` for each test position and verify:

- The outcome is correct.
- The solver returns within 5 seconds.
- A principal variation is printed for `Win`/`Loss` outcomes.

## 7. Risks and mitigations

- **GHI complexity**: Full simulation is complex. Mitigation: first implement `rule50` in the hash and the per-search `path` set; add twin/simulation only if incorrect results appear.
- **Draw handling**: `Loss` and `Draw` both map to `(INF, 0)`. Mitigation: keep `outcome` as the source of truth and distinguish `Loss` from `Draw` by child outcomes.
- **Timeout**: If the search hits the 5-second limit, it returns `Draw`. This is safe for mate search but may be confusing for positions that are actually losses. Mitigation: add an `Unknown` outcome or a `timeout` flag in the future; for now, document that `Draw` on timeout means "not proven".
- **INF overflow**: Use `saturating_add`/`saturating_sub` and `INF = 1 << 60`.
- **Recursion depth**: DF-PN can recurse deeply. Use `std::thread::Builder` with a large stack if running on a separate thread, or use an iterative approach.

## 8. Summary of changes

1. Add `rule50` and path-random keys to `zobrist.rs`.
2. Extend `TtEntry` to support `pn`/`dn` bounds, `outcome`, and GHI path metadata.
3. Implement sequential DF-PN+ with epsilon trick in `src/search/dfpn.rs`.
4. Add GHI-safe lookup and storage.
5. Add 5-second hard timeout.
6. Add tests for the provided positions.
7. Verify with `cargo fmt`, `cargo clippy`, `cargo test`, and `cargo doc`.
