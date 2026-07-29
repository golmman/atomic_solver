# Implementation Plan: Phase 4 — real dump + PPV extraction

## Goal

Reuse the Phase 2 serializer to dump the real `ProofTree` produced by the
worker. Implement `extract_ppv_from_proof_tree` and have the pre-exit hook log
the extracted PPV. Validate the PPV with `Search::validate_pv` and the existing
`verify_ppv` example.

## Changes

1. **`src/proof_tree/mod.rs`**
   * Implement `ProofTree::extract_ppv(&self) -> Vec<String>`:
     * Start at the root node.
     * If node is `Win` (OR), take its only child.
     * If node is `Loss` (AND), take the child with the largest `depth`.
     * Append the child's `uci_move` and repeat until `depth == 0`.
   * Add helper `is_terminal(&self, node_id: usize) -> bool`.

2. **`src/proof_tree/mod.rs` (validation)**
   * Add `validate_ppv(&self, pv: &[String]) -> bool` that walks the tree and
     checks the PV exists from the root, ending at a terminal node.

3. **`src/search/` or `src/notation.rs`**
   * Ensure `Search::validate_pv` (or an equivalent) exists and can replay a UCI
     move list from the root FEN to confirm the PPV is legal and ends in a
     terminal position. If it does not exist, add a minimal implementation.

4. **`src/main.rs`**
   * In the pre-exit hook:
     * Request the full tree (`GetTree`).
     * Call `extract_ppv_from_proof_tree`.
     * Log the PPV as space-separated UCI moves.
     * Call `Search::validate_pv` and log `ppv_valid=true|false`.
     * Write the SQL dump to `--dump-path`.

5. **`examples/verify_ppv.rs`**
   * If not already present, ensure it can read a generated `.sql` dump or the
     root FEN plus a UCI line and confirm the PPV is a valid proof.

## Test plan

* Run the solver on known regression FENs, dump `proof_tree.sql`, and load it
  in Postgres.
* Compare `extract_ppv_from_proof_tree` output to `Search::find_ppv` / `refine_sppv`.
* Run `cargo run --example verify_ppv -- --fen <FEN> --moves <PPV>` and confirm
  it passes.
* Verify the dumped `.sql` contains the real tree, not the Phase-2 dummy.
* Run `cargo test`, `cargo clippy`, and `cargo fmt`.
* Add regression tests in `tests/` that assert PPV extraction matches the
  existing solver output on small forced-mate positions.

## Final task

After implementation, create `docs/plans/storage/report4.md` summarizing the
additional tools/examples used, any problems encountered, open ends, and next
steps.
