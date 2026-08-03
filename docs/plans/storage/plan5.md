# Implementation Plan: Phase 5 — dump the entire proof tree, not just the PV/PPV

## Goal

Make the solver emit the full proven subtree so that `proof_tree.bin` contains
the complete OR-AND proof tree, not only the final PPV/SPPV line.

Direct PostgreSQL export is **out of scope for this milestone**; the compact
binary adjacency dump from Phase 4.1 is the supported output, and the full tree
is the last missing piece of the storage milestone.

## Background

`report4-1` replaced the `ltree` SQL dump with a compact binary adjacency dump
and switched `ProofNode` to store `atomic_movegen::types::Move` values. The
binary format is already capable of holding an arbitrary tree (parent id +
16-bit move code per node). What ends up in the dump, however, is whatever the
`ProofTreeWorker` receives from `NodeProven` events.

Current event sources:

* `solve_outcome` runs `dfpn` with `in_proof_tree=false`, so it emits nothing.
* `find_ppv` clears the tree, runs `extract_ppv_from_proven_subtree` (which does
  **not** emit events), and then emits only the final PPV line via
  `emit_pv_events`.
* `refine_sppv` runs `dfpn` with `in_proof_tree=true` (which leaves partial
  search traces) and finishes with `emit_pv_events` for the shortest line.

Consequently the dumped `proof_tree.bin` is essentially the PPV line plus a few
partial nodes from refinement. The in-memory worker already knows how to attach
out-of-order children and how to keep all children of a `Loss`/AND node; it
just never receives them.

## Changes

### 1. `src/search/dfpn/pv.rs` — emit while traversing the proven subtree

Extend `extract_ppv_from_proven_subtree` to optionally emit `NodeProven`
events for every node it visits.

* Add an `emit_events` flag to the function (or add a thin wrapper).
* Re-use `self.move_stack` and `self.proof_path` to build the
  `root.<uci1>.<uci2>...` label that the worker needs for out-of-order
  attachment. Reset both at the start of the pass (`reset_search_state` already
  does this).
* Before recursing into a child, push the child's move onto `self.move_stack`
  and append its UCI label to `self.proof_path`; pop/truncate after the
  recursion returns.
* Emit a `NodeProven` event whenever a node's result is finalized:
  * terminal leaves (`depth == 0`), and
  * internal nodes after the child loop.
  The event carries:
  * `path` — the current `self.proof_path`,
  * `mv` — the move from the parent (`Move::NONE` for the root),
  * `outcome` — the node's proven outcome,
  * `depth` — the node's proven distance to a terminal.
* For `Win`/OR nodes the function already evaluates children until the shortest
  decisive reply is found; emit all of them and let the worker keep the shortest.
* For `Loss`/AND nodes the function evaluates every legal reply; emit every
  child so the worker keeps the complete defender branching.
* Clear `ppv_cache` at the start of an emitting pass so each path is emitted at
  most once during that pass. (Transpositions that share a `path_code` are
  already shared; all other paths are distinct nodes in the proof tree.)

### 2. `src/search/dfpn/mod.rs` — wire emission into the staged search

`find_ppv`:

* After `reset_search_state`, `clear_proof_tree`, and `ppv_cache.clear()`, call
  the emitting version of `extract_ppv_from_proven_subtree`.
* If it succeeds, the full proof tree is already in the worker; return the PPV.
* If the extraction times out, fall back to the TT-based PV and emit just the
  PPV line with `emit_pv_events`, so the dump still contains a valid line.

`refine_sppv`:

* Run the SPPV probes with `in_proof_tree=false` so partial searches do not
  pollute the worker.
* Record the PV length before refinement starts.
* At the end, if `last_pv` is strictly shorter and time remains:
  * clear the proof tree,
  * clear `ppv_cache`,
  * reset search state,
  * run the emitting `extract_ppv_from_proven_subtree` for the final
    `last_pv`.
* If the final rebuild runs out of time, fall back to `emit_pv_events` for the
  final PV line.
* Remove the unconditional `emit_pv_events` call at the end of `refine_sppv` so
  the tree is not collapsed to a single line when the full tree could be built.

(If the `in_proof_tree` flag, `move_stack`, and `proof_path` bookkeeping in
`dfpn` are no longer used after these changes, remove them as cleanup. If
removal is too invasive, simply ensure `dfpn` no longer emits proof-tree events
and document that the proven-subtree extractor is the single source of truth.)

### 3. `src/proof_tree/mod.rs` — keep the shortest OR child

`ProofTreeWorker::attach_child` currently overwrites a `Win` node's existing
child depth with a deeper one if a later event for the same path arrives. Fix
this:

* For `Win`/OR parents, replace the existing child only when the new event's
  `depth` is **strictly smaller** than the current shortest child. Ignore equal
  or deeper duplicates.
* For `Loss`/AND parents, keep accumulating every distinct child. If the same
  path is seen again, only update its depth when the new depth is smaller
  (since a `Win` child represents a shortest attacker win from that node).

Add or extend unit tests:

* `worker_replaces_win_child_with_shortest_loss` already covers the OR case;
  extend it to verify that a deeper duplicate is ignored, not appended.
* Add `worker_loss_parent_keeps_all_distinct_children` (or extend the existing
  `worker_loss_parent_keeps_all_win_children`) with unequal depths.

### 4. `src/main.rs` — no structural change

The pre-exit hook already requests the tree from the worker, prints
`proof_tree_ppv`, validates it, and writes `tree.to_bin`. No change is needed
there; the dump will now be full because the worker receives the full tree.

Optionally add a diagnostic line such as `proof_tree: nodes=... win=... loss=...
root_depth=... max_width=...` to make it easy to see that the tree contains
branches, not just a line.

### 5. Documentation updates

* `docs/plans/storage/concept.md`: replace the Phase 5 bullet
  "direct Postgres export" with "full proof-tree dump in the compact binary
  format; Postgres import remains an external-loader/post-MVP option."
* `AGENTS.md`: update the `proof_tree` description to say the binary dump now
  contains the full proven subtree, not just the PPV line.

## Test plan

* `cargo test` and `cargo test --test test_proof_tree` — existing PPV-match
  tests must still pass; `tree.extract_ppv()` must equal the solver's returned
  PV.
* Add `proof_tree_contains_defender_replies` in `tests/test_proof_tree.rs`:
  solve a forced-win position where a defender node has more than one legal
  reply; assert that the in-memory `ProofTree` contains a `Loss` node with
  `> 1` `Win` child and that `tree.validate_ppv` accepts the extracted PPV.
* Manual CLI checks on known FENs:
  * `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` — `proof_tree.bin` should be small but
    still contain the forced replies.
  * `6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26` — the dump should
    contain many more nodes than `pv.len() + 1` and `ppv_valid` should remain
    `true`.
* Round-trip: load `proof_tree.bin` with `ProofTree::from_bin` and confirm
  `extract_ppv` / `validate_ppv` still work.
* `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
  `cargo doc --no-deps`.

## Next steps

* Direct PostgreSQL export is deferred; the existing `proof_tree.bin` spec is the
  stable interface and an external loader can still import it.
* The proof tree is still duplicated per path; transposition merging remains a
  post-MVP optimization.
* Future work can add an `ltree` path rebuild in an external loader, or a
  `--pg-url` feature-gated exporter that reads the binary dump.

## Final task

After implementation, create `docs/plans/storage/report5.md` summarizing the
emission strategy, the worker fix, any problems encountered, unresolved parts, and next
steps.
