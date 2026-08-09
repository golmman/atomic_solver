# Implementation Report: Compact Proof-Tree Memory Layout

This report documents the implementation of `docs/plans/proof/plan3.md`.

## Summary of changes

- `ProofNode` was compacted from 56 bytes to 32 bytes:
  - `parent` is now `Option<NonZeroU32>` (encodes `id + 1`; `None` for root).
  - `children: Vec<usize>` was replaced with `first_child` and `next_sibling`
    `Option<NonZeroU32>` intrusive links.
  - `depth` stays `u32` in this pass.
- `ProofTree` gained `children(&self, node_id)` for iterating the intrusive list
  and `add_node` now inserts children at the front of the list.
- The per-node `child_index: Vec<HashMap<u16, usize>>` was replaced by a
  single global `child_index: HashMap<u64, u32>` keyed by
  `(parent_id << 32) | move_to_bits(mv)`.
- `expanded_by_hash` now stores one canonical `usize` id per `(hash, outcome)`
  instead of a `Vec<usize>`.
- `ProofTreeWorkerHandle::dump_to_bin` was added; `main.rs` now dumps the proof
  tree directly from the worker thread without cloning the finalized tree into
  the main thread.
- `binary.rs` was updated to write/read the `NonZeroU32` parent encoding and to
  rebuild the intrusive child list on load.
- All tests, examples, and `AGENTS.md` were updated for the new `ProofNode`
  shape.

## Files changed

- `src/proof_tree/mod.rs` — reduced to a module re-export file.
- `src/proof_tree/node.rs` — new file containing `ProofNode`, `ProofTree` core
  helpers, and unit tests split into `src/proof_tree/node/tests.rs`.
- `src/proof_tree/binary.rs` — updated read/write paths for the compact links.
- `src/proof_tree/worker.rs` — global child index, link-list reconcile, new
  `dump_to_bin`, updated `estimate_memory`.
- `src/proof_tree/worker/tests.rs` — updated assertions for `children()`
  iteration.
- `src/main.rs` — uses `handle.dump_to_bin` instead of `handle.tree().to_bin`.
- `tests/test_proof_tree.rs` — uses `tree.children(id)` for defender-reply
  counting.
- `examples/inspect_pt.rs` — uses `first_child`/`next_sibling` and
  `tree.children(...)`.
- `AGENTS.md` — updated `ProofTreeWorkerHandle` API description.

## Deviations from the original plan

- `src/proof_tree/mod.rs` was split into `node.rs` plus `node/tests.rs` before
  it reached the 10 KiB soft limit, keeping the core tree module small.
- `reconcile_children` recursively removes `child_index` entries for pruned
  subtrees so the map size stays honest for `estimate_memory`. The plan listed
  this as optional; it is implemented here.
- `estimate_memory` uses `1.1` as the safety factor. The plan suggested tuning
  this after measurement; the first run succeeded at `--pt-size 32`, so it was
  left at `1.1`.
- `read_proof_tree` adds an explicit check that `node_count <= u32::MAX` and
  returns an error rather than panicking if a dump exceeds the new id width.

## Verification

### Static checks

- `cargo fmt --check` passes.
- `cargo clippy --all-targets` passes.
- `cargo test` passes (126 lib tests + integration tests, with expected ignored
  slow/stress tests).
- `cargo doc --no-deps` passes.

### ProofNode size

| Layout | Size |
|---|---|
| Old (`parent: Option<usize>`, `children: Vec<usize>`) | 56 bytes |
| New (`parent/first_child/next_sibling: Option<NonZeroU32>`) | 32 bytes |

### Target FEN

Command:

```bash
cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10 --pt-size 32 --dump-path /tmp/pt32.bin
```

Result:

```text
outcome: win
pre_exit: reason=Timeout outcome=win nodes=2783768
proof_tree: nodes=92028 win=46014 loss=46014 root_depth=77
proof_tree_dump: /tmp/pt32.bin
real 0m10.151s
```

| `--pt-size` | nodes | root depth | dump size | wall-clock | outcome |
|---|---|---:|---:|---:|---|
| 64 | 92,028 | 77 | 552,237 B (~540 KB) | ~10.15 s | win |
| 32 | 92,028 | 77 | 552,237 B (~540 KB) | ~10.15 s | win |

Both runs completed with the same node count, root depth, and dump size. The
stretch goal of `--pt-size 32` succeeds.

### Smaller sanity positions

```bash
cargo run --release -- --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1"   --timeout 5 --pt-size 16 --dump-path /tmp/rook.bin
cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --timeout 5 --pt-size 16 --dump-path /tmp/tworooks.bin
```

- Rook mate: `outcome: win length: 1`, 2 nodes, root depth 1.
- Two-rook mate: `outcome: win length: 3`, 4 nodes, root depth 3.

`inspect_pt` and `verify_ppv` confirm the two-rook PPV `f1f7 e8d8 g1g8` is a
valid principal variation.

### PPV validation on the target FEN

`inspect_pt /tmp/pt32.bin` reports `validate_ppv: true` (the extracted 77-ply
line exists in the dumped tree). However, `verify_ppv` with the same line still
refutes it:

```text
is_ppv: false
PPV refuted at defender ply 8/77, supplied move 'c6c5' is not a longest defense (depth 53, longest 55)
```

This matches the pre-existing depth-consistency issue identified in
`report2.md`; the compact layout does not change the canonicalization logic and
therefore does not resolve it.

## Problems encountered and fixes

1. **Borrow-checker complexity with intrusive links.** Rebuilding a parent's
   child list and updating `child_index` required careful sequencing so that
   `self.tree` and `self.child_index` are not borrowed simultaneously. This was
   solved by collecting child ids into a local `Vec` before mutating the tree or
   the map.
2. **Global `child_index` consistency on prune.** When a parent is realized
   and its children are pruned, the removed subtrees' map entries are
   recursively deleted. Without this, `estimate_memory` could over-report and
   `find_or_create_node` might return stale ids for paths that no longer lead
   to the live tree.
3. **Binary loader link rebuild.** `read_proof_tree` needed to insert each child
   at the front of its parent's list to match the order produced by
   `add_node`. A private `node_children` helper mirrors `ProofTree::children`
   during post-order depth derivation.

## Unresolved parts

- **PPV depth consistency on the target FEN** remains unresolved (see
  `report2.md`). `extract_ppv` returns a 77-ply line that exists in the proof
  tree, but `verify_ppv` shows a defender reply that is not the longest
  available defense. This likely requires a post-finalization depth
  consistency pass or path-aware canonicalization for GHI-sensitive positions.
- **Memory budget on very long searches.** The unbounded `mpsc` channel between
  search and worker can still grow a large event backlog before the worker sets
  `memory_limited`. A bounded channel or `is_full` signal remains a future
  safeguard.
- **`estimate_memory` accuracy.** It uses `Vec::capacity()` and
  `HashMap::capacity()` with a fixed `1.1` factor. Geometric growth may
  overestimate actual resident memory; the factor may need further tuning if
  runs start hitting the budget too early.

## Missing tests

- No explicit unit test for `ProofTreeWorkerHandle::dump_to_bin` (it is covered
  indirectly by `tests/test_cli.rs::cli_dump_path_writes_proof_tree_dump` and the
  runtime checks).
- No stress test that exercises `u32` node-id overflow handling.

## Next steps

1. Investigate the PPV depth inconsistency on the target FEN, either by adding
   a post-finalization depth consistency pass or by making canonicalization
   path/repetition-aware.
2. Consider a bounded proof-event channel or worker back-pressure signal.
3. If `worker.rs` grows further, split the handle and event-loop logic into
   submodules.
