# Implementation Plan: Dummy-Parent Proof Tree with Path Traversal

## Goal

Reduce the peak in-memory size of the `ProofTree` by eliminating the
`HashMap<String, usize>` path index and the `pending` event buffer. Instead,
every `NodeProven` event is attached to the tree immediately. Missing ancestors
are created as dummy nodes with `outcome: None` and are later "realized" when
their own event arrives. The final `finalize()` pass removes any remaining dummy
subtrees and rebuilds the canonical proven tree.

The primary success criterion is that the failing FEN
`4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` with
`--pt-size 128 --timeout 10` completes and produces a `proof_tree.bin` of
similar size and correctness to the current `--pt-size 256` run.

## Scope

- Change `ProofNode.outcome` from `Outcome` to `Option<Outcome>` (`None` means a
dummy / not-yet-proven node).
- Remove `ProofTree.index: HashMap<String, usize>` entirely.
- Remove the `pending` map and `flush_pending`/`process_pending` logic from the
worker.
- Build the tree by traversing from the root using the event's `Vec<Move>` path,
creating dummy ancestors as needed.
- Defer parent-child outcome validation until the parent node is realized.
- Update `finalize_tree()` to prune dummy subtrees and rebuild the canonical tree
from the remaining real nodes.
- Update memory accounting in `estimate_memory()`.
- Update all unit tests and the `ProofTree` public API consumers.
- Keep the binary dump format unchanged.

## Non-goals

- No switch to first-child / next-sibling children representation in this plan;
`children: Vec<usize>` stays. That is a follow-up optimization.
- No change to the DF-PN+ search, the `NodeProven` event protocol, or the
`TranspositionTable`.
- No change to the compact `proof_tree.bin` format.
- No `ProofSink` abstraction; the `mpsc::Sender<ProofEvent>` stays.

## Background and constraints

The current worker stores two large path-derived structures:

1. `ProofTree.index: HashMap<String, usize>` — one full UCI path string per
node, plus `String`/HashMap overhead.
2. `pending: HashMap<String, Vec<NodeProven>>` — one parent path string key per
out-of-order event plus the full `Vec<Move>` path inside every buffered event.

Both exist because DF-PN emits child `NodeProven` events before their parent
`NodeProven` event. The worker needs to attach children to parents, but it does
not know the parent's node id when the child arrives.

The path strings are the dominant memory cost for deep trees. The node structs
themselves are comparatively small.

The worker already maintains `expanded_by_hash: HashMap<(u64, Outcome), Vec<usize>>`
for final canonicalization. That index is still needed.

## Architectural decisions

1. **Dummy nodes carry `outcome: None`.**
   A node that has been created as an ancestor placeholder but whose own
   `NodeProven` event has not yet arrived is a dummy. Its `hash` and `depth` are
   uninitialized and are ignored until realization. The rest of the codebase
   keeps the plain `Outcome` enum; only `ProofNode` wraps it in `Option`.

2. **Paths are traversed, not indexed.**
   To locate or create the node for a `Vec<Move>` path, the worker walks from the
   root and scans `children` for a matching `mv`. If a child is missing, a dummy
   node is created and appended. The tree itself is the lookup structure; no
   `HashMap` of paths is kept.

3. **Child events are attached immediately.**
   When a child event arrives, the worker creates any missing ancestor dummies,
   creates or updates the target node, and links the target under its parent.
   Outcome validation is deferred until the parent is realized.

4. **Parent realization reconciles children.**
   When a dummy parent receives its own `NodeProven` event, its `outcome` becomes
   `Some(...)`. At that point the worker validates and selects children:
   * `Win` parents keep only `Loss` children, preferring the smallest `depth`.
   * `Loss` parents keep all `Win` children.
   * `Draw` parents should not occur in the proof tree; any children are removed.
   Inconsistent children are removed from the parent's `children` vector but are
   not deleted from `nodes`; they become orphans and are ignored by the final
   rebuild.

