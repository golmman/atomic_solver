# Implementation Plan: Compact Proof-Tree Memory Layout

## Goal

Reduce the peak in-memory size of the proof tree by shrinking `ProofNode` and
eliminating per-node heap allocations and per-node `HashMap` overhead. The
primary success criterion is that the target FEN
`4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` completes with
`--timeout 10` and a much smaller `--pt-size` than the current 128 MB, while
producing the same outcome and a comparable `proof_tree.bin`.

A stretch goal is `--pt-size 32`.

## Scope

- Compact `ProofNode`:
  - Replace `parent: Option<usize>` with `Option<NonZeroU32>`.
  - Replace `children: Vec<usize>` with `first_child: Option<NonZeroU32>` and
    `next_sibling: Option<NonZeroU32>`.
  - Keep `depth: u32` in the node (node becomes 32 B; 24 B if `depth` is packed
    later).
  - Use `u32` node ids everywhere with an overflow panic.
- Replace `child_index: Vec<HashMap<u16, usize>>` with a single global
  `HashMap<u64, u32>` keyed by `(parent_id << 32) | move_bits`.
- Replace `expanded_by_hash: HashMap<(u64, Outcome), Vec<usize>>` with
  `HashMap<(u64, Outcome), usize>` selecting a single canonical id in one pass.
- Add `ProofTreeWorkerHandle::dump_to_bin` and make `main.rs` dump directly from
  the worker instead of cloning the finalized `ProofTree` first.
- Update `ProofTree` helpers (`extract_ppv`, `validate_ppv`, `is_terminal`,
  `to_bin`, `from_bin`, `add_node`) to use the first-child / next-sibling list.
- Update `estimate_memory` to account for the compact layout and global map.
- Update all worker tests, `proof_tree` tests, `tests/test_proof_tree.rs`, and
  `examples/inspect_pt.rs` that access `ProofNode.children`.

## Non-goals

- No DAG sharing; the proof tree stays a tree and the `proof_tree.bin` format
  stays parent-id + move per node.
- No streaming finalizer to disk; `finalize_tree` still builds the final tree in
  memory.
- No change to the DF-PN+ search, the `NodeProven` event protocol, the
  transposition table, GHI handling, or move ordering.
- No `ProofSink` abstraction; `mpsc::Sender<ProofEvent>` stays.
- No `unsafe` code.

## Background and current footprint

The current `ProofNode` from `src/proof_tree/mod.rs` is 56 bytes and carries a
24-byte `Vec<usize>` shell for `children`:

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

The worker additionally keeps `child_index: Vec<HashMap<u16, usize>>`, which is
48 bytes per node even when empty, plus per-edge `HashMap` overhead. For the
target FEN (92 028 nodes after `report2.md`) the in-memory cost is roughly:

| Component | Approximate size |
|---|---|
| `ProofNode` vector | 5.2 MB |
| `children` `Vec` shells + edge data | ~3.0 MB |
| Per-node `child_index` | ~6.1 MB |
| `expanded_by_hash` (during finalization) | ~2–3 MB |
| **Total actual** | **~14–17 MB** |
| `estimate_memory() × 1.5` | **~21–26 MB** |

A compact node with three `Option<NonZeroU32>` links plus `Move`, `hash`, and
`Option<Outcome>` is 32 bytes; packing `move`/`outcome`/`depth` into a single
`u32` would bring it to 24 bytes. A single global `HashMap` for `child_index`
eliminates the 48 B per-node `HashMap` shells. The combined effect should cut
peak memory by more than half.

## Architectural decisions

1. **Node ids are `u32` with `NonZeroU32` for links.**
   - `ProofNode` stores `parent`, `first_child`, and `next_sibling` as
     `Option<NonZeroU32>`.
   - Node `0` is the root and can never appear as a child or sibling, so `0` is
     a safe niche for `None`.
   - `parent` encodes `id + 1` because the root (`id 0`) can be the parent of
     another node; `None` encodes the root's missing parent.
   - `add_node` panics with a clear message if the node count reaches
     `u32::MAX`.

