# Implementation Plan: Phase 3 — worker thread, proof-tree statistics, and `--pt-size`

## Goal

Add the dedicated proof-tree worker thread and `mpsc` queue. Instrument `dfpn`
to emit `NodeProven { path, uci_move, outcome, depth }` events for nodes in the
proof subtree. The worker builds the real `ProofTree` and answers `GetStats`
queries. Add `--pt-size <MB>` to bound the in-memory tree; when the budget is
exceeded the worker sets the stop flag with reason `MemoryLimit`.

## Changes

1. **`src/proof_tree/mod.rs`**
   * Add `ProofMessage` and `ProofResponse` enums as described in `concept.md`.
   * Add `ProofStats { nodes, win_nodes, loss_nodes, root_depth }`.
   * Implement `ProofTreeWorker` that:
     * Runs in its own thread.
     * Receives `ProofMessage` on an `mpsc` receiver.
     * Inserts `NodeProven` events into `ProofTree`.
     * Handles out-of-order events with a `pending: HashMap<String, Vec<NodeProven>>`
       keyed by `parent_path`.
     * Updates `Win` nodes when a new best child is proven (replaces existing
       child list).
     * Replies to `GetStats` and `GetTree`.
     * Estimates memory use and sets the stop flag if `--pt-size` is exceeded.

2. **`src/search/dfpn/`**
   * Extend the recursive `dfpn` call to carry:
     * `move_stack: Vec<String>` parallel to `path_stack`,
     * an incremental `proof_path` string (`root.<uci1>.<uci2>...`),
     * `in_proof_tree: bool`.
   * Emit `NodeProven` when a node is proven and `in_proof_tree == true`:
     * `Loss` parent -> all children are in the proof tree.
     * `Win` parent -> only the selected `best_move` child is in the proof tree.
   * Reset the proof tree before the final `find_ppv` / `refine_sppv` pass so
     the tree reflects the chosen principal variation (per `concept.md`).

3. **`src/main.rs`**
   * Add `--pt-size <MB>` (default `256`).
   * Spawn the worker thread and pass the sender into the search.
   * Update the pre-exit hook to send `GetStats`, log:
     `proof_tree: nodes=<n> win=<w> loss=<l> root_depth=<d>`.
   * If the worker reports `MemoryLimit`, the hook logs `reason=MemoryLimit`.

4. **Memory estimation**
   * Estimate as `nodes * size_of::<ProofNode>()` plus string capacities plus
     `index` and `pending` map sizes, with a safety factor.
   * When over budget, set the stop flag and stop receiving new `NodeProven`
     messages.

## Test plan

* Run `cargo run` with a short timeout and compare `proof_tree:` stats against
  an expected small size for a forced-mate FEN.
* Press `q` + Enter and confirm stats are logged immediately without lost events.
* Run with a very small `--pt-size` (e.g. `1`) and confirm `reason=MemoryLimit`
  appears.
* Run `cargo test` for worker insertion, out-of-order buffering, and stats
  correctness.
* Run `cargo clippy` and `cargo fmt`.

## Final task

After implementation, create `docs/plans/storage/report3.md` summarizing the
additional tools/examples used, any problems encountered, open ends, and next
steps.
