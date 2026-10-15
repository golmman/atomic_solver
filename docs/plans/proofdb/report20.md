# report20 — censor state into the DB: the one-artifact frontier

Executes `plan20.md` (initiative item 17, added with the plan). Gate:
`make test` green — all suites 0 failures (release profile); `cargo fmt
--check`, `cargo clippy --all-targets`, and `cargo doc` clean. No `src/`
changes — examples-side tooling only, as planned.

## What was done

**S1 — schema v2** (`examples/proofdb/schema.rs`):

- `write_db` creates the additive `censor` table (`path TEXT PRIMARY KEY,
  work_done INTEGER NOT NULL, passes_failed INTEGER NOT NULL`) and writes
  `meta.schema_version = "2"`; it takes a new `censor: &[CensorRow]`
  parameter so the preserved/imported rows land inside the same fresh-file
  creation (a rebuild can never lose them mid-flight).
- New helpers: `CensorRow`, `read_censor_rows` (a missing table reads as
  **empty**, not an error — the merger's preserve-and-rebuild must not fail
  on a pre-v2 file), `insert_censor_row` (one autocommit transaction per
  call — the per-censor durability discipline; a `work_done` beyond the
  i64 storage width is a clean abort, never a wrap).
- Canonical dump header echoes `schema_version 2` (dump format itself
  unchanged — it covers the tree surfaces only).

**S2 — ledger backend swap** (`examples/proofdb/ledger.rs`, plan D2–D4):