2. **Children are an intrusive first-child / next-sibling list.**
   - No `Vec<usize>` per node and no per-node heap allocations.
   - `ProofTree` exposes `children(&self, id: usize) -> impl Iterator<Item = usize>`
     so the rest of the code can iterate children without knowing the internal
     representation.
   - The binary dump format is unchanged because `to_bin` only needs each node's
     parent and move; `from_bin` rebuilds the linked list from the parent array.

3. **A single global map replaces the per-node `child_index`.**
   - Key: `u64` packing `parent_id` in the high 32 bits and `move_to_bits(mv)` in
     the low 32 bits. This is fast to hash and small to store.
   - Value: child `u32` id.
   - The map is used by `find_or_create_node` and by `reconcile_children` when
     it removes pruned children.

4. **`expanded_by_hash` keeps one canonical id per `(hash, outcome)` key.**
   - The finalization pass selects the canonical expanded node while building the
     map, instead of collecting a `Vec<usize>` for every key and then choosing
     later.

5. **`main.rs` dumps directly from the worker.**
   - `ProofTreeWorkerHandle::dump_to_bin` serializes the worker's finalized tree
     without returning a clone to the main thread.
   - `handle.tree()` is retained for tests and examples.

6. **`depth` stays as `u32` for simplicity in this plan.**
   - The 32-byte `ProofNode` is already a large win. Packing `move`/`outcome`/
     `depth` into a single `u32` (14 bits for depth, 2 for outcome, 16 for move)
     is listed as a future micro-optimization.

## Detailed implementation tasks

### 1. Redefine `ProofNode` in `src/proof_tree/mod.rs`

```rust
use std::num::NonZeroU32;

#[derive(Debug, Clone)]
pub struct ProofNode {
    pub parent: Option<NonZeroU32>,       // id + 1; None for root
    pub first_child: Option<NonZeroU32>,  // raw child id; None if no children
    pub next_sibling: Option<NonZeroU32>, // raw sibling id; None if last
    pub mv: Move,
    pub hash: u64,
    pub outcome: Option<Outcome>,
    pub depth: u32,
}
```

- `first_child`/`next_sibling` use raw ids because a child/sibling can never be
  the root (`id 0`).
- `parent` encodes `id + 1` because the root can be a parent.
- Add small accessor helpers if needed, e.g. `parent_id(&self) -> Option<u32>`.

### 2. Add child-iteration helpers to `ProofTree`

```rust
impl ProofTree {
    /// Iterate over the ids of `node_id`'s children.
    pub fn children(&self, node_id: usize) -> impl Iterator<Item = usize> + '_ {
        let mut next = self.nodes[node_id]
            .first_child
            .map(|nz| nz.get() as usize);
        std::iter::from_fn(move || {
            let id = next?;
            next = self.nodes[id].next_sibling.map(|nz| nz.get() as usize);
            Some(id)
        })
    }

    /// Add a child under `parent_id` and return its id.
    pub(crate) fn add_node(
        &mut self,
        parent_id: usize,
        mv: Move,
        hash: u64,
        outcome: Option<Outcome>,
        depth: u32,
    ) -> usize {
        let id = self.nodes.len();
        assert!(id < u32::MAX as usize, "proof tree node id overflow");

        let parent = NonZeroU32::new((parent_id as u32) + 1);
        let id_nz = NonZeroU32::new(id as u32).unwrap();

        let old_first = self.nodes[parent_id].first_child;
        self.nodes.push(ProofNode {
            parent,
            first_child: None,
            next_sibling: old_first,
            mv,
            hash,
            outcome,
            depth,
        });
        self.nodes[parent_id].first_child = Some(id_nz);
        id
    }
}
```

Update `ProofTree::new` to create a root node with all link fields set to
`None`.

### 3. Update `extract_ppv`, `validate_ppv`, and `is_terminal`

Replace every use of `node.children.iter()` or `node.children.is_empty()` with
`self.children(id)` or `self.nodes[id].first_child.is_none()`.