5. **Finalization prunes dummies.**
   `finalize_tree()` drains remaining events, then rebuilds the tree from the
   root. Any node with `outcome == None` is skipped, which prunes its entire
   subtree. The canonicalization by `(hash, outcome)` still runs on the remaining
   real nodes.

## Detailed implementation tasks

### 1. Extend `ProofNode` with `Option<Outcome>`

`src/proof_tree/mod.rs`:

```rust
pub struct ProofNode {
    pub parent: Option<usize>,
    pub mv: Move,
    pub hash: u64,
    pub outcome: Option<Outcome>,
    pub depth: u32,
    pub children: Vec<usize>,
}
```

Update `ProofTree`:

```rust
pub struct ProofTree {
    pub root_fen: String,
    pub nodes: Vec<ProofNode>,
}
```

Remove `index` and all code that inserts or reads from it. `ProofTree::new` and
`add_node` signatures change to accept `Option<Outcome>` and no longer take a
`full_path`.

### 2. Update `ProofTree` helpers and serialization

- `ProofTree::new(root_fen, root_hash, root_outcome, root_depth)` takes
  `root_outcome: Option<Outcome>`.
- `ProofTree::add_node(..., outcome: Option<Outcome>, ...)` no longer takes or
  stores a path string.
- `is_terminal()` returns true only for real nodes with `depth == 0`:
  `node.outcome.is_some() && node.depth == 0`.
- `extract_ppv()` and `validate_ppv()` match on `node.outcome` and treat `None`
  as an invalid / stop node.
- `to_bin()` expects a finalized tree and can `expect` or return an error on
  `None` outcomes. `from_bin()` wraps derived outcomes in `Some(...)`.

### 3. Add path traversal and dummy creation to the worker

`src/proof_tree/worker.rs`:

```rust
fn find_or_create_node(&mut self, path: &[Move]) -> usize {
    let mut id = 0;
    for &mv in path {
        let mut found = None;
        for &child_id in &self.tree.nodes[id].children {
            if self.tree.nodes[child_id].mv == mv {
                found = Some(child_id);
                break;
            }
        }
        id = if let Some(child_id) = found {
            child_id
        } else {
            let new_id = self.tree.nodes.len();
            self.tree.nodes.push(ProofNode {
                parent: Some(id),
                mv,
                hash: 0,
                outcome: None,
                depth: 0,
                children: Vec::new(),
            });
            self.tree.nodes[id].children.push(new_id);
            new_id
        };
    }
    id
}
```

### 4. Replace `insert_event` / `process_event`

```rust
fn process_event(&mut self, event: NodeProven) {
    if self.memory_limited.load(Ordering::Acquire) {
        return;
    }
    let id = self.find_or_create_node(&event.path);
    let was_dummy = self.tree.nodes[id].outcome.is_none();
    self.apply_event(id, &event);
    if was_dummy {
        self.reconcile_children(id);
    }
    if !event.path.is_empty() {
        let parent_len = event.path.len() - 1;
        let parent_id = self.find_or_create_node(&event.path[..parent_len]);
        if self.tree.nodes[parent_id].outcome.is_some() {
            self.reconcile_children(parent_id);
        }
    }
    if self.estimate_memory() > self.budget {
        self.memory_limited.store(true, Ordering::Release);
    }
}

fn apply_event(&mut self, id: usize, event: &NodeProven) {
    let node = &mut self.tree.nodes[id];
    node.mv = event.mv;
    node.hash = event.hash;
    match node.outcome {
        None => {
            node.outcome = Some(event.outcome);
            node.depth = event.depth;
        }
        Some(_) if event.depth < node.depth => {
            node.depth = event.depth;
        }
        _ => {}
    }
    self.insert_expanded(id);
}
```

`insert_expanded` should only add nodes with `Some(outcome)` to
`expanded_by_hash`:

