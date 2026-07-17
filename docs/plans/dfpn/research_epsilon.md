# Implementing the 1 + ε Trick in a Rust DF-PN Solver

## Summary

This report extracts the technique from *Improving Depth-first PN-Search: 1 + ε Trick* (Pawlewicz & Lew, 2007) and shows how to apply it to a Rust implementation of DF-PN. The trick is a one-line change to the threshold calculation: instead of raising a child’s threshold to only one unit above the sibling’s bound, raise it by a small multiplicative factor `1 + ε`. This turns the number of recursive re-searches at a node from `O(threshold)` into `O(log threshold)`, which is especially valuable when the transposition table is much smaller than the search tree.

The `atomic_solver` codebase already contains a working implementation of this enhancement in `src/search/dfpn.rs`. This document explains the theory, the exact formulas, the Rust code changes, and practical tuning advice.

## Background: DF-PN threshold recursion

DF-PN is a depth-first variant of Proof-Number search. Each node stores a proof number `pn` and a disproof number `dn`. Internal nodes combine their children according to the usual AND/OR rules (see Figure 1 of the paper):

- **OR node** (the side to move tries to find one winning child):
  - `pn = min_i pn_i`
  - `dn = sum_i dn_i` (capped at `INF`)
- **AND node** (the opponent tries to refute all children):
  - `pn = sum_i pn_i` (capped at `INF`)
  - `dn = min_i dn_i`

A node is solved when `(pn, dn)` reaches `(0, INF)` (proven) or `(INF, 0)` (disproven). During search the algorithm walks down the *most-proving node* (MPN) and recursively re-searches children with updated thresholds. Only nodes on the path from the root to the current node keep live thresholds, which is what makes DF-PN memory-friendly.

For an **OR node** whose children are sorted by `pn_i` (so `pn_1 ≤ pn_2 ≤ … ≤ pn_n`), the threshold passed to the first child is normally:

```
pt1 = min(pt, p2 + 1)
dt1 = dt - d + d1
```

where:

- `pt`, `dt` are the parent’s PN and DN thresholds,
- `p`, `d` are the parent’s current PN and DN,
- `p1`, `d1` are the first child’s PN and DN,
- `p2` is the second child’s PN.

The analogous formulas for an **AND node** (sorted by `dn_i`) are:

```
pt1 = pt - p + p1
dt1 = min(dt, d2 + 1)
```

When the child returns, its `pn`/`dn` have usually grown, so the parent values are updated and a new MPN is chosen. If the first child remains the best, the same subtree is searched again with the threshold raised by exactly one.

## The problem: linear re-searches and table thrashing

When the search space is far larger than the transposition table, most nodes explored in a child subtree are overwritten before that child is revisited. Because the standard threshold formula only raises the bound by `+1` on every recursive call, DF-PN may call the same child `O(pt)` times before the threshold is exhausted. Each call rebuilds almost the entire child tree from scratch. This is the main weakness the paper addresses.

## The 1 + ε trick

Instead of the `+1` addend, use a small multiplicative margin:

For an **OR node**:

```
pt1 = min(pt, ceil(p2 * (1 + ε)))
dt1 = dt - d + d1
```

For an **AND node**:

```
pt1 = pt - p + p1
dt1 = min(dt, ceil(d2 * (1 + ε)))
```

where `ε` is a small positive real number.

After each recursive call, the child’s `pn` (or `dn`) grows by at least a factor of `1 + ε` (unless the parent’s own threshold is reached first). Therefore a child is re-entered at most `log_{1+ε} pt` times, which dramatically reduces repeated tree reconstruction when the transposition table is small.

The paper notes that this loses the property that DF-PN expands nodes in exactly the same order as plain best-first PN search, but in practice the gain from fewer re-searches far outweighs that theoretical property.

## Rust implementation

### 1. Add an epsilon parameter

Store `epsilon` as a `f64` in the search structure. A compile-time default is enough for a fixed solver; exposing it at runtime lets you tune per position or per hardware limit.

```rust
// src/search/dfpn.rs
const EPSILON: f64 = 0.25;

pub struct Search {
    // ... existing fields ...
    epsilon: f64,
    // ...
}

impl Search {
    pub fn new(tt_mb: usize) -> Self {
        Self {
            // ...
            epsilon: EPSILON,
            // ...
        }
    }

    // Optional runtime setter; valid range is [0.0, 1.0]
    pub fn set_epsilon(&mut self, epsilon: f64) {
        assert!(
            (0.0..=1.0).contains(&epsilon),
            "epsilon must be in [0.0, 1.0], got {epsilon}"
        );
        self.epsilon = epsilon;
    }
}
```

A `set_epsilon` method is not present in the current code, but it is the natural way to expose the parameter to the CLI or tests.

### 2. Implement `epsilon_ceil`

This helper converts a sibling bound into a threshold. It must be careful with `INF` to avoid overflow and must round up, because the threshold is an upper bound that must be strictly greater than the sibling value. The implementation additionally enforces a minimum step of `x + 1`, which makes `ε = 0.0` behave exactly like the original `+1` DF-PN threshold and guarantees progress for all valid `ε`.

```rust
fn epsilon_ceil(&self, x: u64) -> u64 {
    if x >= INF {
        return INF;
    }
    let scaled = (x as f64 * (1.0 + self.epsilon)).ceil() as u64;
    scaled.max(x.saturating_add(1)).min(INF)
}
```

The current implementation is in <ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="746-752" />.