```rust
pub fn is_terminal(&self, node_id: usize) -> bool {
    self.nodes
        .get(node_id)
        .is_some_and(|n| n.outcome.is_some() && n.depth == 0)
}

pub fn extract_ppv(&self) -> Vec<Move> {
    let mut pv = Vec::new();
    let mut id = 0usize;
    while !self.is_terminal(id) {
        let node = &self.nodes[id];
        let Some(outcome) = node.outcome else { break; };
        let children: Vec<usize> = self.children(id).filter(|&c| {
            let child = &self.nodes[c];
            match outcome {
                Outcome::Win => child.outcome == Some(Outcome::Loss),
                Outcome::Loss => child.outcome == Some(Outcome::Win),
                Outcome::Draw => false,
            }
        }).collect();
        let next = match outcome {
            Outcome::Win => children.into_iter().min_by_key(|&c| self.nodes[c].depth),
            Outcome::Loss => children.into_iter().max_by_key(|&c| self.nodes[c].depth),
            Outcome::Draw => None,
        };
        let Some(next_id) = next else { break; };
        pv.push(self.nodes[next_id].mv);
        id = next_id;
    }
    pv
}
```

### 4. Update `src/proof_tree/binary.rs`

- `write_proof_tree` already writes `node.parent` as a `u32`; change the read to
  `p.get().saturating_sub(1)` because `parent` now encodes `id + 1`.
- `read_proof_tree` reads each node's `parent` as before. After all node records
  are loaded, rebuild the `first_child`/`next_sibling` linked list by inserting
  each child `i` at the front of its parent `p`'s child list.
- The post-order depth derivation and outcome parity derivation remain the same,
  but iterate over `tree.children(i)` instead of `nodes[i].children`.

### 5. Update the worker data structures in `src/proof_tree/worker.rs`

```rust
pub(crate) struct ProofTreeWorker {
    tree: ProofTree,
    child_index: HashMap<u64, u32>, // (parent_id << 32) | move_bits -> child_id
    expanded_by_hash: HashMap<(u64, Outcome), usize>,
    budget: usize,
    memory_limited: Arc<AtomicBool>,
}
```

Remove `children_len` and `child_index_entries`; edge count is now
`child_index.len()` and child links are inside the nodes.

### 6. Update `find_or_create_node`

```rust
fn find_or_create_node(&mut self, path: &[Move]) -> u32 {
    let mut id = 0u32;
    for &mv in path {
        let key = ((id as u64) << 32) | (move_to_bits(mv) as u64);
        if let Some(&child_id) = self.child_index.get(&key) {
            id = child_id;
            continue;
        }
        let parent_id = id as usize;
        let new_id = self.tree.add_node(
            parent_id,
            mv,
            0,
            None,
            0,
        ) as u32;
        self.child_index.insert(key, new_id);
        id = new_id;
    }
    id
}
```

`add_node` now asserts overflow.

### 7. Update `apply_event`

Same logic as today, but using `u32` ids and converting to `usize` only when
indexing `tree.nodes`.

### 8. Update `reconcile_children`

- Walk the parent's `first_child`/`next_sibling` chain and collect the child ids
  into a local `Vec<u32>`.
- For `Win` parents keep only the `Loss` child with the smallest `depth`.
- For `Loss` parents keep all `Win` children.
- For `Draw` parents keep none.
- Rebuild the parent's child list from the kept ids:

```rust
self.tree.nodes[parent_id as usize].first_child = None;
for child_id in kept.into_iter().rev() {
    let node = &mut self.tree.nodes[child_id as usize];
    node.next_sibling = self.tree.nodes[parent_id as usize].first_child;
    self.tree.nodes[parent_id as usize].first_child = NonZeroU32::new(child_id);
}
```

- For each removed child, delete its `(parent_id, move)` entry from
  `child_index`. Optionally recursively remove the removed subtree's entries from
  `child_index` to keep the map's memory honest; the nodes themselves become
  orphans and are ignored by `finalize_tree`.

### 9. Update `build_expanded_index` and `finalize_tree`

Change `expanded_by_hash` to `HashMap<(u64, Outcome), usize>`.