- The in-memory `Ledger` (`BTreeMap` + `bump`/`insert_fresh`/lineage-drop
  semantics) and all selection code are untouched; only persistence
  changed. `Ledger::load_db(conn)` replaces `Ledger::load(path)`; a missing
  `censor` table aborts cleanly with the upgrade hint ("re-run
  proofdb_merge … schema v2"). `Ledger::save` and the JSON codec are gone;
  `Ledger::load_legacy_json` keeps a minimal read-only parser for the
  migration only. A missing legacy file is an **error** for the import flag
  (an operator mistake, not fresh state — the old fresh-init semantic died
  with the file backend; a deviation from the draft plan's "loads as
  empty", judged safer).
- `on_censored(ledger, censor_conn, key, evals)`: commit-then-bump — the
  `INSERT OR REPLACE` runs first, so a failed commit leaves even the
  in-memory map untouched (plan D4's abort semantics, strengthened);
  a commit failure aborts the batch like the old ledger-save failure.
- New `Ledger::upsert_max` — the per-field max rule (each of
  `work_done`/`passes_failed` keeps the max of stored vs incoming):
  total, deterministic, idempotent, never decreases a stored value.
  Note: the retired `proofdb_ledger_union` used a lexicographic
  `(passes, work)` tuple rule; the import uses the plan's explicit
  per-field max, which is strictly more conservative on `work_done`
  (corner case: stored `(pass 2, work 100)` + imported `(pass 1,
  work 10⁹)` → union kept the stored row, import keeps pass 2 **and**
  work 10⁹ — never a loss of recorded knowledge).
- `--ledger` is removed from `harvest_args.rs`; passing it aborts with a
  migration hint (better than silent unknown-flag usage).

**S3 — merge preservation + import** (`examples/proofdb_merge.rs`, D5/D6):

- `collect_censor_rows`: if the DB file exists, its censor rows are read
  (read-only) *before* `write_db` replaces the file and re-inserted into
  the fresh DB — bare merge-after-batch and recipe R5 can no longer wipe
  selection state. With `--import-censor <work.json>` the legacy ledger is
  merged under `upsert_max` (work values saturate at `i64::MAX` on
  import). Merge never modifies censor rows; only harvest does (the D3
  carve-out: harvest writes `censor` rows only — one RW connection opened
  without `SQLITE_OPEN_CREATE`, so a missing DB stays a clean abort).
- Stdout: a `censor:` census line is printed only when censor state is
  actually carried (`censor: P censor rows preserved + I imported records
  -> T rows`, or `censor: P censor rows preserved`) — a fresh-layer
  bootstrap merge keeps its exact pre-plan20 stdout, so the §3.1 pins did
  not move.

**S4 — retirement**: `examples/proofdb_ledger_union.rs` and
`examples/proofdb/ledger_union.rs` deleted; `mod.rs` registration and the
union unit tests removed (the `ledger_of`/`entries` test helpers stay —
the walk tests use them). AGENTS.md row removed.

**Docs (S5)**: spec amended additively (title/schema v2, the `censor` DDL +
§2.1 semantics section, meta row `2`, invariant 8 "scheduling state, no
proof facts"); operator manual rewritten in place — §1 map, §2.1 artifact
table (two artifacts left: shards + DB), §2.2 tools (three tools), §3.1
quickstart re-pinned verbatim against the rebuilt binary (including the
censor-state SQL check and the re-pinned `child_evals` chunk-overshoot
numbers 200021/400021 — the pre-plan20 pins had drifted), §4 rewritten as
censor-state pick-up, §7.1/§7.2 grammars, §7.4 deleted, R4 = the import
recipe, R5 rewritten around preservation, §9 writer-split bullet, §10
fresh-vs-carried state, §11 plan20 history entry; AGENTS.md
harvest/merge bullets updated.

**S6 — standing-layer migration** (production `data/proofdb/`, bare
defaults):

- `proofdb_merge --import-censor data/proofdb/work.json`: **3,615 records
  imported** (the plan predicted 3,548 — the sidecar had grown with the
  plan16/19 session work; both numbers are honest counts of their dates),
  0 preserved (the standing DB was still schema v1). Post-migration
  census: schema_version 2, 3,615 censor rows, 43,092 nodes (43,028
  proven + 64 open), 405 shards, `built_from 70a160a6…`; `proofdb_flip`:
  `flips 0 verified 0`, exit 0 (v2-additive verified on the live layer).
- Idempotence verified on the standing layer: a second identical import
  reported `3615 preserved + 3615 imported -> 3615 rows` and changed
  nothing. A one-job harvest smoke started cleanly against the migrated
  layer (census `records 3615 … — root open`) and its two follow-up
  censor commits landed in the DB (the table now holds 3,617 rows — two
  honest 200k-eval censor records from the smoke; harmless scheduling
  state).
- `data/proofdb/work.json` deleted — the layer is now one file
  (`proofdb.db`) plus the durable shard set.

**S7 — rebuild-gate re-pin**: the fixture rebuild digest moved
`0d929f4c…` → `406752cc93fb5fd8667615b4720fbd39c20a4df5b014dde97bf8311a
22130fb6` (schema-v2 fixture: additive censor table, empty). Re-pinned
in `tests/proofdb.rs` and manual R1/§10 (the pre-plan20 value is kept as
a dated remark). Equality semantics unchanged — the fixture carries no
censor rows, verified.

## Tests added

- Unit (`ledger/tests.rs` rewritten): DB roundtrip + durability (reopen
  the file), missing-table abort, commit-failure abort via a read-only
  connection injector (DB keeps its last committed state), i64-overflow
  abort, legacy JSON parse + defect rejection + missing-file error,
  `upsert_max` (per-field max, stale-import safety, idempotence).
- Integration (`tests/proofdb.rs`): censor roundtrip through `write_db` +
  schema-version row + zero-eval censor retention; v1-DB read = empty;
  row-for-row rebuild preservation (D5); censor commits visible to a
  second session's frontier through `run_walk_session`; import max-rule /
  idempotence / clamp at the `Ledger` primitive level. The walk driver
  tests now pin censor rows instead of ledger bytes (the plan16
  determinism gate's row-for-row analogue).
- CLI-level (`tests/proofdb_harvest_cli.rs` adapted): per-test DB copies
  (the harvest writes censor rows to `--db` now), censor seeding by SQL,
  censor-state assertions read back from the DB; new
  `tests/proofdb_merge_cli.rs`: rebuild preservation and import
  max-rule/idempotence through the real binaries.

## Tools used

Standard cargo toolchain only (`cargo test/clippy/fmt/doc`, `make test`);
python3 for the standing-layer census probes and the legacy-ledger record
count; the release binaries for the §3.1 quickstart replay (in a sandbox
cwd — the defaults are relative paths) and the S6 migration.

## Problems encountered

- `rusqlite::OpenFlags` has no `SQLITE_OPEN_NO_CREATE` — plain
  `SQLITE_OPEN_READ_WRITE` already refuses to create a missing file,
  which is exactly the wanted harvest semantics.
- The quickstart re-run exposed a pre-existing pin drift (chunk-overshoot
  child-evals 200015 → 200021 with the current movegen); re-pinned while
  editing those exact lines.
- **Process slip (self-inflicted, repaired):** verifying harvest-stop
  behavior I ran `rm -f STOP` against the repo root's operator stop file
  (the plan19 finding); restored with `touch STOP` immediately after.
  Lesson: use a scratch stop-file path outside the repo root for smoke
  runs.

## Unresolved parts / missing tests

- The merger census line grammar changed additively (`censor:` line,
  printed only when state is carried); the byte-identity contract covers
  DB bytes, not stdout, and the manual documents the line — but external
  consumers of merge stdout (none known) would see it.
- Concurrency of the harvest's per-censor commits against SQLite's
  default journal is untested under genuine parallel load (the documented
  discipline is "never concurrently"; item 18's incremental fold will
  revisit).
- The `--import-censor` flag has no `--expect`-style digest pin (the
  retired union tool had one); the max rule makes a wrong import
  harmless-by-construction, so the pin was not carried over — noted for
  item 18's audit if parallel import ever needs attribution.
- `tests/proofdb.rs` (~40 KB) keeps growing past the file-size split
  threshold (the report12 finding) — still outstanding.

## Next steps

1. **Item 18 (the pivot's step 2)** — incremental merge fold, deletable
   shards, `proofdb_verify` + backup duty, unmerged-shard staleness gate;
   its plan must settle the manifest-retirement path and decide whether a
   DB→shard-set export is retained.
2. Item 6 (parallel harvesters) is now un-blocked on its censor side:
   per-worker censor merging is plain SQL max-upserts against the
   `censor` table.
3. Kickoff prompt for the recommended next session:

```
Read docs/plans/proofdb/initiative.md (items 17–19) and plan20/report20,
then draft plan18-of-item-18.md: the incremental merge fold (validate a
shard, graft into the standing DB keyed on the shards-table sha256,
idempotent), shard deletion after merge, proofdb_verify (replay-validate
the DB tree read back from the DB), the unmerged-shard staleness gate
replacing built_from, and the VACUUM INTO backup duty — settle the two
open decisions (manifest retirement path; DB→shard export retention) in
the plan session first.
```