```rust
fn insert_expanded(&mut self, id: usize) {
    let node = &self.tree.nodes[id];
    let Some(outcome) = node.outcome else { return; };
    let has_children = !node.children.is_empty();
    if node.depth == 0 || has_children {
        let key = (node.hash, outcome);
        let present = self
            .expanded_by_hash
            .get(&key)
            .is_some_and(|v| v.contains(&id));
        if !present {
            self.expanded_by_hash.entry(key).or_default().push(id);
        }
    }
}
```

### 5. Implement `reconcile_children`

```rust
fn reconcile_children(&mut self, parent_id: usize) {
    let parent = &self.tree.nodes[parent_id];
    let Some(parent_outcome) = parent.outcome else { return; };
    let parent_outcome = parent_outcome; // copy
    let children = &mut self.tree.nodes[parent_id].children;
    match parent_outcome {
        Outcome::Win => {
            children.retain(|&c| self.tree.nodes[c].outcome == Some(Outcome::Loss));
            if let Some(&best) = children
                .iter()
                .min_by_key(|&&c| self.tree.nodes[c].depth)
                .copied()
            {
                children.clear();
                children.push(best);
            } else {
                children.clear();
            }
        }
        Outcome::Loss => {
            children.retain(|&c| self.tree.nodes[c].outcome == Some(Outcome::Win));
        }
        Outcome::Draw => {
            children.clear();
        }
    }
}
```

Because `find_or_create_node` already links the target under its parent, the
parent does not need a separate attachment step. `reconcile_children` is called
whenever the parent's `outcome` becomes known or when a real parent receives a
new/updated child.

### 6. Remove `pending` and `index`

Delete from `ProofTreeWorker`:
- `pending: HashMap<String, Vec<NodeProven>>`
- `index_path_bytes: usize`
- `pending_path_bytes: usize`
- `pending_move_bytes: usize`
- `pending_event_count: usize`
- `fn clear()` resets `expanded_by_hash` but no longer touches `pending` or
  `index_path_bytes`.
- `fn process_pending(...)`, `fn flush_pending()` are removed.

`ProofTreeWorker::new` initializes `tree` with a dummy root:

```rust
let tree = ProofTree::new(root_fen, 0, None, 0);
```

The root remains a dummy until the first `NodeProven` event with an empty path
realizes it.

### 7. Update `finalize_tree`

The existing finalization algorithm in `worker.rs` stays structurally the same,
but with two changes:

1. **No `flush_pending`.** After draining events there is no pending buffer.
2. **Prune dummies during rebuild.** When traversing `old_tree` in the rebuild
   step, skip any node with `outcome == None`. The children of such a node are
   not copied into `new_tree`.

The post-order depth recomputation and the check for unexpanded internal nodes
are unchanged, except they now operate on `Option<Outcome>`.

The root must be real after a successful search; if `self.tree.nodes[0].outcome`
is `None`, `finalize_tree` should log an error and exit (or produce an empty
final tree, depending on what the CLI expects).

### 8. Simplify memory accounting

`estimate_memory()` becomes approximately:

```rust
fn estimate_memory(&self) -> usize {
    let node_size = std::mem::size_of::<ProofNode>();
    let nodes_mem = self.tree.nodes.capacity() * node_size;
    let children_mem: usize = self
        .tree
        .nodes
        .iter()
        .map(|n| n.children.capacity() * std::mem::size_of::<usize>())
        .sum();
    let expanded_by_hash_mem = self
        .expanded_by_hash
        .iter()
        .map(|(k, v)| {
            std::mem::size_of::<(u64, Outcome)>()
                + std::mem::size_of::<usize>() * v.capacity()
        })
        .sum();
    let total = nodes_mem + children_mem + expanded_by_hash_mem;
    (total as f64 * 1.5) as usize
}
```

The 1.5x multiplier can be kept or tuned after measurement.

### 9. Update `main.rs`

`main.rs` is largely unchanged. The pre-exit hook still calls
`pt_handle.finalize()` and dumps the tree. The only difference is that a
successful `finalize()` now guarantees that all nodes in the dumped tree have
`Some(outcome)`.

`ExitReason::MemoryLimit` handling is unchanged: the CLI exits before the hook.

### 10. Update tests and examples

