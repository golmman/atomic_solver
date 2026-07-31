# Implementation Report: Phase 4.1 — compact binary proof-tree dump

## Summary

Replaced the PostgreSQL `ltree` `.sql` dump from Phase 4 with a compact, driver-free
binary adjacency dump. The in-memory `ProofNode` now stores the actual
`atomic_movegen::types::Move` instead of a UCI string, shrinking both the
in-memory tree and the emitted event payload. The `ltree` path strings are now
used only inside the worker to attach out-of-order `NodeProven` events; they
never appear in the dump.

## Changes made

- **`src/proof_tree/binary.rs` (new)**
  - `move_to_bits(mv: Move) -> u16` and `bits_to_move(code: u16) -> Option<Move>`
    encode/decode moves using only `atomic_movegen`'s public API
    (`from_sq`, `to_sq`, `move_type`, `promotion_type`, `Square::from_u8`, and the
    `make_*` constructors). No `unsafe` or private `u16` access is used.
  - `write_proof_tree` and `read_proof_tree` implement the format from
    `docs/spec/proof_tree_dump.md`: `ATOMTREE` magic, version 1, FEN + `\n`,
    `root_outcome`/`root_depth`, then `parent_id: u32 LE` and `move_code: u16 LE`
    per node.
  - `read_proof_tree` reconstructs `outcome` by parity from the root and `depth`
    by a post-order traversal, and rebuilds the path index so the loaded tree is
    fully usable.
  - Unit tests cover normal moves, promotions to Q/R/B/N, en-passant, castling,
    and `Move::NONE`, and verify the worked-example move codes from the spec.

- **`src/proof_tree/mod.rs`**
  - `ProofNode` and `NodeProven` now use `mv: Move` instead of `uci_move: String`.
  - Added `ProofTree::to_bin` and `ProofTree::from_bin` delegating to
    `binary.rs`.
  - `extract_ppv` returns `Vec<Move>`; `validate_ppv` takes `&[Move]`.
  - Removed `to_sql` and `sanitize_label`; `add_node` builds path labels from
    `move_to_uci`.
  - Updated worker memory estimate to account for the smaller `ProofNode` and
    `NodeProven` payloads.
  - Replaced all SQL-related unit tests with binary round-trip and event tests
    using `Move` values.

- **`src/search/dfpn/mod.rs` and `src/search/dfpn/children.rs`**
  - `Search.move_stack` changed from `Vec<String>` to `Vec<Move>`.
  - `emit_proof_node`, `emit_pv_events`, and the child-proven event in
    `evaluate_child` now send `NodeProven { path, mv, outcome, depth }`.
  - `proof_path` is still built from UCI labels for the worker's event ordering,
    but the stored proof node keeps the `Move` value directly.

- **`src/main.rs`**
  - `--dump-path` now defaults to `proof_tree.bin`.
  - Pre-exit hook writes `tree.to_bin` instead of `tree.to_sql`.
  - `proof_tree_ppv` is printed by converting `Move`s to UCI with `pv_str`.
  - PPV validation now combines `tree.validate_ppv` and `Search::validate_pv`
    directly on the `Vec<Move>`.
  - Help text and doc comments updated to describe the binary dump.

- **`tests/test_proof_tree.rs`**
  - Updated to compare the proof-tree `Vec<Move>` against the solver PV, using
    `move_to_uci` only for assertion messages.

- **`AGENTS.md` and `docs/plans/storage/concept.md`**
  - Updated to describe the `Move`-based proof tree, `src/proof_tree/binary.rs`,
    and the binary adjacency dump.

- **`scripts/load_proof_tree.py` (new, optional reference loader)**
  - Reads `proof_tree.bin`, derives `outcome`, `depth`, `terminal`, and
    `uci_move`, and either prints a CSV or inserts rows into PostgreSQL when
    given `--db-url`.

## Verification

- `cargo fmt --check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test` passed (all 60 unit tests and integration tests, including
  `proof_tree_ppv_matches_two_rook_mate`, `proof_tree_ppv_matches_m27`, and
  `proof_tree_validate_ppv_accepts_extracted_line`).
- `cargo doc --no-deps` passed.
- Manual CLI check on `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` produced:
  - `proof_tree_ppv: f1f7 e8d8 g1g8`
  - `ppv_valid: true`
  - `proof_tree_dump: proof_tree.bin`
  - `proof_tree.bin` size: 70 bytes for a 4-node proof tree.
- `python3 scripts/load_proof_tree.py proof_tree.bin` correctly decoded the
  dump and derived `outcome`/`depth`/`terminal` for each node.

## Problems encountered

- `MoveType` is `#[non_exhaustive]`, so `match mv.move_type()` in
  `binary.rs` required a wildcard arm even though all four variants were listed.
  Added `_ => unreachable!()` to satisfy the compiler.
- `cargo clippy` flagged a needless `for i in 1..node_count` loop in
  `read_proof_tree`; rewrote it as `nodes.iter().enumerate().skip(1)`.

## Open ends and next steps

- The reference loader (`scripts/load_proof_tree.py`) currently reconstructs
  UCI for castling by mapping the standard king/rook-square pairs to the
  canonical `e1g1`/`e1c1`/`e8g8`/`e8c8` form. If `atomic_movegen` ever emits
  castling moves with a different `to_sq`, the loader may need to match more
  cases.
- `read_proof_tree` validates that the derived `root_depth` matches the header.
  A future version could also validate parity consistency across the whole tree
  or surface warnings for partial trees produced by an interrupted search.
- Phase 5 can now build on the binary dump: a `--pg-url` flag or feature-gated
  export can read the in-memory `ProofTree` and insert rows directly, reusing the
  same adjacency records rather than `ltree` labels.
