# Implementation Report: Storage Phase 3

## Summary

Implemented the dedicated proof-tree worker thread, `mpsc` event queue,
`--pt-size` memory cap, and `NodeProven` instrumentation of the DF-PN+
search. The pre-exit hook now requests real proof-tree statistics and logs:

```
proof_tree: nodes=<n> win=<w> loss=<l> root_depth=<d>
```

During verification the previously failing `test_plan6` m27 PV tests were also
resolved by integrating the proven-subtree PPV extraction from `docs/plans/pv/plan6.md`
(`extract_ppv_from_proven_subtree`). That fix was required because `find_ppv`
was following stale TT `best_move` chains and returning an 11-plies non-PPV
instead of the expected 7-plies line; the new extraction ignores those hints
and minimaxes the proven subtree directly.

## Changes made

- `src/proof_tree/mod.rs`
  - `ProofMessage` and `ProofResponse` enums (`NodeProven`, `GetStats`, `GetTree`,
    `Clear`).
  - `ProofStats { nodes, win_nodes, loss_nodes, root_depth }`.
  - `ProofTreeWorker` running in its own thread:
    - Receives events and inserts them into `ProofTree`.
    - Buffers out-of-order children in `pending: HashMap<String, Vec<NodeProven>>`.
    - For `Win` parents replaces the child list with the latest (shortest) `Loss`
      child; for `Loss` parents keeps every `Win` child.
    - Estimates in-memory size and sets the shared `memory_limited` stop flag when
      `--pt-size` is exceeded.
    - Replies to `GetStats` and `GetTree`.
  - `ProofTree::to_sql` from Phase 2 is still available for Phase 4.

- `src/search/dfpn/mod.rs`
  - Extended `Search` with `memory_limited`, `proof_tree_sender`, `move_stack`,
    `proof_path`, and `ppv_cache`.
  - Added `clear_proof_tree`, `emit_proof_node`, and `emit_pv_events` helpers.
  - `find_ppv` now:
    - Records a TT-based fallback PV first with `extract_pv_checked` and uses
      its length to tighten `bootstrap_success_depth`.
    - Clears the proof tree and `ppv_cache`.
    - Tries `extract_ppv_from_proven_subtree` first to obtain a correct,
      minimaxed PPV without trusting TT `best_move` hints.
    - If the proven-subtree pass times out, emits the TT-based fallback PV so
      the proof tree is not left with only the empty root.
    - Otherwise falls back to the old `extract_ppv` / `extract_pv` chain.
    - Emits the final PV as `NodeProven` events for the worker.
  - `refine_sppv` emits the final shortest PV as events.
  - `exit_reason` returns `MemoryLimit` when the worker flag is set.
  - `time_exceeded` checks the `memory_limited` flag as well as the deadline and
    `stop_flag`.

- `src/search/dfpn/core.rs`
  - `dfpn` signature extended with `in_proof_tree: bool`.
  - Emits `NodeProven` at terminal nodes, TT-resolved nodes, and solved stores.
  - During recursive child expansion updates `move_stack` and `proof_path` so
    emitted events carry the correct `ltree` path.

- `src/search/dfpn/children.rs`
  - `evaluate_all_children` and `evaluate_child` propagate `in_proof_tree` and
    emit `NodeProven` when a child is proven.

- `src/search/dfpn/pv.rs`
  - Added `extract_ppv_from_proven_subtree` with alpha-beta depth pruning and
    a `ppv_cache` keyed by `(pos.hash(), path_code, expected)`.
  - The extractor picks the shortest winning attacker move and the longest
    defender reply, verifying every reply at `Loss` nodes.

- `src/main.rs`
  - Parses `--pt-size <MB>` (default `256`).
  - Spawns `ProofTreeWorker` and wires the sender / `memory_limited` flag into
    `Search` unless `--outcome-only` is given.
  - Pre-exit hook requests `GetStats` and logs the `proof_tree:` summary.
  - Prints `memory` and `reason=MemoryLimit` when the proof-tree budget is hit.
  - Removed `--dump-path` from this phase; the SQL dump will be re-enabled in
    Phase 4.

- `src/position.rs`
  - `Outcome` now derives `Hash` so it can be used in the `PpvCache` key.

- `AGENTS.md`
  - Updated CLI option list and module descriptions to reflect the proof-tree
    worker and `--pt-size`.