- `src/proof_tree/mod.rs` tests no longer use `tree.index[...]`. Instead,
  verify `nodes[i].children` and `extract_ppv`/`validate_ppv`.
- `src/proof_tree/worker/tests.rs` update every manual `NodeProven` construction
  and every assertion that accessed `tree.index`.
- Add new worker tests:
  * a child event arrives before its parent and is realized correctly when the
    parent event arrives;
  * a `Win` parent receives multiple `Loss` children and keeps only the
    shallowest;
  * a `Loss` parent receives both `Win` and `Loss` children and keeps only the
    `Win` children;
  * `finalize()` prunes a dummy subtree whose parent event never arrived.
- `tests/test_proof_tree.rs` and examples that use `ProofTree::from_bin` or
  `add_node` need signature updates for `Option<Outcome>` and the removed
  `full_path` argument.

### 11. Update `AGENTS.md` / `README.md` / inline comments

Remove references to the path-string `index` and the `pending` buffer. Describe
the dummy-parent, traversal-based construction and the final prune step.

## Public API changes

- `ProofNode.outcome` becomes `Option<Outcome>`.
- `ProofTree.index` is removed.
- `ProofTree::new` and `ProofTree::add_node` signatures change to accept
  `Option<Outcome>` and no path string.
- The binary dump format is unchanged.

## File-size considerations

- `src/proof_tree/worker.rs` will grow slightly due to `find_or_create_node`,
  `apply_event`, and `reconcile_children`, but will also lose the `pending` and
  `flush_pending` code. Net size should be roughly similar.
- `src/proof_tree/mod.rs` will shrink because `index` and its tests are removed.

## Verification

- `cargo fmt --check`
- `cargo clippy --all-targets`
- `cargo test`
- `cargo doc`

Runtime checks:

```bash
cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10 --pt-size 128 --dump-path /tmp/pt128.bin
cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10 --pt-size 256 --dump-path /tmp/pt256.bin
cargo run --release --example verify_ppv -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --moves <extracted_ppv> --timeout 5
```

Compare:
- outcome,
- `proof_tree` node count and root depth,
- dump file size,
- wall-clock time,
- whether `--pt-size 128` now succeeds.

A handful of other known FENs should also be tested for outcome parity:
`4k3/8/8/8/8/8/8/4R1K1 w - - 0 1` and `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1`.

## Risks and trade-offs

- **Traversal cost.** `find_or_create_node` scans `children` linearly at every
  ply. For `Loss` (AND) nodes with many defender replies this can become
  expensive. If profiling shows it is a bottleneck, the next step is a
  `HashMap<Move, usize>` per node, sorted children with binary search, or a
  first-child / next-sibling representation.
- **Eager dummy creation.** Every event creates an entire ancestor chain of
  dummies. For deep paths and large branching this may create more nodes than the
  current `pending` approach. Measurement on the target FEN will show whether it
  is a net memory win.
- **Orphan nodes.** Removing a child from a `Win` parent's `children` vector
  leaves the child node in `nodes` until `finalize` rebuilds the tree. Memory is
  not freed until then.
- **GHI / path-dependent outcomes.** `reconcile_children` removes inconsistent
  children when a parent is realized. This is the same validation the current
  `attach_child` performs, only deferred. The final canonicalization by
  `(hash, outcome)` must still respect that a node may be `Win` along one path
  and `Loss` along another, which is why `outcome` is part of the canonical
  key.
- **Hard failure on unfinalized dump.** `to_bin()` is only called after
  `finalize()`. If any public caller tries to serialize an unfinalized tree
  containing dummy nodes, `to_bin` must either error or skip them explicitly.

## Next steps

After implementation and verification, create
`docs/plans/proof/report2.md` documenting:

- actual files changed,
- baseline vs. post-change measurements for the target FEN,
- any problems encountered (especially traversal cost and orphan-node memory),
- unresolved parts and missing tests,
- next optimization candidates (first-child / next-sibling, per-node child move
  index, path-hash event keys).
