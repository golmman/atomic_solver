# report14 — centralized generated-data layout: `data/proofdb/`

Executes `plan14.md` (backlog item 9, owner request 2026-10-02, amended
same session). Size S: defaults + docs; no `src/` changes, no pipeline
logic changes.

## 1. What was done (per plan task order)

1. **Code (defaults + usage strings + docs-comments).**
   - `examples/proofdb/mod.rs` — new single source of truth for the
     compiled defaults: `DEFAULT_DB` (`data/proofdb/proofdb.db`),
     `DEFAULT_MANIFEST` (`data/proofdb/shards/manifest.json`),
     `DEFAULT_SHARD_DIR` (`data/proofdb/shards`), `DEFAULT_LEDGER`
     (`data/proofdb/work.json`), `DEFAULT_FLIP_OUT`
     (`data/proofdb/flip.json`), plus `create_parent_dir()` (plan14 D3
     write-path helper; reading never creates anything).
   - `examples/proofdb_merge.rs` — `--db`, `--manifest`, `--shard-dir`
     now default to the constants; the required-argument check dropped;
     usage line shows the defaults; the DB's parent directory is created
     before the DB write; header comment updated.
   - `examples/proofdb/harvest_args.rs` — `--db`, `--manifest`,
     `--shard-dir`, `--ledger` defaults; required-argument check dropped
     (nothing is required anymore); usage line updated.
   - `examples/proofdb/session.rs` — the shard dir is created on the
     first shard write (`produce_shard`) and the manifest's parent on the
     rewrite (`rewrite_manifest`).
   - `examples/proofdb_flip.rs` — `--db`, `--manifest`, `--out` defaults
     (`--out` was previously "optional, empty = don't write"; with a
     default it always writes, so the empty-out branch was removed);
     report parent dir created before the write; usage line updated.
   - Doc-comment path references: `examples/proofdb/ledger.rs`
     (`data/proofdb/`), `examples/proofdb_ledger_union.rs`
     (`data/proofdb/work.json`).
   - Defaults pinned by a new unit test
     `tests/proofdb.rs::cli_defaults_are_centralized_under_data_proofdb`
     (exact paths + directory-qualified + all under `data/proofdb/`).
2. **Manual `docs/proofdb_pipeline.md`.** §2.1 artifact table → default
   paths, all production rows gitignored via `data/`, backup
   responsibility stated; new **production vs development** paragraph in
   §2; §3 rewritten as the production-shaped quickstart (repo root, no
   path flags, cleanup = `rm -rf data/proofdb`, expectations re-pinned —
   see §6 below); §4 default ledger path + the migration one-liner; §7
   usage lines/option tables/output examples show the new defaults
   (`db: data/proofdb/proofdb.db …`, `harvest: db data/proofdb/…`); §8
   recipes restructured — new **R0** (bootstrap + optional seed-from-
   fixture), R1 now the explicit-flags fixture rebuild (digest pin
   retained), R2/R3/R5 migrated to bare defaults, R5 carries the
   migration one-liner; §10 gained the missing-default-manifest abort
   example, stale-DB fix updated; §11 plan14 history note.
3. **AGENTS.md** — the `proofdb_flip`/`proofdb_harvest`/`proofdb_merge`
   entries gained one default-path clause each (`proofdb_ledger_union`
   has no generated-data defaults; its inputs stay explicit).
4. **`.gitignore`** — comment block now names `data/proofdb/` as the
   working-layer root and states the backup responsibility (the `data/`
   rule itself unchanged).
5. **Verification** — all executed, see §6.
6. **`report14.md`** — this file; initiative item 9 and
   `docs/plans/README.md` updated (item closed this session).

## 2. Tools used

Standard toolchain only (`cargo build/clippy/fmt/doc`, `make test`,
`sha256sum`); no additional tooling.

## 3. Problems encountered

- One self-inflicted doc edit initially dropped the §7.4 heading in the
  manual (overlapping replacement); caught and restored immediately.
- The old quickstart's `--dump nodes.txt` step was dropped from §3 (the
  dump stays default-off and the production bootstrap doesn't need it);
  no consumer of that step exists.

## 4. Deviations from the plan

- None in substance. One small judgment call: `proofdb_flip --out` no
  longer supports the "empty = don't write" mode (it was undocumented and
  unreachable from the CLI surface anyway once a default exists). The
  plan's decision 2 lists `--out` → default `data/proofdb/flip.json`;
  making the default unconditional is the natural reading.
- `proofdb_ledger_union` got no default-path clause in AGENTS.md because
  it has none (plan's "four tool entries" phrasing anticipated a clause
  per tool; the ledger-union entry would have been vacuous).

## 5. Unresolved parts / missing tests

- The shard-dir creation on first write (`session.rs`) is exercised only
  indirectly (no unit test forces a fresh-directory harvest; the E2E
  smokes below covered it manually).
- The migration one-liner is a documented recipe, not an executed step —
  this checkout had no standing working-layer files (plan §0).
- Per-worker ledger files (item 6) will land under `data/proofdb/`
  naturally when that item is taken up; no scaffolding added (plan
  non-goal).

## 6. Gate and hygiene

- `make test` green (32 suites, 0 failures; includes
  `standing_layer_rebuilds_byte_identical` with explicit paths and the
  new `cli_defaults_are_centralized_under_data_proofdb`).
- `cargo clippy --release --examples --all-targets` clean;
  `cargo fmt --check` clean; `cargo doc` clean.
- **Quickstart §3 re-executed verbatim** (production-shaped, no path
  flags): bootstrap merge `db: data/proofdb/proofdb.db (nodes 1), manifest
  built_from d801aa1fb7ddcc33`; first harvest `C1 1 / C2 0 / C3 20`,
  `fresh 20, ledger-censored 0`, 2 censored jobs, `evals 400014`,
  manifest unchanged; second harvest `ledger-censored 2`, `pass 2`,
  `work_before 200009/200005`, budgets `400018/400010`, `evals 800078`;
  flip `open_rows 1 flips 0 verified 0 root Null (fixpoint 1 rounds)`;
  report at `data/proofdb/flip.json`. All pins match the manual verbatim.
- **R1 fixture rebuild with explicit flags**: census identical
  (262/262 shards, 55,703 merged nodes), digest
  `0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8` —
  unchanged.
- **Default-path smokes**: bare `proofdb_merge` against an empty default
  manifest created `data/proofdb/` + `data/proofdb/proofdb.db`
  (`db: data/proofdb/proofdb.db (nodes 1)`); bare `proofdb_flip` and bare
  `proofdb_harvest` with no default manifest aborted cleanly naming
  `data/proofdb/shards/manifest.json` (exit 1 / exit 2).
- `git status` clean of `data/` litter (quickstart state removed after
  verification; the tree is gitignored anyway).

## 7. Next steps

- Item 6 (per-worker ledgers) is the natural next tooling item; its
  files should live under `data/proofdb/` from day one.
- Item 7 (`--root-fen` subtree harvesting) remains parked.
- If production harvesting starts, the first operator action is recipe
  R0 (or R0's seed-from-fixture variant, which must reproduce R1's
  digest).
