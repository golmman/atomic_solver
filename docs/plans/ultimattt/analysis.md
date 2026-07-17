# Analysis of `nelhage/ultimattt` DFPN for the atomic-chess solver

## 1. What `ultimattt` is

`ultimattt` is a Rust solver for [Ultimate Tic-Tac-Toe](https://en.wikipedia.org/wiki/Ultimate_tic-tac-toe). It contains several search engines, all built around proof/disproof numbers:

- `pn` — a vanilla proof-number search tree.
- `dfpn` — a sequential depth-first proof-number search.
- `pn-dfpn` — a hybrid central tree with a job pool and worker threads.
- `spdfpn` — a shared-virtual-table parallel DFPN.
- `endgame` — a fast positional analyzer that can prove or prune many positions without search.

The engine is intended to solve the game, so it has a strong focus on fixed-size transposition tables, work-limited iterative deepening, checkpointing, and detailed metrics.

## 2. Key DFPN techniques in `ultimattt`

### 2.1. `Bounds` and the threshold formula

`ultimattt` uses the classic pair `(phi, delta)` (proof / disproof numbers) with `INFINITY = 1 << 31`. A node is `Bounds::winning()` when `phi = 0` and `delta = INFINITY`, and `losing()` when the roles are swapped.

For a node with sorted children, the `dfpn` core in `src/lib/prove/dfpn.rs` computes child thresholds with:

```rust
pub fn thresholds(epsilon: f64, bounds: Bounds, nd: Bounds, phi_1: u32, delta_2: u32) -> Bounds {
    Bounds {
        phi: bounds.delta + phi_1 - nd.delta,
        delta: min(
            bounds.phi,
            max(delta_2 + 1, (delta_2 as f64 * (1.0 + epsilon)) as u32),
        ),
    }
}
```

The `+1` term is the standard DFPN device that forces the search to try the second-best child when the current child has caught up. The `1+epsilon` factor is a small multiplier (`epsilon = 1/8` by default) that avoids spending too much work on marginally better children. This is a tighter, more principled threshold than a simple `ceil(second * 1.25)`, especially for large proof numbers.

### 2.2. `select_child` and the `child` field

`ultimattt` keeps a `child: u8` in each transposition-table entry. `select_child` first checks whether the previously selected child is still within the new threshold. If so, it re-uses that child instead of recomputing the argmin. This reduces the well-known DFPN sibling-thrashing problem.

```rust
if data.child != std::u8::MAX && data.child != (idx as u8) {
    let child_bounds = thresholds(...);
    if children[data.child as usize].entry.bounds.delta < child_bounds.delta {
        return (data.child as usize, child_bounds);
    }
}
data.child = idx as u8;
```

### 2.3. Work-limited iterative deepening (MID)

The sequential `dfpn` driver does not call a single monolithic search. Instead it calls `mid` in work-bounded chunks of `CHECK_TICK_WORK = 500_000` nodes:

```rust
while !root.bounds.solved() {
    let (out, this_work, _) = worker.mid(
        Bounds { phi: INFINITY/2, delta: INFINITY/2 },
        CHECK_TICK_WORK,
        root,
        &self.root,
    );
    root = out;
    work += this_work;
    // check time limit, dump table, print debug info, etc.
}
```

Each `mid` call returns as soon as its local work budget is exhausted, even if the node is not solved. This makes time limits, progress dumps, and cancellation very cheap and predictable.

### 2.4. Transposition-table replacement policy

The table entry is small (`Bounds`, `hash`, `work`, `sync`, `pv`, `child`) and the replacement policy in `src/lib/table.rs` is:

```rust
fn better_than(&self, other: &Entry) -> bool {
    if self.hash == other.hash {
        if self.bounds.solved() != other.bounds.solved() {
            return self.bounds.solved();
        }
    }
    self.work >= other.work
}
```

A newly stored entry replaces an existing one if it is solved when the old one is not, or if it has accumulated more search work. This keeps the most-expensive subtrees in the table rather than the most-recently seen ones.

### 2.5. Endgame / positional analysis

`src/lib/endgame.rs` implements a fast UTTT-specific analyzer. It can prove win/loss/draw for many positions by checking critical boards, forced sends, and global win masks. It is used in `dfpn` in three places:

1. At the start of `mid` to solve terminal / forced positions immediately.
2. During child generation to skip moves that are proven losses for the mover.
3. To seed `ttlookup_or_default` with non-unity bounds for new children.

### 2.6. Move ordering and early termination

Inside `dfpn::mid`, children are generated one by one. The loop:

```rust
for m in pos.all_moves() {
    let eval = analysis.evaluate_move(m);
    if eval.is_won(pos.player().other()) && children.len() > 0 {
        continue;             // proven losing move, skip if we have an alternative
    }
    let g = pos.make_move(m).expect(...);
    let data = self.ttlookup_or_default(&g, eval);
    children.push(...);
    if bounds.delta == 0 {    // first child is a proven win: stop generating
        break;
    }
}
```

This is a powerful combination: filter proven-losing moves, use the TT to seed bounds, and stop move generation as soon as a winning child is found.

### 2.7. PV extraction and table checkpoints

`ultimattt` can dump the entire transposition table to disk (`--dump-table`) and reload it (`--load-table`). This is useful for very long solves. The PV extractor follows the stored `pv` move in each entry and verifies parity. It also prints histograms of branch factor, open boards, TT hit rates, and endgame hits.

### 2.8. Parallel engines

`pn-dfpn` (`src/lib/prove/pn_dfpn.rs`) and `spdfpn` (`src/lib/prove/spdfpn.rs`) are two parallel variants. They share:

- A thread-safe `ConcurrentTranspositionTable` with per-entry `AtomicU32` locks.
- A `Node` pool / allocator to represent the central tree.
- Virtual bounds (`vbounds`) and a job queue to split subtrees across worker `mid` calls.
- A `max_work_per_job`/`split_threshold` parameter to decide when a worker must descend further before returning.

These use `unsafe` for the lock and node-pool internals, which is acceptable for `ultimattt` but conflicts with the `AGENTS.md` preference for safe Rust unless there is a measured performance win.

## 3. Which techniques are useful for `atomic_solver`?

### 3.1. Directly applicable

| Technique | `ultimattt` location | Relevance to `atomic_solver` | Effort |
|-----------|----------------------|------------------------------|--------|
| Tight threshold formula with `+1` and `1+epsilon` | `src/lib/prove/dfpn.rs` `thresholds()` | `src/search/dfpn.rs` already has the same structure but uses `epsilon_ceil(second)` without an explicit `+1` floor. Adopting `max(second+1, second*(1+epsilon))` is a small change and can reduce over-expansion. | Low |
| Best-child stability (`child` field) | `src/lib/prove/dfpn.rs` `select_child()` | `Search` currently recomputes `best_and_second_unsolved` from scratch every iteration. Storing the previously selected child index in `TtEntry` or in `Search` reduces thrashing. | Low |
| Work-based TT replacement | `src/lib/table.rs` `better_than()` | `src/search/tt.rs` uses a 2-slot bucket and always replaces the primary entry. Adding a `work` counter and a "solved beats unsolved" rule would retain expensive subtrees. | Low |
| Work-limited root loop | `src/lib/prove/dfpn.rs` `DFPN::run()` | `solve()` currently calls `dfpn` once and checks the wall-clock every node. A `max_work` per top-level `dfpn` call would make progress reporting, time slicing, and future parallel splitting cleaner. | Low |
| Persistent table dump/load | `src/lib/prove/dfpn.rs` `dump_table()` / `table.rs` `from_reader()` | Not present in `atomic_solver`. Adding a `--dump-table` / `--load-table` would help long-running positions. | Low |
| Early termination of child generation | `src/lib/prove/dfpn.rs` `mid()` | `select_children()` in `src/search/dfpn.rs` evaluates all children before deciding. If a child is already a proven win, the parent is solved and the remaining siblings can be skipped. | Medium |
| Rich metrics and histograms | `src/lib/prove/dfpn.rs` `Stats`, `src/lib/util.rs` | `atomic_solver` only counts `nodes`. Adding branch-factor histograms, TT hit rates, and endgame stats would help tuning. | Low |

### 3.2. Applicable with domain adaptation

- **Endgame / tactical analyzer**: `ultimattt` has a dedicated UTTT analyzer. `atomic_solver` has `position.rs` `outcome()` for terminal / rule-50 / no-commoner positions. A stronger atomic-chess analyzer could recognize immediate forced blasts (e.g. "capture the opponent's last commoner in one move" or "this move leaves our last commoner en prise") and seed bounds or prune moves. This requires careful correctness proofs because atomic captures are explosive, but the pattern is valuable.

- **Move ordering**: `ultimattt` uses `analysis.evaluate_move` to penalize losing moves. `src/search/ordering.rs` already has a rich static scorer and the recent plan 6 additions (history, killers, TT best-move ordering). The next step is to integrate a *proven* tactical evaluation: a move that allows the opponent to blast the last commoner should be scored extremely low (or skipped once a non-losing alternative exists).

- **Parallel search**: `pn-dfpn` and `spdfpn` are concrete Rust implementations of parallel DFPN. `atomic_solver` already has `plans/dfpn/research_parallel.md` based on Kaneko's paper. The `ultimattt` code is an alternative reference, especially the `ConcurrentTranspositionTable` and the `max_work_per_job` splitting rule. Any port would need a thread-safe `Position` clone and a thread-safe `TranspositionTable`; the `AGENTS.md` rule on `unsafe` means we should prefer `std` sync primitives or existing crates over a hand-rolled `unsafe` lock scheme.

### 3.3. Not directly transferable

- The actual UTTT board representation, move notation, and the SIMD win-mask logic in `src/lib/endgame.rs` are specific to Ultimate Tic-Tac-Toe.
- The `ultimattt` definition of a draw (treated as a win for the non-to-move player because the solver is trying to prove a forced win for the root side) does not map to the three-outcome `Win/Draw/Loss` model used in `atomic_solver`.

## 4. Concrete recommendations

1. **Tighten the threshold formula**. In `src/search/dfpn.rs`, replace the single `epsilon_ceil` with the two-term formula used by `ultimattt`:
   ```rust
   fn new_threshold(second: u64, th: u64, epsilon: f64) -> u64 {
       let scaled = (second as f64 * (1.0 + epsilon)).ceil() as u64;
       std::cmp::min(th, std::cmp::max(second.saturating_add(1), scaled))
   }
   ```
   and consider lowering `EPSILON` toward `0.125`.

2. **Track a `best_child_idx` / `child` field**. Add a `best_child: u8` to `TtEntry` (or keep it transiently in `Search`) and reuse it in `select_children` when it still satisfies the new threshold. This is the most effective anti-thrashing measure in `ultimattt`.

3. **Add a `work` counter to `TtEntry`**. Use `work` (or `nodes` searched under the subtree) as a tie-breaker for replacement; keep solved results and high-work subtrees, evict fresh, cheap leaves.

4. **Batch work at the top level**. Refactor `solve()` to call `dfpn` with a `max_work` budget rather than a single unbounded call. This is the entry point to both predictable time checks and to future parallel splitting.

5. **Exit early on a proven winning child**. In `select_children`, if a child returns `Outcome::Win` for the parent, return immediately without evaluating the rest of the siblings. This matches `ultimattt` `break` on `bounds.delta == 0`.

6. **Add a persistent table dump/load**. This is low effort and enables resuming long searches; the `ultimattt` binary format is simple (header + index + entries) and can be adapted to `TtEntry`.

7. **Build a small atomic-chess tactical analyzer**. Start with a proven one-move "can the opponent blast our last commoner?" test. Use it to penalize moves in `ordering.rs` and, when a proven win exists, to skip losing moves. This is the atomic analogue of `ultimattt`'s `endgame::Analysis`.

8. **Parallelization** is the highest-impact but highest-cost item. Keep it as a future milestone; the `ultimattt` `pn-dfpn` and the `plans/dfpn/research_parallel.md` paper together give two concrete designs. The existing `ConcurrentTranspositionTable` in `ultimattt` is a good reference for a lock-per-bucket table, but a safe Rust port should avoid the `unsafe` `AtomicEntry` path unless profiling proves it necessary.

## 5. Bottom line

`ultimattt` is a mature, well-instrumented DFPN solver. The ideas most worth stealing first are the tightened threshold formula, the best-child stability field, the work-based transposition-table replacement, and the work-bounded iterative-deepening loop. These are small, safe changes that fit directly into the existing `atomic_solver` architecture. The endgame analyzer and the parallel `pn-dfpn` are valuable but require domain-specific work and a larger engineering effort.
