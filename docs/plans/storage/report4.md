# Implementation Report: Storage Phase 4

## Summary

Implemented the Phase 4 real proof-tree dump and PPV extraction. The pre-exit
hook now:

1. Requests the full in-memory `ProofTree` from the worker.
2. Extracts a `proof_tree_ppv` line from the tree.
3. Validates that PPV both structurally (against the tree) and by replaying the
   UCI moves on the original FEN with `Search::validate_pv`.
4. Logs `proof_tree_ppv: ...`, `ppv_valid: true|false`, and
   `proof_tree_dump: <path>`.
5. Writes the tree to `--dump-path` (default `proof_tree.sql`) using the
   existing PostgreSQL `ltree` serializer.

Also added `notation::uci_to_move` to convert UCI strings back into legal
`Move`s for validation, and new regression tests in `tests/test_proof_tree.rs`
that assert the extracted tree PPV matches the solver's returned PV.

## Changes made

- `src/proof_tree/mod.rs`
  - Added `ProofTree::is_terminal(node_id) -> bool`.
  - Added `ProofTree::extract_ppv() -> Vec<String>`:
    - `Win` (OR) nodes pick the proven `Loss` child with the smallest depth.
    - `Loss` (AND) nodes pick the `Win` child with the largest depth.
    - Stops at a terminal node.
  - Added `ProofTree::validate_ppv(pv) -> bool`:
    - Walks the tree from the root following the supplied `uci_move` sequence
      and checks that the walk ends at a terminal node.
  - Updated `ProofTree::to_sql` to cut the dump size:
    - `parent_path` is now a generated column derived from `path` instead of
      being stored and duplicated in every `COPY` row.
    - This removes ~45% of the on-disk path repetition for deep PV chains.

- `src/notation.rs`
  - Added `uci_to_move(uci, pos) -> Option<Move>`:
    - Generates all legal moves in `pos` and returns the one whose
      `move_to_uci` equals the supplied UCI string.
    - Used by the CLI and tests to replay UCI PPVs.

- `src/search/dfpn/pv.rs`
  - Made `Search::validate_pv` `pub` so the CLI can call it.
  - Added a unit test `validate_pv_accepts_three_ply_mate` for the `4KRR1`
    mate.

- `src/main.rs`
  - Restored the `--dump-path <FILE>` CLI option (default `proof_tree.sql`).
  - Updated help text accordingly.
  - In the pre-exit hook:
    - Sends `ProofMessage::GetTree` to the worker.
    - Calls `tree.extract_ppv()` and prints `proof_tree_ppv:`.
    - Validates with `tree.validate_ppv()` and then `Search::validate_pv()`
      after replaying each UCI move on a fresh copy of the root position.
    - Writes the SQL dump and prints `proof_tree_dump:`.

- `tests/test_proof_tree.rs` (new)
  - `proof_tree_ppv_matches_two_rook_mate`
  - `proof_tree_ppv_matches_m27`
  - `proof_tree_validate_ppv_accepts_extracted_line`
  - Spawns a real `ProofTreeWorker`, wires it into `Search`, solves the
    position, requests the tree, and compares `tree.extract_ppv()` to the
    solver's PV.

- `tests/test_plan6.rs`
  - Updated `m27_ppv_only` expected stdout line count from 4 to 7 to account
    for the new `proof_tree_ppv`, `ppv_valid`, and `proof_tree_dump` lines.

- `tests/test_review.rs`
  - Updated `cli_does_not_duplicate_final_output` to count lines starting with
    `outcome:` and `pv:` so the new `proof_tree_ppv:` line is not mistaken for
    an extra `pv:` block.

- `AGENTS.md`
  - Updated the `main.rs` bullet to document `--dump-path` and the new
    pre-exit output lines.

## Unit / integration tests

`src/proof_tree/mod.rs` already contains worker tests; new tests are in
`tests/test_proof_tree.rs` and `src/search/dfpn/pv.rs`.

All pass:

- `cargo test --test test_proof_tree`
- `cargo test --test test_plan6 m27_ppv_only`
- `cargo test --test test_review cli_does_not_duplicate_final_output`

## Verification

- `cargo fmt --check` passed.
- `cargo clippy --all-targets` passed (no warnings).
- `cargo doc --no-deps` built cleanly.
- `cargo test` (debug) passed.

Manual CLI checks:

```bash
# Small forced mate; dump + validation should be compact and valid
cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --no-refine-shortest
# -> outcome: win
# -> pv: f1f7 e8d8 g1g8
# -> pre_exit: reason=Complete outcome=win nodes=6
# -> proof_tree: nodes=4 win=2 loss=2 root_depth=3
# -> proof_tree_ppv: f1f7 e8d8 g1g8
# -> ppv_valid: true
# -> proof_tree_dump: proof_tree.sql

# Deep FEN where proven-subtree extraction times out; the CLI still emits the
# TT-based PV and populates the dump along that line.
cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23" --dump-path /tmp/deep.sql
# -> proof_tree: nodes=76 win=38 loss=38 root_depth=75
# -> proof_tree_ppv: e3f4 c6c5 g1e1 c5c4 ... a6a4
# -> ppv_valid: true
# -> proof_tree_dump: /tmp/deep.sql
# Dump file contains 94 lines and a real `root.e3f4.c6c5...` tree.
# Size dropped from ~31 KiB (with duplicated parent_path) to ~17 KiB.
```

## Problems encountered

1. **`extract_ppv` returned an empty vector in tests**. `Search::new()` sets
   `refine_shortest: false` by default; the `solve` convenience wrapper therefore
   takes the non-refining path, which does not run `find_ppv` and therefore
   does not emit proof-tree events. The tests now explicitly call
   `search.refine_shortest(true)` before solving.

2. **`ppv_valid` was false for a clearly legal PPV**. The CLI was converting
   every UCI move against the original (root) position, so mid-line black
   replies were not found among white's legal moves. Fixed by replaying the PV
   step by step on a cloned position in `main.rs` (and in the new unit test).

3. **`ProofTreeWorker` thread did not terminate in tests**. The `Search` struct
   held a clone of the proof-tree sender, so `handle.join()` blocked forever.
   Fixed by `drop(search)` before joining the worker in the tests.

4. **`test_plan6` and `test_review` line-count assertions broke** because the
   pre-exit hook now prints extra lines. Updated the expected counts and the
   `pv:` detection to use line-prefix matching.

## Open ends / next steps

- Phase 5: persist the proof tree to a real PostgreSQL instance or further
  polish the SQL dump format (e.g., include timestamps, batch `INSERT`s, or
  compress large dumps).
- The proof tree still records only the principal-variation path; for full
  correctness `Loss` (AND) nodes should contain every proven defender reply,
  not just the one selected by `extract_ppv`. The worker already keeps all
  children it receives; the search side must be taught to emit all defender
  replies.
- `notation::uci_to_move` is currently `O(legal_moves)`. This is fine for short
  PVs but could be replaced with direct `Move` parsing if `atomic_movegen`
  exposes a `Move::from_uci` helper.
- The `ltree` path is still materialized, so a single deep PV chain produces
  O(depth^2) bytes of path text. The `parent_path` duplication has been
  removed, but the next step is an adjacency-list (`id`, `parent_id`) dump with
  a `ltree` path populated by a recursive CTE on import, or by switching to an
  integer-array path encoding if `ltree` is not required.
