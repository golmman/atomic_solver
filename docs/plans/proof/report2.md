# Implementation Report: Dummy-Parent Proof Tree with Path Traversal

This report documents the implementation of `docs/plans/proof/plan2.md`.

## Summary of changes

- `ProofNode.outcome` changed from `Outcome` to `Option<Outcome>`; `None` marks a dummy / not-yet-proven node.
- `ProofTree.index: HashMap<String, usize>` was removed entirely.
- The worker `pending` map and `flush_pending`/`process_pending` logic were removed.
- The worker now attaches every `NodeProven` event immediately by traversing the event's `Vec<Move>` path from the root, creating dummy ancestors as needed.
- Parent-child outcome validation is deferred until the parent node is realized.
- `finalize_tree()` drains remaining events, prunes dummy subtrees, selects canonical expanded nodes by `(hash, outcome)`, and rebuilds the proven tree.
- Binary dump format is unchanged.

Files modified:

- `src/proof_tree/mod.rs`
- `src/proof_tree/binary.rs`
- `src/proof_tree/worker.rs`
- `src/proof_tree/worker/tests.rs`
- `tests/test_proof_tree.rs`
- `examples/inspect_pt.rs`
- `AGENTS.md` (file-size justification for `worker.rs`)

## Deviations from the original plan

The plan explicitly kept `find_or_create_node` as a linear scan over `children` and did not add a child-move index. During verification that implementation took several minutes of wall-clock time for the target FEN because `estimate_memory()` was O(nodes) and called after every event. To make the target FEN complete in roughly its search timeout, three performance improvements were added:

1. **Per-node child move index.** `ProofTreeWorker` maintains `child_index: Vec<HashMap<u16, usize>>` parallel to `tree.nodes`. `find_or_create_node` is now O(path length) instead of O(path length × branching).
2. **Running memory counters.** `children_len` and `child_index_entries` are updated incrementally, making `estimate_memory()` O(1) instead of scanning the whole tree.
3. **Deferred `expanded_by_hash` build.** The `(hash, outcome) -> Vec<node id>` index is rebuilt once at the start of `finalize_tree()` instead of being updated incrementally on every event, removing per-event HashMap churn.

These changes keep `ProofNode.children` as a `Vec<usize>` (per the plan's non-goal) and do not change the binary format.

## Verification

### Static checks

- `cargo fmt --check` passes.
- `cargo clippy --all-targets` passes.
- `cargo test` passes (126 lib tests + integration tests, with expected ignored slow/stress tests).
- `cargo doc --no-deps` passes.

### Target FEN

```bash
cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10 --pt-size 128 --dump-path /tmp/pt128.bin
cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10 --pt-size 256 --dump-path /tmp/pt256.bin
```

Results:

| `--pt-size` | `proof_tree` nodes | root depth | dump size | wall-clock |
|-------------|-------------------:|-----------:|----------:|-----------:|
| 128         | 92,028             | 77         | 540 KB    | ~10.15 s   |
| 256         | 92,028             | 77         | 540 KB    | ~10.16 s   |

Both runs completed with the same outcome (`win`), the same node count, the same root depth, and nearly identical dump sizes. `--pt-size 128` now succeeds.

### Other FENs

```bash
cargo run --release -- --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1"   --timeout 5 --pt-size 64 --dump-path /tmp/rook.bin
cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --timeout 5 --pt-size 64 --dump-path /tmp/tworooks.bin
```

- Rook mate: `outcome: win length: 1`, 2 nodes, root depth 1.
- Two-rook mate: `outcome: win length: 3`, 4 nodes, root depth 3.

`verify_ppv` validates the extracted PPV for the two-rook position as `is_ppv: true` and the rook mate trivially.

### PPV validation on the target FEN

`inspect_pt` extracts a 77-ply line from `/tmp/pt128.bin`, but `verify_ppv` refutes it:

```text
is_ppv: false
PPV refuted at defender ply 8/77, supplied move 'c6c5' is not a longest defense (depth 53, longest 55)
```

This means the proof tree stores a defender reply that is not the longest available defense at that node. The tree itself is still a valid proof of `win` (all defender replies are losing), but the line returned by `extract_ppv` is not a principal variation. This appears to be a pre-existing issue with depth consistency across transpositions / GHI positions rather than a bug introduced by the dummy-parent traversal, because `extract_ppv` uses the stored `depth` values and the same canonicalization-by-`(hash, outcome)` logic as before.

## Problems encountered and fixes

1. **Mismatched `Outcome` / `Option<Outcome>` in `binary.rs`.**
   The post-order depth recomputation in `from_bin` matched on `Outcome` directly after `ProofNode.outcome` became `Option<Outcome>`. Fixed by matching `Some(Outcome::Win)`, `Some(Outcome::Loss)`, and `_`.

2. **`finalize_copies_expanded_twin_to_unexpanded_sibling` test failure.**
   Unexpanded twin nodes were not being canonicalized because their parents were not inserted into `expanded_by_hash` when they gained children. Fixed by inserting the parent into `expanded_by_hash` after `reconcile_children` in `process_event`. (This was later subsumed by the deferred `expanded_by_hash` rebuild.)

3. **`finalize_prunes_dummy_subtree` test failure.**
   The test used the same Zobrist hash (`0`) for every node, so canonicalization mapped the root hash to a child. Updated the test to use distinct hashes for real nodes.

4. **Extreme worker slowdown / memory blow-up on longer runs.**
   The original `estimate_memory()` scanned every node after every event, making event processing O(events × nodes). Combined with `find_or_create_node` linear scans, the target FEN took several minutes and longer runs risked multi-gigabyte memory use. Fixed by adding the child move index and running memory counters.

## Unresolved parts

- **PPV extraction for the target FEN is not verified.** `extract_ppv` returns a line, but `verify_ppv` shows a defender move that is not the longest defense. The proof tree is still correct in the sense that all children are losing, but the stored depths are not consistent enough to produce a true principal variation. This may require path-aware canonicalization, a post-finalization depth consistency pass, or a change to how `verify_ppv` interprets the dump.
- **Memory budget on very long searches.** The `mpsc` channel between search and worker is unbounded. With the worker now fast enough to keep up in the 10-second target case, this is not an issue there, but very long `--timeout` runs can still build a large event backlog before the worker sets `memory_limited`. A bounded channel or an `is_full` signal from the worker would be a future safeguard.
- **`src/proof_tree/worker.rs` is ~22 KB.** It remains a single module because the handle, event loop, traversal, reconciliation, finalization, and memory accounting share the same state. `AGENTS.md` now contains a file-size justification.

## Next steps

1. Investigate the PPV depth inconsistency on the target FEN. Options:
   - Add a post-finalization depth consistency check and recompute depths from the rebuilt tree (the current rebuild already recomputes depths, so the issue may be in the canonicalization itself).
   - Make canonicalization path- or repetition-aware for GHI-sensitive positions.
2. Consider a bounded proof-event channel to avoid unbounded memory growth on long searches.
3. If the worker grows further, split it into `worker/handle.rs` and `worker/builder.rs` submodules.
