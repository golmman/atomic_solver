# Implementation Plan: Phase 4.1 — compact binary proof-tree dump

## Goal

Replace the `ltree` SQL dump from Phase 4 with a compact binary adjacency dump.
The Phase 4 PPV extraction and validation logic is kept; only the
serialization format and the in-memory move representation change.

## Background

`report4.md` showed the `proof_tree.sql` dump dropping from ~31 KiB to ~17 KiB
after removing the duplicated `parent_path` column, but the `ltree` path text
itself is still repeated in every row and grows as `O(depth²)` for a single
deep PV chain.

`atomic_movegen::types::Move` is a 16-bit packed value. Its public API
(`from_sq`, `to_sq`, `move_type`, `promotion_type`, `Square::from_u8`, and the
`make_*` constructors) allows lossless encoding/decoding to a `u16` without
accessing private fields and without `unsafe`.

Storing `Move` in `ProofNode` instead of a UCI string also shrinks the
in-memory tree and avoids re-parsing UCI when writing the dump.

## Binary format

```
<8-byte magic> "ATOMTREE"
<1-byte version> 1
<FEN>\n
<root_outcome: u8>    // 0 Draw, 1 Win, 2 Loss
<root_depth: u32 LE>

for each node (root first, topological order):
    parent_id: u32 LE   // u32::MAX for the root
    move_code: u16 LE   // Move::NONE == 0 for the root
```

`move_code` layout (identical to `Move`'s documented bit layout):

- bits 0-5: `to_sq`
- bits 6-11: `from_sq`
- bits 12-13: move type (0 Normal, 1 Promotion, 2 EnPassant, 3 Castling)
- bits 14-15: promotion piece index (0 Queen, 1 Rook, 2 Bishop, 3 Knight)

`outcome` is derivable by parity from the root. `depth` is derivable via a
post-order traversal: terminal nodes have `depth == 0`; `Win` nodes are
`1 + min(child depths)`; `Loss` nodes are `1 + max(child depths)`.

## Changes

1. **`src/proof_tree/binary.rs` (new)**
   - `move_to_bits(mv: Move) -> u16`
   - `bits_to_move(code: u16) -> Option<Move>`
   - `write_proof_tree<W: Write>(tree: &ProofTree, writer: &mut W) -> io::Result<()>`
   - `read_proof_tree<R: Read>(reader: &mut R) -> io::Result<ProofTree>` (for
     round-trip tests and external loaders)

2. **`src/proof_tree/mod.rs`**
   - Change `ProofNode` and `NodeProven`:
     - replace `uci_move: String` with `mv: Move`
     - keep `path: String` in `NodeProven` so the worker can still attach
       out-of-order events using its path index
   - Add `ProofTree::to_bin<W: Write>(&self, writer: &mut W) -> io::Result<()>`
     that delegates to `binary::write_proof_tree`.
   - Update `extract_ppv` to return `Vec<Move>`; callers convert to UCI for
     display.
   - Update `validate_ppv` to take `&[Move]`.
   - Update `add_node` and unit tests to use `Move`.
   - Remove `to_sql` and `sanitize_label` (the binary path no longer needs
     `ltree` labels). If `docs/plans/storage/plan5.md` direct Postgres is
     implemented later, it can reuse the same adjacency records rather than the
     old `ltree` labels.

3. **`src/search/dfpn/` (`children.rs` / `core.rs`)**
   - Where `NodeProven` is emitted, pass the `Move` value directly. The
     `proof_path` string may still be built from UCI for the worker's path key.

4. **`src/main.rs`**
   - Change `--dump-path` default to `proof_tree.bin`.
   - Update the pre-exit hook to call `tree.to_bin(&mut file)`.
   - Print `proof_tree_ppv` by converting `Move`s to UCI.
   - Update help text and `--help` output.

5. **`src/notation.rs`**
   - Keep `move_to_uci` and `uci_to_move` for CLI/display use.
   - Optionally re-export the binary move helpers if they live here instead of
     `proof_tree/binary.rs`.

6. **`docs/plans/storage/concept.md`**
   - Update the concept doc to describe the binary adjacency format,
     `Move`-based `ProofNode`s, and the fact that `path` strings are now only
     used for worker event ordering, not for the dump.

7. **`AGENTS.md`**
   - Update `proof_tree/mod.rs` and `main.rs` descriptions to reflect binary
     dump and `Move`-based proof tree.

8. **`docs/spec/proof_tree_dump.md` (new)**
   - Write a standalone format specification for `proof_tree.bin` covering file
     layout, move encoding, derivation of `outcome`/`depth`/`terminal`/UCI, and
     a worked example.

9. **External loader (optional, outside the solver)**
   - Provide a reference loader script (e.g. `scripts/load_proof_tree.py`) that
     reads `.bin`, derives `outcome` and `depth`, and inserts rows into a
     `proof_nodes(id, parent_id, uci_move, outcome, depth, terminal)` table.
     It can optionally rebuild an `ltree` path via a recursive CTE.

## Test plan

- Round-trip tests for `move_to_bits` / `bits_to_move` on:
  - normal moves,
  - promotions to N/B/R/Q,
  - en-passant,
  - castling,
  - `Move::NONE`.
- Unit test for `ProofTree::to_bin` / `from_bin` on a 3-node Win/Loss/Win tree.
- Update existing `proof_tree` tests:
  - replace `to_sql` tests with `to_bin` tests,
  - update `extract_ppv` / `validate_ppv` expectations to use `Move`.
- Regression tests in `tests/test_proof_tree.rs` still compare the extracted
  tree PPV to the solver PV.
- Run `cargo test`, `cargo clippy`, `cargo fmt`, `cargo doc`.
- Manual CLI checks:
  - solve a forced mate and confirm `proof_tree.bin` is tiny and
    `proof_tree_ppv` is still valid;
  - inspect the binary with `xxd` or the reference loader script.

## Final task

After implementation, create `docs/plans/storage/report4-1.md` summarizing the
chosen format, the `Move` encoding, any problems encountered, open ends, and
next steps (e.g. external loader, Phase 5 direct Postgres export).