```rust
fn build_expanded_index(&mut self) {
    self.expanded_by_hash.clear();
    for (id, node) in self.tree.nodes.iter().enumerate() {
        let Some(outcome) = node.outcome else { continue; };
        let expanded = node.depth == 0 || node.first_child.is_some();
        if !expanded { continue; }

        let key = (node.hash, outcome);
        let implied = if node.depth == 0 {
            0
        } else {
            let child_depths: Vec<u32> = self.tree.children(id).map(|c| self.tree.nodes[c].depth).collect();
            match outcome {
                Outcome::Win => child_depths.iter().min().copied().unwrap_or(0).saturating_add(1),
                Outcome::Loss => child_depths.iter().max().copied().unwrap_or(0).saturating_add(1),
                Outcome::Draw => 0,
            }
        };
        let consistent = node.depth == implied;
        let better = match self.expanded_by_hash.get(&key) {
            None => true,
            Some(&other) => {
                let other_node = &self.tree.nodes[other];
                let other_implied = /* compute for other */;
                let other_consistent = other_node.depth == other_implied;
                if consistent != other_consistent {
                    consistent
                } else {
                    node.depth < other_node.depth
                }
            }
        };
        if better {
            self.expanded_by_hash.insert(key, id);
        }
    }
}
```

The post-order depth recompute in `finalize_tree` uses
`self.tree.children(i)` instead of `nodes[i].children`.

### 10. Add `ProofTreeWorkerHandle::dump_to_bin`

Add a new worker message:

```rust
enum ProofTreeWorkerMessage {
    GetStats(Sender<ProofResponse>),
    GetTree(Sender<ProofResponse>),
    Finalize,
    DumpToBin { path: String, tx: Sender<io::Result<()>> },
}
```

```rust
impl ProofTreeWorkerHandle {
    pub fn dump_to_bin<P: AsRef<std::path::Path>>(&self, path: P) -> io::Result<()> {
        let (tx, rx) = channel();
        self.query_tx
            .send(ProofTreeWorkerMessage::DumpToBin {
                path: path.as_ref().to_string_lossy().into_owned(),
                tx,
            })
            .expect("worker thread alive");
        rx.recv().expect("worker response")?
    }
}
```

The worker handles `DumpToBin` by opening the file and calling `tree.to_bin`.

### 11. Update `main.rs`

Replace the post-finalize `handle.tree()` clone + `to_bin` call with a direct
dump:

```rust
hook_handle.finalize();
let stats = hook_handle.stats();
println!("proof_tree: nodes={} win={} loss={} root_depth={}",
    stats.nodes, stats.win_nodes, stats.loss_nodes, stats.root_depth);

if let Err(e) = hook_handle.dump_to_bin(&dump_path) {
    eprintln!("failed to write proof-tree dump to {dump_path}: {e}");
} else {
    println!("proof_tree_dump: {dump_path}");
}
```

### 12. Update `estimate_memory`

```rust
fn estimate_memory(&self) -> usize {
    let node_size = std::mem::size_of::<ProofNode>();
    let nodes_mem = self.tree.nodes.capacity() * node_size;

    // HashMap<u64, u32>: entry size ~ size_of::<(u64, u32)>() + control byte
    let child_index_entry = std::mem::size_of::<(u64, u32)>() + 1;
    let child_index_mem =
        std::mem::size_of::<HashMap<u64, u32>>() +
        self.child_index.capacity() * child_index_entry;

    // expanded_by_hash only exists briefly during finalization.
    let total = nodes_mem + child_index_mem;
    (total as f64 * 1.1) as usize
}
```

Tune the `1.1` factor after measurement.

### 13. Update tests and examples

Files that must be changed because they touch `ProofNode.children`:

- `src/proof_tree/mod.rs` tests
- `src/proof_tree/worker/tests.rs`
- `tests/test_proof_tree.rs`
- `examples/inspect_pt.rs`

Replace:
- `node.children.iter()` with `tree.children(id)`.
- `node.children.len()` with `tree.children(id).count()`.
- `node.children.is_empty()` with `node.first_child.is_none()`.
- `tree.nodes[parent].children` direct accesses with `tree.children(parent)` and,
  where needed, `tree.nodes[parent].first_child`.

