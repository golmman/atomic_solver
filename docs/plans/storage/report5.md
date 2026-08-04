# Implementation Report: Phase 5 — dump the entire proof tree

## Summary

Made the solver emit the complete proven OR-AND subtree, so `proof_tree.bin`
now contains the full proof tree instead of just the final PPV/SPPV line.
Direct PostgreSQL export remains out of scope; the compact binary adjacency
dump from Phase 4.1 is the stable output.

## Changes made

- **`src/search/dfpn/pv.rs`**
  - Split `extract_ppv_from_proven_subtree` into a `#[cfg(test)]` wrapper and an
    internal `_impl` that accepts an `emit` flag.
  - Added `extract_ppv_from_proven_subtree_emit`, which emits `NodeProven`
    events for every finalized node while walking the proven subtree.
  - Reuses `move_stack` and `proof_path` to build `root.<uci1>.<uci2>...`
    labels; pushes/pops the child's move around each recursive call.
  - Emits a terminal leaf when `pos.outcome()` matches the expected outcome and
    emits an internal node after its children are resolved.
  - Fixed a pre-existing `path_stack` imbalance by popping the current node's
    repetition key before terminal or `remaining == 0` returns.

- **`src/search/dfpn/mod.rs`**
  - `find_ppv` now calls `extract_ppv_from_proven_subtree_emit` and no longer
    collapses the tree to a single line with `emit_pv_events` on success; it
    still falls back to `emit_pv_events` if extraction times out.
  - `refine_sppv` now runs SPPV probes with `in_proof_tree = false`, records
    the initial PV length, and only rebuilds the full proof tree when a strictly
    shorter SPPV is found. The rebuild uses the emitting extractor; if it times
    out, it falls back to `emit_pv_events` for the final PV line.
  - `refine_sppv` only replaces the existing `last_pv` on an equal-length probe
    when no PPV was known yet, preventing a mismatch between the final solver
    output and the proof tree stored in the worker.
  - Removed the unconditional `emit_pv_events` at the end of `refine_sppv`.

- **`src/proof_tree/mod.rs`**
  - Fixed `ProofTreeWorker::attach_child`:
    - `Win`/OR parents keep only the shortest decisive child; a deeper or equal
      duplicate is ignored, and the same path is only updated when the new depth
      is smaller.
    - `Loss`/AND parents accumulate every distinct child; a duplicate path is
      only updated when the new depth is smaller.
  - Extended unit tests to cover deeper duplicates on `Win` parents and
    unequal depths on `Loss` parents.

- **`tests/test_proof_tree.rs`**
  - Added `solve_and_get_tree` helper.
  - Added `proof_tree_contains_defender_replies` to verify that a `Loss` node in
    the dumped tree has more than one `Win` child.
  - Added `proof_tree_bin_round_trips_full_tree` to serialize the full tree to
    the compact binary format and confirm `extract_ppv` / `validate_ppv` still
    work after loading.

- **`AGENTS.md` and `docs/plans/storage/concept.md`**
  - Updated proof-tree and CLI descriptions to say the binary dump now contains
    the full proven subtree.
  - Replaced the Phase 5 "direct PostgreSQL export" bullet with a full
    proof-tree dump entry; Postgres import is described as an external-loader
    option.

## Verification

- `cargo fmt --check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test` passed (all unit and integration tests, including the new
  `proof_tree_contains_defender_replies` and
  `proof_tree_bin_round_trips_full_tree`).
- `cargo doc --no-deps` passed.
- Manual CLI checks:
  - `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1`:
    - `proof_tree: nodes=4 win=2 loss=2 root_depth=3`
    - `proof_tree_ppv: f1f7 e8d8 g1g8`
    - `ppv_valid: true`
  - `6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26`:
    - `proof_tree: nodes=54 win=25 loss=29 root_depth=7`
    - `proof_tree_ppv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8`
    - `ppv_valid: true`
    - The dump contains far more nodes than `pv.len() + 1`.

## Performance regression and fix

After the full-tree dump landed, a deep position (`4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23`) was observed to spend roughly 90s in the `pre_exit` hook producing `proof_tree.bin`, even though the search itself finished in a few seconds. Profiling showed the delay was inside `ProofTreeWorker::run`, not the binary serializer.

Root cause: `ProofTreeWorker::estimate_memory()` iterated over the entire `pending` `HashMap` on every `NodeProven` event. With ~200k events and a pending queue that grew to ~150k entries, this made event handling quadratic. The same pending vectors were re-summarized on every call. The `NodeProven` volume is a consequence of the full proven-subtree extractor exploring deep defender lines; the worker should not make each event costlier as the pending queue grows.

Fix:
- `src/proof_tree/mod.rs`: `ProofTreeWorker` now maintains `index_path_bytes`, `pending_path_bytes`, and `pending_event_count` incrementally. `estimate_memory()` is O(1) and just multiplies the maintained lengths/capacities.
- `src/search/dfpn/pv.rs`: `extract_ppv_from_proven_subtree_impl` now caches proven-subtree results in `ppv_cache` even when the discovered depth exceeds the current `remaining` bound, so the same `(hash, path_code, expected)` triple is not re-proved as the bound grows.

Verification after the fix:
- `cargo test --lib --tests`, `cargo clippy -- -D warnings`, `cargo fmt --check`, and `cargo doc --no-deps` all pass.
- `cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23"` finishes inside the default 5s timeout and the binary dump is effectively instant (previously ~90s).

## Problems encountered

- The original `extract_ppv_from_proven_subtree` returned from terminal and
  `remaining == 0` branches without popping the repetition key it had just
  pushed. This left the `path_stack` unbalanced for callers. Added explicit
  `self.path_pop()` calls in those early return paths.
- The worker's old `Win`-parent logic would overwrite a stored child depth with a
  deeper value when a later event for the same path arrived. Restructured the
  comparison so only strictly smaller depths replace the selected child.
- `refine_sppv` originally replaced `last_pv` on every equal-length probe. When
  the proof tree was built for the previous `last_pv`, the new equal-length line
  would no longer match the stored tree. Fixed by recording `u32::MAX` as the
  initial length when `last_pv` is empty (so any found PPV triggers a rebuild)
  and by only accepting an equal-length probe when no PPV was known yet.

## Open ends and next steps

- Transposition merging across different paths is still not implemented; the
  proof tree duplicates transposed nodes per path, which can grow large on deep
  positions. This remains a post-MVP optimization.
- Direct PostgreSQL export is still deferred. The binary adjacency dump is the
  stable interface; an external loader can import it and, if desired, rebuild an
  `ltree`-style path from `parent_id` chains.
- Future work can add an optional `--pg-url` feature that reads the completed
  `ProofTree` and inserts rows directly, without changing the search emission
  logic.
