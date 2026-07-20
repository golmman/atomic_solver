# Plan 11: Cache `ChildInfo` across `select_children` iterations

## Start

Read `docs/plans/speed/analysis.md`.  Open `src/search/dfpn/children.rs` and
`src/search/dfpn/core.rs`.  Trace how `dfpn` calls `select_children` inside its
main loop and how `evaluate_child` is invoked for every move on every iteration.

## Goal

Avoid re-evaluating every child of a node after each child expansion.

## Background

Inside `dfpn` the main loop looks like this:

```rust
loop {
    let selection = self.select_children(pos, &moves, max_depth, is_or_node);
    // ... pick child and expand it with self.dfpn(...)
}
```

`select_children` evaluates every child once to build a `Vec<ChildInfo>`:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn/children.rs" lines="34-105" />

After the selected child is expanded, `dfpn` loops again and `select_children`
evaluates *all* children again, even though only one child's bounds have changed.
The repeated `do_move`/`undo_move`, outcome generation and TT probes are
expensive.

## Implementation tasks

1. Split `select_children` into two parts:
   - `evaluate_all_children(...) -> Vec<ChildInfo>` that builds the per-node
     child table.
   - `select_from_children(children: &[ChildInfo], is_or_node: bool) ->
     ChildSelection` that computes the proof/disproof numbers and the best child
     from the cached table.  `is_solved_by_children` and
     `best_and_second_unsolved` already work on `&[ChildInfo]`, so this should
     be a clean refactor.
2. In `dfpn`, allocate one `Vec<ChildInfo>` before the loop and fill it with
   `evaluate_all_children` on the first iteration.
3. After expanding the selected child, call `evaluate_child` for only that
   child and overwrite the corresponding `ChildInfo` in the cached vector.
4. Call `select_from_children` to recompute `pn`, `dn`, the best/second child,
   and any solved outcome from the updated cache.
5. Preserve the exact same `ChildSelection` semantics (including shortest-win
   / longest-loss depth handling and `repetition_seen` flags).
6. Consider keeping a stack of `Vec<ChildInfo>` buffers inside `Search` to avoid
   per-node allocation, or use a single reusable `Vec` with `clear()` per
   recursion level.

## File changes

- `src/search/dfpn/children.rs`
- `src/search/dfpn/core.rs`
- `src/search/dfpn/mod.rs` (if a child-info stack is added to `Search`)

## Risks

- `evaluate_child` depends on `self.path` and `self.path_code`.  The cache must
  be rebuilt or updated whenever the path changes, which it does on every child
  expansion.  Updating a single cached child is safe because the child's
  evaluation already accounts for the path at the time it is re-evaluated, but
  siblings' cached values must not be evaluated with a stale path.
- The `moves` `MoveList` is generated once per node and is independent of the
  path, so it can be reused.
- Be careful that `select_children` currently also returns `best_move` for the
  unsolved case; `select_from_children` must keep the same best-move selection.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19"
```

Outcomes and PVs must be identical.  Node counts should drop, especially on
positions where the DF-PN loop revisits the same parent many times.

## Final task

Write `docs/plans/speed/report11.md` with node-count and timing before/after on
a set of positions, and note how many `do_move`/`undo_move` calls were saved.