`ε = 0.0` is therefore not a degenerate multiplicative scaling (`ceil(x * 1.0) = x`) but the strict `x + 1` threshold used by classic DF-PN. The valid range for `epsilon` remains `[0.0, 1.0]`; values outside this range are rejected by `set_epsilon`.

### 3. Update the recursive call thresholds

Inside the main `dfpn` loop, after selecting the best unsolved child and the second-best unsolved child (`selection.second_child`), compute the child’s thresholds:

```rust
let (mv, child_pn, child_dn, _vpn, _vdn) = selection.best_child;
let (second_pn, second_dn) = selection.second_child;

let (np, nd) = if is_or_node {
    // OR node: raise the PN threshold by (1 + ε) over the second-best child.
    // DN threshold is computed exactly from the parent DN budget.
    let new_th_pn = std::cmp::min(th_pn, self.epsilon_ceil(second_pn));
    let new_th_dn = if th_dn == INF {
        INF
    } else {
        th_dn.saturating_sub(dn).saturating_add(child_dn)
    };
    (new_th_pn, new_th_dn)
} else {
    // AND node: raise the DN threshold by (1 + ε) over the second-best child.
    // PN threshold is computed exactly from the parent PN budget.
    let new_th_dn = std::cmp::min(th_dn, self.epsilon_ceil(second_dn));
    let new_th_pn = if th_pn == INF {
        INF
    } else {
        th_pn.saturating_sub(pn).saturating_add(child_pn)
    };
    (new_th_pn, new_th_dn)
};
```

This matches the code in <ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="379-401" />.

### 4. Ensure child selection uses the same keys

The `select_children` routine must return:

- the best unsolved child according to `pn` for OR nodes and `dn` for AND nodes,
- the second best unsolved child (or `INF` if there is only one),
- the parent’s combined `pn` and `dn` so the threshold budget can be recomputed.

The existing `best_and_second_unsolved` function in <ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="874-924" /> does this by comparing `vpn` for OR nodes and `vdn` for AND nodes. Those virtual values are initialized to the real `pn`/`dn` in `evaluate_child`, so the selection is consistent with the threshold formulas above.

### 5. Correctness invariants

The trick does not change the correctness of DF-PN because:

- The threshold `ceil(p2 * (1 + ε))` is still an upper bound on `p2`, so the child search stops as soon as the child is no longer the most-proving child or the parent threshold is reached.
- The DN threshold `dt - d + d1` and the PN threshold `pt - p + p1` are unchanged, so the parent’s budget is still respected exactly.
- When a child is solved, the parent re-evaluates `pn`/`dn` and may pick a different child.

What changes is the *granularity* of re-search: each call is allowed to grow the child bound by a multiplicative step rather than a single unit.

## Tuning ε

The paper gives two concrete recommendations based on experiments on Atari Go and Lines of Action:

- **DF-PN**: start with `ε = 0.25` (`1/4`). Larger values can cause noticeable over-exploration of a single child; smaller values lose the logarithmic benefit and revert toward the original `+1` behavior.
- **PDS**: use a much smaller value, `ε = 1/16`. PDS already tends to spend more time in a single child because its stopping condition requires both thresholds to be exceeded simultaneously, so a large `ε` easily leads to over-exploration.

For **atomic chess** there is no published optimal value, but the same rule of thumb applies:

1. Start with `ε = 0.25`.
2. If the solver times out while the transposition table is small and the search tree is huge, try a larger `ε` (e.g., `0.5`).
3. If the solver seems to waste nodes exploring one child much deeper than necessary, reduce `ε` (e.g., `0.125` or `0.0625`).
4. Benchmark node counts and solving times over a set of hard positions with the same TT size before settling on a default.

A useful addition is a command-line flag `--epsilon <f64>` that sets `Search::set_epsilon` so you can run tuning sweeps without recompiling.

## Interaction with other enhancements

- **Move ordering**: good move ordering makes the epsilon trick more effective because the best child is likely to be tried first, and the second-best bound is a realistic estimate of how far the first child can grow before another child takes over.
- **Transposition-table replacement**: the benefit of the trick is largest when the TT is small. If the TT is large enough to hold the entire relevant search tree, the trick has little effect.
- **Shortest-PV refinement**: `refine_shortest` repeatedly solves with increasing depth bounds and clears the TT. During each individual bounded search the epsilon trick still applies. Be careful that a too-large `ε` may cause the bounded search to explore beyond the depth limit in node count even when `max_depth` is small.
- **GHI handling**: the paper does not discuss graph-history interaction. The epsilon trick is independent of cycle handling; it can be combined with the existing `path` set and `path_code` twin logic in `src/search/tt.rs` without modification.

## Verification

To confirm the implementation works:

1. **Correctness**: run the full test suite with `ε = 0.0` (which, because `epsilon_ceil` enforces `x + 1`, reproduces the original `+1` threshold exactly) and with the chosen `ε > 0`. Outcomes should match. A small test harness can compare results on a suite of FENs.
2. **Performance**: for each `(ε, tt_size)` pair, measure:
   - number of nodes searched,
   - wall-clock time,
   - number of recursive calls at the root.
   The expectation is that `ε > 0` reduces root re-calls and total time, especially for small TT sizes and hard positions.
3. **Regression**: run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, and `cargo doc` after any changes.

## References

- Pawlewicz, J., & Lew, L. (2007). *Improving Depth-first PN-Search: 1 + ε Trick*. Institute of Informatics, Warsaw University.
- `src/search/dfpn.rs` in `atomic_solver` — current DF-PN+ implementation including the epsilon trick.
