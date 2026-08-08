# Implementation Plan: Authoritative Incremental Proof Tree

## Goal

Make the in-memory `ProofTree` authoritative by carrying the Zobrist hash of
each node. A final pre-exit pass copies fully expanded subtrees from earlier
occurrences of the same position onto unexpanded nodes, eliminating the need for
the current `Search::emit_proof_tree` transposition-table reconstruction pass.

When the proof-tree memory budget is exceeded the process logs an error and
exits.

## Scope

- Add `hash: u64` to `NodeProven` and `ProofNode`.
- Stop clearing the `ProofTree` at the start of `Search::solve`; accumulate events
  across the entire run.
- Maintain an `expanded_by_hash` index in the worker.
- Add a worker `Finalize` step (triggered from the pre-exit hook) that rebuilds
  the tree from canonical expanded subtrees.
- Remove `Search::emit_proof_tree` and the final `ProofEvent::Clear` / rebuild
  pass from `solve`.
- Update `main.rs` to trigger finalization and export the finalized tree.
- On `ExitReason::MemoryLimit`, log and exit instead of exporting an incomplete
  tree.
- Update worker tests and the `AGENTS.md` / `README` architecture notes.

## Non-goals

- No DAG sharing; copied subtrees are duplicated in the tree. Memory correctness
  first, memory optimization later.
- No change to the compact `proof_tree.bin` format.
- No change to the DF-PN+ search, transposition table, GHI handling, or move
  ordering.
- No multi-root / multi-FEN support. The root FEN is fixed for the lifetime of
  the program.

## Background and constraints

The current pipeline is:

1. `Search::solve` emits `NodeProven` events while searching.
2. Iterative refinement can resolve a node from the TT without re-searching its
   descendants, so the worker's tree may contain non-terminal leaves.
3. At the end of `solve`, `Search::emit_proof_tree` sends `ProofEvent::Clear`
   and re-walks the TT to emit a complete proven subtree.

That last step is best-effort: if TT entries have been evicted, the emitted tree
is incomplete. It also couples the final proof-tree shape to the TT, which is not
part of the `ProofTree` data model.

With hash-based sharing the worker can complete itself: every proven node is
assumed to have been expanded at least once during the same run, so the worker
should already contain a fully expanded twin for every hash it ever receives.
The final pass simply copies the best (shortest-depth, consistent) expanded twin
onto any unexpanded occurrences.

## Architectural decisions

1. **Every `NodeProven` carries `pos.hash()`.**
   - `Position::hash()` includes the board hash and `rule50`, so two equal hashes
     are the same position from the same side to move.
   - `Win`/`Loss` are path-independent after the GHI simplification, so sharing
     by hash is safe for non-`Draw` nodes. `Draw` nodes are not emitted to the
     proof tree.

2. **The `ProofTree` is never cleared automatically.**
   - `Search` no longer sends `ProofEvent::Clear` at the start or end of a
     `solve`.
   - `ProofEvent::Clear` is kept as a test/debug facility but is not used by the
     production solver.

3. **Finalization is a worker-side, post-search pass.**
   - `main.rs` sends `ProofTreeWorkerMessage::Finalize` from the pre-exit hook,
     then queries `GetTree` and dumps.
   - The worker drains any remaining events, selects canonical expanded nodes by
     hash, and rebuilds the tree.

4. **Copying, not reference sharing.**
   - Each occurrence of a transposition gets its own node copies. This keeps the
     existing tree data model and the binary dump unchanged.

5. **Memory limit is a hard failure.**
   - If the worker's estimate exceeds `--pt-size`, `memory_limited` is set.
   - `Search` stops on the next `time_exceeded` check.
   - The pre-exit hook logs the error and exits with a non-zero status.

## Detailed implementation tasks

### 1. Extend `NodeProven` and `ProofNode`

`src/proof_event.rs`:

```rust
#[derive(Debug, Clone)]
pub struct NodeProven {
    pub path: Vec<Move>,
    pub mv: Move,
    pub hash: u64,
    pub outcome: Outcome,
    pub depth: u32,
}

impl NodeProven {
    pub fn new(path: Vec<Move>, hash: u64, outcome: Outcome, depth: u32) -> Self {
        let mv = path.last().copied().unwrap_or(Move::NONE);
        Self { path, mv, hash, outcome, depth }
    }
}
```

`src/proof_tree/mod.rs`:

