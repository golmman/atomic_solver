# Implementation Plan: Phase 2 — dummy/test SQL `ltree` dump

## Goal

Implement the `ProofTree -> .sql` serializer independently of the search. The
pre-exit hook builds a small hard-coded test `ProofTree` and writes it to
`--dump-path` (default `proof_tree.sql`). This validates the `ltree` path
encoding, the `COPY` format, and the schema before the live proof-tree worker
is wired in.

## Changes

1. **New module `src/proof_tree/mod.rs`**
   * Define `ProofNode` and `ProofTree` from `concept.md`:
     ```rust
     pub struct ProofNode {
         pub parent: Option<usize>,
         pub uci_move: String,
         pub outcome: Outcome,
         pub depth: u32,
         pub children: Vec<usize>,
     }
     pub struct ProofTree {
         pub root_fen: String,
         pub nodes: Vec<ProofNode>,
         pub index: HashMap<String, usize>,
     }
     ```
   * Implement `ProofTree::to_sql<W: Write>(&self, writer: &mut W)` that emits:
     * `CREATE EXTENSION IF NOT EXISTS ltree;`
     * `proof_meta` and `proof_nodes` table definitions.
     * Gist/btree indexes.
     * `INSERT` of `root_fen`.
     * `COPY ... FROM STDIN` block with tab-separated rows.
   * Sanitize each UCI move into a valid `ltree` label (alphanumeric,
     underscores, hyphens only). Reject or rewrite any other characters.
   * Reconstruct full `ltree` paths by walking the `Vec`-backed tree during DFS.

2. **`src/main.rs`**
   * Add `--dump-path <FILE>` (default `proof_tree.sql`).
   * In the Phase 2 hook, build a hard-coded test `ProofTree` with at least a
     root Win node, one Loss child, and one Win grandchild, then call
     `to_sql` and write it to disk.

3. **Tests**
   * Unit test in `src/proof_tree/mod.rs` that serializes a small tree and
     compares against an expected SQL snippet.
   * Test path reconstruction on a 3-node tree.
   * Test UCI-to-label sanitization for normal and edge-case moves.

## Test plan

* Run `cargo run` and load `proof_tree.sql` with `psql < proof_tree.sql` into
  a local Postgres instance (or `docker run -e POSTGRES_PASSWORD=... postgres`).
* Query `SELECT * FROM proof_nodes WHERE path = 'root';` and
  `SELECT * FROM proof_nodes WHERE path <@ 'root.e2e4';` to validate `ltree`
  labels and ancestry.
* Run `cargo test`, `cargo clippy`, and `cargo fmt`.

## Final task

After implementation, create `docs/plans/storage/report2.md` summarizing the
additional tools/examples used, any problems encountered, open ends, and next
steps.