- `tests/test_plan6.rs`
  - `m27_ppv_only` line-count remains at 4 to account for the `proof_tree:`
    stats line.

## Unit tests

`src/proof_tree/mod.rs` tests:

- `worker_handles_out_of_order_events`
- `worker_loss_parent_keeps_all_win_children`
- `worker_replaces_win_child_with_shortest_loss`
- `worker_sets_memory_limited_flag`
- `to_sql_serializes_small_tree`
- `to_sql_escapes_fen_single_quotes`
- `sanitize_label_*`

`src/search/dfpn/pv.rs` added:

- `extract_ppv_from_proven_subtree_finds_shortest_win`

All pass under `cargo test`.

## Verification

- `cargo fmt --check` passed.
- `cargo clippy --all-targets` passed (no warnings).
- `cargo doc --no-deps` built cleanly.
- `cargo test` (debug) passed, including the previously failing
  `m27_ppv_only`, `m27_shortest_pv`, and `m27_streaming_output`.
- `cargo test --release` passed.

Manual CLI checks:

```bash
# Default run now prints the proof-tree stats line
cargo run -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --no-refine-shortest
# -> proof_tree: nodes=4 win=2 loss=2 root_depth=3

# m27 now returns the expected 7-plies PPV
cargo run -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" --no-refine-shortest
# -> pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8

# Memory-limit stop still returns the PV and reports MemoryLimit
cargo run -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" --pt-size 0
# -> pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8
# -> memory
# -> pre_exit: reason=MemoryLimit

# Deep position where the proven-subtree extraction cannot finish in time:
# the solver reports `timeout` but emits the TT-based PV, so the proof tree
# is populated with all nodes along that line.
cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23"
# -> pv: e3f4 c6c5 g1e1 c5c4 e1e7 ... a6a4
# -> timeout
# -> proof_tree: nodes=76 win=38 loss=38 root_depth=75
```

## Problems encountered

1. **Test suite regression in `test_plan6`**. `find_ppv` was returning the
   11-plies `b1b8 g8h7 ...` line for the m27 FEN because it followed the
   `best_move` chain stored by `solve_outcome`. The TT `best_move` was a valid
   winning continuation but not the strongest defense. Fixed by bringing in the
   `extract_ppv_from_proven_subtree` pass from `pv/plan6.md`, which minimaxes
   the proven subtree without trusting `best_move`.

2. **Worker child insertion before parent outcome known**. The worker initially
   created the root with `Outcome::Win`. Child events arriving before the root
   event were rejected because the parent outcome did not match. Fixed by
   initializing the root with `Outcome::Draw` and treating an unknown parent
   outcome as a reason to buffer the child in `pending`.

3. **`Clear` reset root to `Outcome::Win`**. The `ProofMessage::Clear` handler
   also used `Outcome::Win`, re-introducing the rejection bug. Changed to
   `Outcome::Draw`.

4. **Memory-limit race on very fast mates**. `--pt-size 0` with an instant mate
   sometimes reports `reason=Complete` because the worker sets the flag
   asynchronously and the search thread has already finished. This is a soft
   cap; with longer searches (e.g. m27 with refinement enabled) the flag is
   reliably observed and `reason=MemoryLimit` is reported.

5. **Deep positions left the proof tree empty**. For positions with a long
   forced mate, `extract_ppv_from_proven_subtree` can consume the whole time
   budget and fail before emitting any nodes, leaving `proof_tree: nodes=1`.
   Fixed by first extracting a TT-based fallback PV and emitting it when the
   proven-subtree pass times out; this populates the tree along the proven PV
   line even when the shorter/minimax PPV cannot be completed.

6. **Clippy `collapsible_if` warnings**. Fixed by chaining `let-else` conditions
   in `find_ppv`.

## Open ends / next steps

- Phase 4: have the pre-exit hook request `GetTree` and write the real
  `ProofTree` to `--dump-path`.
- The current `emit_pv_events` emits only the principal-variation path. For a
  fully correct proof tree, `Loss` (defender/AND) nodes should include *all*
  proven `Win` replies, not just the longest one. The worker already keeps all
  children it receives; the search side needs to emit all defender replies.
- Add memoization of failed (`None`) results in `ppv_cache` to avoid re-exploring
  branches that have already been proven too deep for a given `(hash, path_code,
  expected)`.
- Consider a bounded `dfpn`-per-child optimization if `extract_ppv_from_proven_subtree`
  becomes a bottleneck on very wide subtrees.