Update `child_by_move` in `worker/tests.rs` to use `tree.children(parent)` and
find the matching move.

### 14. Update `AGENTS.md` if needed

- `src/proof_tree/worker.rs` is already over the 20 KiB soft limit with a
  documented justification; the additional link-list and global-map logic can stay
  there but the justification should be refreshed if the file grows
  substantially.
- If `src/proof_tree/mod.rs` exceeds 10 KiB after adding the child iterator and
  `add_node` helper, split `ProofNode` and `ProofTree` core helpers into
  `src/proof_tree/node.rs` and re-export from `mod.rs`.

## Public API changes

- `ProofNode` fields change: `parent`, `first_child`, `next_sibling` are now
  `Option<NonZeroU32>`; `children: Vec<usize>` is removed.
- `ProofTree` gains `children(&self, node_id: usize) -> impl Iterator<Item = usize>`.
- `ProofTree::add_node` signature is unchanged but the internal child storage
  changes.
- `ProofTreeWorkerHandle` gains `dump_to_bin`.
- The binary dump format is unchanged.

## File-size considerations

- `src/proof_tree/mod.rs` may approach the 10 KiB soft limit; split into
  `node.rs` if it exceeds it.
- `src/proof_tree/worker.rs` remains the single file for the worker state
  machine and is already justified in `AGENTS.md`.
- `src/proof_tree/binary.rs` only changes how it builds the linked list, so its
  size should stay similar.

## Verification

Static checks:

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
cargo doc --no-deps
```

Runtime checks:

```bash
# Target FEN with smaller pt-size
cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10 --pt-size 64 --dump-path /tmp/pt64.bin

# Stretch goal
cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10 --pt-size 32 --dump-path /tmp/pt32.bin

# Sm sanity positions
cargo run --release -- --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" --timeout 5 --pt-size 16 --dump-path /tmp/rook.bin
cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --timeout 5 --pt-size 16 --dump-path /tmp/tworooks.bin

# Inspect and validate
cargo run --release --example inspect_pt -- /tmp/pt64.bin
cargo run --release --example verify_ppv -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --moves <extracted_ppv> --timeout 5
```

Compare:

- outcome,
- `proof_tree` node count and root depth from stats,
- dump file size,
- whether `--pt-size 64` (and ideally `--pt-size 32`) succeeds.

## Risks and trade-offs

- **Linked-list bookkeeping.** `first_child` / `next_sibling` insert and remove
  operations are more error-prone than `Vec::push`. Unit tests should cover
  multi-child add, child removal, and sibling ordering.
- **`u32` overflow.** The panic is a safety net; reaching `u32::MAX` nodes is not
  expected for the current search scope.
- **Global `child_index` must stay consistent with the linked list.** If
  `reconcile_children` removes a child but forgets to remove the map entry,
  `find_or_create_node` may return an orphan or a stale id. The link list and
  the map must be updated together.
- **Orphan subtrees during processing.** Removed children and their descendants
  remain in `tree.nodes` until `finalize_tree` rebuilds the tree. Recursively
  removing their `child_index` entries is optional but recommended to keep memory
  honest.
- **`estimate_memory` accuracy.** Using `Vec::capacity()` and `HashMap::capacity()`
  can overestimate by up to ~2x because of geometric growth. Monitor whether the
  safety factor still causes premature `memory_limited` hits.
- **`dump_to_bin` path handling.** The worker opens the file in the worker
  thread; I/O errors are sent back through the one-shot channel.

## Next steps

After implementation and verification, create
`docs/plans/proof/report3.md` documenting:

- actual files changed and any deviations from this plan,
- `ProofNode` size before and after,
- memory and timing measurements for the target FEN with `--pt-size 64` and
  `--pt-size 32`,
- any problems encountered (especially linked-list bugs, map consistency, and
  test rewrites),
- unresolved parts and missing tests,
- next optimization candidates: packing `move`/`outcome`/`depth` into a `u32`,
  streaming the finalizer to disk, or moving to a DAG proof-tree representation.