```rust
pub struct ProofNode {
    pub parent: Option<usize>,
    pub mv: Move,
    pub hash: u64,
    pub outcome: Outcome,
    pub depth: u32,
    pub children: Vec<usize>,
}
```

Update `ProofTree::new` to accept the root hash and `ProofNode` construction
callers.

### 2. Capture the hash in `Search`

In `src/search/dfpn/core.rs`:

- `dfpn` terminal path: pass `pos.hash()` to `emit_proof_node`.
- `try_use_tt` resolved path: pass `pos.hash()`.
- `dfpn` bottom solved-outcome path: pass `pos.hash()`.

In `src/search/dfpn/children.rs`:

- `evaluate_child` already computes `child_key = pos.hash()`; include it in the
  manually constructed `NodeProven`.

Change `Search::emit_proof_node` to take `pos: &Position`:

```rust
fn emit_proof_node(&self, pos: &Position, outcome: Outcome, depth: u32) {
    if outcome == Outcome::Draw {
        return;
    }
    if let Some(sender) = &self.proof_event_sender {
        let event = ProofEvent::NodeProven(NodeProven::new(
            self.move_stack.clone(),
            pos.hash(),
            outcome,
            depth,
        ));
        let _ = sender.send(event);
    }
}
```

### 3. Remove `emit_proof_tree` from the search

- Delete `Search::emit_proof_tree`, `Search::emit_proof_subtree`, and
  `Search::send_proof_node` from `src/search/dfpn/pv.rs`.
- Remove the `ProofEvent` import from `pv.rs` if it is no longer needed.
- Remove `Search::clear_proof_events` and the `ProofEvent::Clear` send from
  `src/search/dfpn/mod.rs`.
- Remove the `if outcome != Outcome::Draw { self.emit_proof_tree(...) }` call at
  the end of `solve_with_progress`.

`Search::solve` and `Search::search_depth` become pure search-and-emit functions;
proof-tree finalization is no longer their responsibility.

### 4. Worker accumulates and indexes expanded nodes

`src/proof_tree/worker.rs`:

- Add `expanded_by_hash: HashMap<(u64, Outcome), Vec<usize>>`.
- When a node becomes "expanded" (terminal `depth == 0` or receives children),
  insert its id into `expanded_by_hash[(hash, outcome)]`.
- When a node's children are replaced or its depth decreases, do **not** try to
  update other nodes during the live search. The final pass recomputes
  everything from the accumulated raw tree.

### 5. Add the `Finalize` control message

`src/proof_tree/worker.rs`:

```rust
enum ProofTreeWorkerMessage {
    GetStats(Sender<ProofResponse>),
    GetTree(Sender<ProofResponse>),
    Finalize,                 // new
}
```

Add `ProofTreeWorkerHandle::finalize(&self)`:

```rust
pub fn finalize(&self) {
    self.query_tx
        .send(ProofTreeWorkerMessage::Finalize)
        .expect("worker thread alive");
}
```

`ProofTreeWorker::handle_query` routes `Finalize` to a new `finalize_tree`
method.

### 6. The finalization algorithm

`finalize_tree` runs in the worker thread after search has stopped:

1. **Drain remaining events.**
   ```rust
   while let Ok(event) = self.event_rx.try_recv() {
       self.handle_event(event);
   }
   ```

2. **Flush any pending children whose parents now exist.** Repeat until the
   pending map stabilizes.

3. **Select canonical expanded nodes per hash.**
   For each `(hash, outcome)` key:
   - Consider only expanded nodes (terminal `depth == 0` or `children` non-empty).
   - Compute each node's "implied depth" from its children:
     * `Win`: `1 + min(Loss child depth)`
     * `Loss`: `1 + max(Win child depth)`
     * terminal: `0`
   - Prefer nodes whose stored `depth` equals the implied depth (consistent).
   - Among those, pick the node with the smallest stored depth as canonical.

4. **Rebuild the final tree from the root.**
   - Create a new `ProofTree` with the same `root_fen` and root hash.
   - DFS from root id 0. For each raw node:
     * Look up its canonical by `(hash, outcome)`.
     * If a canonical exists and is not the node itself, copy the entire
       canonical subtree into the new tree at the current path.
     * Otherwise, copy the raw node and continue into its existing children.
     * Track `current_path_hashes: HashSet<u64>`; if a hash repeats on the path,
       stop expanding to prevent accidental cycles (this should not happen for
       `Win`/`Loss` proofs, but it is a cheap safety guard).

