# Implementation Plan: Phase 5 — direct Postgres export

## Goal

Add optional direct export of the proof tree to a live PostgreSQL database.
This is gated behind a feature flag or a `--pg-url` CLI flag. The pre-exit hook
inserts nodes into the DB instead of (or in addition to) writing the `.sql`
file.

## Changes

1. **Dependency management**
   * Add `tokio-postgres` or `sqlx` as an **optional** dependency behind a Cargo
     feature (e.g. `pg-export`).
   * Keep the `.sql` dump path as the default behavior so the solver remains
     driver-free unless explicitly requested.

2. **`src/proof_tree/db.rs` (new file)**
   * Implement `export_to_postgres(tree: &ProofTree, url: &str) -> Result<...>`.
   * Create the same schema as `to_sql` (`proof_meta`, `proof_nodes`, indexes).
   * Use a parameterized `INSERT` or `COPY` for efficiency.
   * Reuse the sanitized `ltree` label logic from `src/proof_tree/mod.rs`.

3. **`src/main.rs`**
   * Add `--pg-url <URL>` optional flag. Mutually exclusive with writing a `.sql`
     file unless `--dump-path` is also supplied.
   * Update the pre-exit hook:
     * If `--pg-url` is present, call `export_to_postgres`.
     * Otherwise (or also, if both are requested), write the `.sql` dump.
   * Add error handling for connection failures and log the result.

4. **`src/proof_tree/mod.rs`**
   * Expose the `ltree` label sanitizer publicly so `db.rs` can reuse it.

## Test plan

* Start a Postgres container:
  `docker run -e POSTGRES_PASSWORD=... -p 5432:5432 postgres`
* Run:
  `cargo run --features pg-export -- --pg-url postgres://... --fen <FEN> --timeout 10`
* Query the DB:
  * `SELECT * FROM proof_meta WHERE key = 'root_fen';`
  * `SELECT * FROM proof_nodes WHERE path = 'root';`
  * `SELECT * FROM proof_nodes WHERE path <@ 'root.<first_move>';`
  * `SELECT count(*) FROM proof_nodes;`
* Verify child counts for `Loss` nodes include all legal defender replies.
* Run `cargo test --features pg-export` to test the exporter with a local DB
  connection string.
* Run `cargo clippy` and `cargo fmt` with and without the feature.

## Final task

After implementation, create `docs/plans/storage/report5.md` summarizing the
additional tools/examples used, any problems encountered, open ends, and next
steps.
