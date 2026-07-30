# Implementation Report: Storage Phase 2

## Summary

Implemented a standalone `ProofTree` type and a PostgreSQL `ltree` SQL dump
serializer (`to_sql`). The default pre-exit hook now builds a small,
hard-coded test proof tree and writes it to `--dump-path` (default
`proof_tree.sql`) before the program exits.

## Changes made

- `src/proof_tree/mod.rs` (new module)
  - `ProofNode` and `ProofTree` as specified in `concept.md`:
    - `ProofNode`: `parent`, `uci_move`, `outcome`, `depth`, `children`.
    - `ProofTree`: `root_fen`, `nodes`, `index` (`HashMap<String, usize>`).
  - `ProofTree::new` and `ProofTree::add_node` for building trees.
  - `ProofTree::to_sql<W: Write>` that emits:
    - `CREATE EXTENSION IF NOT EXISTS ltree;`
    - `proof_meta` and `proof_nodes` table definitions.
    - Btree index on `parent_path` and GiST index on `path`.
    - `INSERT INTO proof_meta ...` for `root_fen` (single-quote escaped).
    - `COPY proof_nodes (...) FROM STDIN` block with tab-separated rows.
  - `sanitize_label`: lower-cases UCI moves, keeps ASCII alphanumeric,
    underscores and hyphens, replaces any other character with `_`, and prepends
    `_` if the first character is a digit.
- `src/lib.rs`
  - Re-exports `proof_tree`.
- `src/main.rs`
  - Parses `--dump-path <FILE>` (default `proof_tree.sql`).
  - `make_test_proof_tree` helper builds a three-node Win/Loss/Win tree.
  - The default pre-exit hook writes that tree to `dump_path` via `to_sql`.
  - `--outcome-only` still disables the hook entirely.
- `AGENTS.md`
  - Updated the `lib.rs` re-export list and the `main.rs` CLI option list to
    include `proof_tree` and `--dump-path`.
- `tests/test_plan6.rs`
  - `m27_ppv_only` line-count update from Phase 1 remains in place.

## Unit tests

`src/proof_tree/mod.rs` contains:

- `sanitize_label_lowercases_and_replaces_invalid_chars`
- `sanitize_label_handles_empty_and_leading_digit`
- `add_node_reconstructs_ltree_path` (3-node tree)
- `to_sql_serializes_small_tree` (checks schema, extension, COPY marker, and
  the three expected rows)
- `to_sql_escapes_fen_single_quotes`

All pass under `cargo test --lib proof_tree`.

## Verification

- `cargo fmt --check` passed.
- `cargo clippy --all-targets` passed.
- `cargo doc --no-deps` built cleanly.
- Manual CLI checks:
  - `cargo run -- --dump-path /tmp/t.sql --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"`
    wrote a valid-looking `/tmp/t.sql` file.
  - `--outcome-only` did not create the dump file.
  - `cat /tmp/t.sql` showed the expected extension, schema, index, meta
    insert, COPY header, three rows, and `\.` terminator.
- `cargo test --release`:
  - All unit/integration test suites pass except `test_plan6`, where
    `m27_ppv_only` and `m27_streaming_output` still fail with the pre-existing
    11-plies vs 7-plies PPV discrepancy documented in
    `docs/plans/ultimattt/report5.md`.
  - The new `proof_tree` tests pass.
- `cargo test` (debug):
  - Same two `test_plan6` failures as release, plus `m27_shortest_pv` timing
    out in debug (release passes). These are pre-existing performance/PV issues
    unrelated to the Phase 2 serializer.
- Postgres `psql` / Docker test:
  - `psql` and Docker are not installed in this environment, so the SQL dump
    was validated by inspection only. The generated file is in the documented
    format.

## Problems encountered

- `sanitize_label` and `path_for` initially caused borrow-check errors around
  `HashMap::iter` and pattern matching; resolved by using `|&(_, &v)| v == id`
  and by cloning the parent path before mutating the tree.
- `to_sql` produced a typo `PRIMARY_KEY` in the `CREATE TABLE` statement; fixed
  before running `cargo test`.
- No live Postgres was available, so `ltree` label validity was checked against
  the PostgreSQL documentation (alphanumeric ASCII + `_` + `-`, max 1000 bytes)
  rather than by loading into a real database.

## Open ends / next steps

- Phase 3: integrate a worker thread that collects proof-tree events from the
  search loop, store them in a thread-safe structure, and wire the pre-exit
  hook to dump the real (not hard-coded) tree.
- Phase 4+: implement the actual proof-node insertion points inside `dfpn` so
  `ProofTree` records the solving subtree, then eventually persist the tree
  to Postgres.
- The pre-existing `find_ppv` 11-plies issue in `test_plan6` should still be
  addressed before a live proof-tree dump can be validated against the
  expected m26/m27 shortest win.