5. **Recompute depths in the rebuilt tree.**
   Post-order traversal:
   - terminal leaf: `0`
   - `Win` with `Loss` children: `1 + min(child.depth)`
   - `Loss` with `Win` children: `1 + max(child.depth)`
   Update stored `depth` values where the computed value differs.

6. **Replace `self.tree` with the rebuilt tree.**

7. **Check for unexpanded internal nodes.**
   If any non-terminal node has no children after the final pass, the
   assumption was violated. Log an error and exit.

### 7. Update the pre-exit hook in `main.rs`

```rust
if let Some(hook) = hook {
    hook(search.exit_reason(), outcome, search.nodes(), &pv);
}
```

becomes approximately:

```rust
if search.exit_reason() == ExitReason::MemoryLimit {
    eprintln!("error: proof-tree memory limit ({pt_size} MB) reached");
    std::process::exit(1);
}

if let Some(hook) = hook {
    // hook now calls pt_handle.finalize(), stats(), tree(), dump
    hook(search.exit_reason(), outcome, search.nodes(), &pv);
}
```

Move the proof-tree dump logic into the hook closure and call
`pt_handle.finalize()` before `pt_handle.tree()`.

### 8. Memory-limit exit

The worker already sets `memory_limited` when `estimate_memory()` exceeds the
budget. <ref_snippet file="/workspace/atomic_solver/src/proof_tree/worker.rs" lines="183-191" />
`Search::time_exceeded` already checks this flag. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/mod.rs" lines="443-456" />
The only change is in `main.rs`: detect `ExitReason::MemoryLimit` and exit before
calling the export hook.

### 9. Update tests and examples

- `src/proof_tree/worker/tests.rs`: add `hash` values to every manually created
  `NodeProven`.
- Add new worker tests:
  * two transpositions with the same hash, where the second one is unexpanded
    and gets its children copied from the first;
  * a node whose depth is improved by a later event, and the final pass picks
    the shorter canonical;
  * a `Finalize` round-trip that produces a complete tree.
- `src/search/dfpn/pv.rs` tests that used `emit_proof_tree` must be rewritten
  or removed.
- `examples/verify_ppv.rs` and `examples/inspect_pt.rs` should continue to work
  because the binary dump format and the root outcome/depth semantics are
  unchanged.

### 10. Update `AGENTS.md` and `README.md`

- Remove references to `Search::emit_proof_tree` rebuilding from the TT.
- Describe the authoritative incremental tree, hash-based sharing, and the
  pre-exit finalization pass.
- Note that the proof tree is not cleared between `solve` calls and that the
  root FEN is fixed.

## File-size considerations

- `src/proof_tree/worker.rs` will grow because it gains the finalization
  algorithm and the hash index. This is acceptable; it was already split from
  `mod.rs` for size reasons.
- `src/search/dfpn/pv.rs` should shrink after removing `emit_proof_tree` and
  `send_proof_node`.
- `src/proof_tree/mod.rs` stays under 10 KB if the finalization logic lives in
  `worker.rs`.

## Verification

- `cargo fmt --check`
- `cargo clippy --all-targets`
- `cargo test`
- `cargo run -- --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1"` (mate in one)
- `cargo run -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"` (mate in three)
- `cargo run --example inspect_pt` on the resulting `proof_tree.bin`
- `cargo run --example verify_ppv` on a known line

## Risks and trade-offs

- **Memory blow-up from subtree copying.** We deliberately copy instead of sharing
  to keep the tree data model and binary format. The existing `--pt-size` cap
  handles runaway growth.
- **Finalization cost.** The final pass is `O(nodes)` and rebuilds the tree
  once. It runs after search, so it does not affect search throughput.
- **Assumption failure.** If a node is proven by TT reuse and no expanded twin is
  in the worker tree, the final pass logs an error and exits. This should not
  happen within a single fresh `solve` without memory limit, but it is a hard
  failure mode.
- **Stale expanded nodes.** A node expanded at depth 5 and later re-proven at
  depth 2 can leave a stale depth-5 subtree. The final pass handles this by
  selecting the canonical node with the smallest consistent depth and copying
  its subtree.

## Next steps

After implementation, create
`docs/plans/proof_tree_authoritative/report1.md` documenting the actual changes,
test results, any problems encountered, and unresolved parts.
