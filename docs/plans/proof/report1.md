# Implementation Report: Authoritative Incremental Proof Tree

This report documents the implementation of `docs/plans/proof/plan1.md`.

## Summary

The in-memory proof tree is now authoritative. Each `ProofNode` and `NodeProven`
event carries the position Zobrist hash. The search emits events incrementally
and never clears them; the proof-tree worker accumulates the raw tree and then
runs a `finalize()` pass that copies fully expanded canonical subtrees onto
unexpanded transpositions. The previous `Search::emit_proof_tree` transposition-
table reconstruction pass has been removed. On `ExitReason::MemoryLimit` the CLI
now logs an error and exits with a non-zero status.

## Files Changed

- `src/proof_event.rs` — added `hash: u64` to `NodeProven` and updated
  `NodeProven::new`.
- `src/proof_tree/mod.rs` — added `hash: u64` to `ProofNode`, updated
  `ProofTree::new` and `ProofTree::add_node` signatures, updated unit tests.
- `src/proof_tree/worker.rs` — added `expanded_by_hash` index, `Finalize`
  message routing, `ProofTreeWorkerHandle::finalize()`, and the full
  finalization algorithm (drain events, flush pending, select canonical nodes,
  rebuild tree, recompute depths, check for unexpanded internal nodes).
- `src/proof_tree/worker/tests.rs` — updated existing tests for the new
  `NodeProven` and `handle_query` signatures; added three new tests for
  transposition copying, canonical depth selection, and finalize round-trip.
- `src/proof_tree/binary.rs` — set `hash: 0` when deserializing (the binary
  format is unchanged).
- `src/search/dfpn/mod.rs` — `emit_proof_node` now takes `pos: &Position` and
  includes `pos.hash()`; removed `clear_proof_events`, `search_depth` /
  `solve_with_progress` no longer clear events; removed the final
  `emit_proof_tree` call.
- `src/search/dfpn/core.rs` and `src/search/dfpn/children.rs` — updated all
  `emit_proof_node` and manual `NodeProven` construction to supply the hash.
- `src/search/dfpn/pv.rs` — removed `emit_proof_tree`, `emit_proof_subtree`,
  and `send_proof_node`; removed the now-unneeded `ProofEvent` import; updated
  module comment.
- `src/main.rs` — pre-exit hook now calls `pt_handle.finalize()` before
  querying stats/tree; detects `ExitReason::MemoryLimit` and exits before the
  hook.
- `tests/test_proof_tree.rs` — `solve_and_get_tree` now calls `handle.finalize()`
  before retrieving the tree.
- `AGENTS.md` and `README.md` — updated architecture and public API notes to
  describe the authoritative incremental tree, hash-based sharing, and the
  `finalize()` pass.

## Implementation Notes

- `ProofEvent::Clear` is retained as a test/debug facility but is no longer
  used by the production solver.
- The finalization pass builds a brand-new `ProofTree` rather than sharing nodes,
  as specified. This keeps the existing binary format and in-memory data model
  unchanged.
- Canonical node selection prefers nodes whose stored depth equals the depth
  implied by their children; among consistent nodes it picks the smallest stored
  depth. If no consistent node exists, it falls back to the smallest stored
  depth among expanded nodes.
- A path-hash set guards the rebuild DFS so that accidental cycles (which
  should not occur in valid Win/Loss proofs) do not cause infinite recursion.
- `flush_pending` repeatedly drains pending children whose parents now exist in
  the tree until no more progress is made.

## Verification

Ran the checks and examples requested in the plan:

- `cargo fmt --check` — clean.
- `cargo clippy --all-targets` — clean.
- `cargo test` — all tests pass (124 unit tests, plus integration tests; slow
  tests ignored as normal).
- `cargo doc` — generated without warnings.
- `cargo run -- --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1"` — reports win in one,
  dumps `proof_tree.bin`.
- `cargo run -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"` — reports win in
  three (`f1f7 e8d8 g1g8`), dumps `proof_tree.bin`.
- `cargo run --example inspect_pt -- proof_tree.bin` — shows 4 nodes, root
  depth 3, and `validate_ppv: true`.
- `cargo run --example verify_ppv -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --moves f1f7 e8d8 g1g8 --timeout 5` — reports `is_ppv: true`.

## Problems Encountered

- The `NodeProven` signature change required touching every manual test event
  in `src/proof_tree/worker/tests.rs` (21 call sites). This was handled with
  targeted `replace_all` edits; a few direct `worker.handle_event` calls used
  different indentation and required a second replacement pass.
- `cargo fmt` had to be run after the large insertions in `worker.rs` and
  `worker/tests.rs` to satisfy `--check`.

## Trade-offs and Unresolved Parts

- **Hard failure on incomplete finalization.** If the final tree contains a
  non-terminal node with no children, `finalize_tree` logs an error and calls
  `std::process::exit(1)`. This is appropriate for the CLI but makes the
  library less test-friendly; unit/integration tests must supply complete trees
  when calling `finalize()`.
- **Memory accounting for `expanded_by_hash`.** The hash-to-node-id index adds a
  small amount of memory that is not currently included in `estimate_memory()`.
  This was accepted because the existing estimate is already conservative
  (1.5x multiplier) and the index is small relative to the nodes and path
  strings.
- **DAG sharing deferred.** The plan explicitly copies subtrees rather than
  sharing nodes. This keeps the implementation simple and the binary format
  unchanged, at the cost of larger trees for highly transpositional positions.
- **No `ProofSink` trait.** The `proof_event_sender` is still exposed as an
  `Option<mpsc::Sender<ProofEvent>>`. A future `ProofSink` abstraction would
  make unit testing with a `Vec`-collecting sink cleaner.

## Missing Tests

- A test that exercises `ExitReason::MemoryLimit` through the CLI and confirms
  the non-zero exit code.
- A larger integration test that finalizes a deeper position and checks that
  every non-terminal proof-tree node has children.
- A test verifying that the binary dump of a finalized tree round-trips through
  `ProofTree::from_bin` and still validates the PPV.

## Next Steps

- Add `ProofSink` trait to decouple the solver from `mpsc::Sender` and make the
  event-based tests fully deterministic without spawning worker threads.
- Evaluate memory/performance impact of the copy-based finalization on deeply
  transpositional positions; consider DAG sharing if tree sizes become
  prohibitive.
- Add CLI and integration tests for the memory-limit exit path.
