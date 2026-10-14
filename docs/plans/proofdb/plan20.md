# plan20 — censor state into the DB: one-artifact frontier (retire `work.json` and `proofdb_ledger_union`)

**Initiative:** `proofdb` · **Executes:** backlog item 17 (added with this
plan) · **Size:** S–M (examples-side schema + persistence swap + merger
preservation + one tool retirement + docs; **no `src/` changes**) ·
**Date drafted:** 2026-10-15 (plan session, docs-only)

> **Owner framing (2026-10-15 design dialogue), verbatim:** *"my reasoning
> is that it will get hard to keep track of potentially tens of thousands
> maybe even millions of individual shards. I'd rather have a single file
> (the `proofdb.db`) which can grow to multiple gigabytes but is the single
> source of truth. I also doubt we still need the `work.json`. It was
> needed for laddered harvesting (e.g. `and-close` policy) but we should be
> able to get all necessary info directly from the `proofdb.db` for the
> only remaining `frontier-walk` policy. I think the shards are still fine
> so we can cleanly decouple the harvest from the merge, but it should be
> ok to delete them after the merge."*
>
> **Design-dialogue outcome (this session):** the single-file goal is
> adopted as a three-step program — items 17/18/19. This plan executes
> step 1 only. Two corrections from the dialogue shape it: (i) the ledger
> is **not** laddered-harvest legacy — it holds censor history
> (`passes_failed`/`work_done`), the *facts of absence* of censored draw
> jobs, which are not representable in the tree model and therefore not
> derivable from the tree tables; it must be **relocated into the DB**, not
> dropped. (ii) "delete shards after merge" is **not** part of this plan:
> it requires the incremental-fold merger and the `proofdb_verify` audit
> (item 18); this plan leaves the durable layer (shards + manifest +
> `built_from` gate) byte-for-byte unchanged.

## 0. State this plan starts from

- Plan19 shipped `--moves` scoped harvest; plan18 made `frontier-walk` the
  only policy. The walk's only ledger consumer surface (verified by code
  read this session): `Ledger::load` at `walk/driver.rs:278`,
  `on_censored(ledger, path, key, evals)` at `driver.rs:119`,
  `ledger.contains/get/len/iter` in `walk.rs` (`WalkFrontier::new`, `pick`,
  `is_handled`, census) and `driver.rs:240`.
- **The ledger is censor-history only.** `Ledger::insert_fresh` has no
  production call site (tests only); records enter exclusively via
  `on_censored` → `bump` (which creates the record if absent). So the
  `censor` table below is a faithful relocation: today's standing
  `data/proofdb/work.json` holds 3,548 records, all censor-derived.
- Durability precedent: `on_censored` saves the ledger atomically after
  **every censor** (plan17's per-job discipline for the manifest has its
  analogue here); a save failure aborts the batch with the last saved
  state consistent.
- Single-writer contract today: `proofdb_merge` writes the DB; harvest
  never writes it (`load_db_rows` is read-only). `built_from` =
  manifest-sha256 gate in harvest/flip — **unchanged by this plan**.
- Merger rebuild semantics: `schema.rs` creates a fresh DB (CREATE TABLE +
  INSERT); a bare `proofdb_merge` discards the previous file entirely —
  this is the hazard that makes censor preservation (§1 D5) mandatory.
- Spec: `docs/spec/global_proof_store.md`, schema v1, `meta.schema_version`
  = 1. The website consumes `nodes` (+ `meta`/`shards`); `censor` is
  additive and ignorable by consumers.

## 1. Design rulings (proposed by this plan session; owner may veto any)

- **D1 — Relocate, don't drop.** New `censor` table: `CREATE TABLE censor
  (path TEXT PRIMARY KEY, work_done INTEGER NOT NULL, passes_failed
  INTEGER NOT NULL)`. `path` is the absolute space-joined UCI path key —
  byte-for-byte today's ledger key. Schema bumps to v2 (`schema_version`
  = 2; additive — a v1 reader sees no change in `nodes`/`shards`/`meta`
  semantics).
- **D2 — Swap the persistence backend, keep the type.** The in-memory
  `Ledger` BTreeMap struct, its `bump`/max-rule semantics, and all
  selection code (`WalkFrontier`, driver) stay untouched. Only the
  backend changes: `Ledger::load(path)` → load from the DB (`SELECT`
  into the map); `on_censored(ledger, path, …)` → bump + immediate
  committed `INSERT OR REPLACE` on the DB connection. The `--ledger` flag
  and `DEFAULT_LEDGER` are removed.
- **D3 — Narrow writer carve-out.** Harvest gains DB write access for
  **`censor` rows only**; merge remains sole writer of
  `nodes`/`shards`/`meta`. The operator discipline (harvest and merge
  never run concurrently) is unchanged and documented; SQLite's locking
  is the mechanical backstop. Each bump is one committed transaction
  (default journal mode; censor frequency is far below search frequency,
  fsync cost is negligible).
- **D4 — Abort semantics preserved.** A failed censor commit aborts the
  batch exactly like today's ledger-save failure: the DB's last committed
  state is consistent (censors before the failure are committed; the
  in-flight job's censor is lost, matching the plan17 loss window).
- **D5 — Merge preserves `censor` across its full rebuild.** Merge reads
  the existing DB's `censor` table (if the file exists) before the
  rebuild and re-inserts it after. Without this, every merge-after-batch
  and recipe R5 would wipe selection state. Merge never *modifies* censor
  rows; only harvest does. (Item 18's incremental fold subsumes this.)
- **D6 — One-shot migration via merge.** `proofdb_merge --import-censor
  <work.json>` parses a legacy ledger and merges it into the table under
  the `proofdb_ledger_union` max rule (per key: `work_done` and
  `passes_failed` each keep the max of existing vs imported; new keys
  inserted). Exported by this plan for the standing layer; afterwards a
  plain no-op for fresh layers. (Alternative rejected: auto-import on
  harvest start — magic state, harder to test.)
- **D7 — `proofdb_ledger_union` is retired** (deleted binary + module +
  manual §R4 + AGENTS.md row). Its max-rule survives in D6's import and
  (later, item 6) becomes plain SQL for parallel harvesters.
- **D8 — stderr grammar, minimal change.** The harvest banner drops
  `— policy … over ledger <path>`; the `records` count stays (census).
  Job JSON lines are unchanged. The censor-pick-up semantics section of
  the manual moves to the table.

## 2. Implementation map

- `examples/proofdb/schema.rs` — add the `censor` table + `schema_version`
  = 2; censor row insert/read helpers used by both tools.
- `examples/proofdb/ledger.rs` — keep `LedgerEntry`/`Ledger` (map + bump
  semantics); replace `load`/`save` with DB-backed load and a
  commit-on-bump hook; `on_censored` re-signature (DB handle instead of
  path). JSON codec deleted. `ledger/tests.rs` split: pure-logic tests
  stay; persistence tests move to the DB layer.
- `examples/proofdb/harvest_args.rs` — drop `--ledger`/`DEFAULT_LEDGER`;
  usage text; unknown-flag error path already exists.
- `examples/proofdb/harvest.rs` / `walk/driver.rs` — open the DB RW;
  `Ledger::load` from the DB; banner line update.
- `examples/proofdb/merge*.rs` — D5 preservation + D6 `--import-censor`.
- Delete: `examples/proofdb/ledger_union.rs`, its CLI registration.
- Docs: `docs/spec/global_proof_store.md` (additive v2 amendment),
  `docs/proofdb_pipeline.md` (§2.1 artifact table, §4 pick-up, §7
  grammars, R4 retired, R5 rewritten around D5), `AGENTS.md`
  (`proofdb_ledger_union` row removed; harvest/merge bullets updated).

## 3. Edge cases (normative)

- **Rebuild with censor state:** bare `proofdb_merge` on a DB carrying
  censor rows must preserve them exactly (row-for-row, including keys
  never proven). Rebuild on a missing DB → empty censor (fresh layer).
- **Import idempotence:** importing the same `work.json` twice changes
  nothing (max-rule against already-imported rows).
- **Import after growth:** importing a stale ledger into a DB with newer
  censor rows must never decrease `work_done`/`passes_failed` (max-rule;
  regression-tested).
- **Zero-eval censors** (on-path repetition, plan16's complete-mark rule):
  record created with `work_done = 0` — same bump semantics, retained.
- **`--moves` scoped sessions:** censor keys stay absolute (plan19's
  `rel_of`/`join_scope` operate on the in-memory map — untouched).
- **Stale-DB guard:** the `built_from` digest gate is orthogonal to the
  censor table and continues to abort harvest/flip on manifest mismatch.

## 4. Tasks

1. S1 schema v2: `censor` table, `schema_version` = 2, row helpers
   (unit: round-trip, max-rule upsert, schema-version row).
2. S2 ledger backend swap (D2–D4) + `--ledger` removal (unit: bump
   commit-on-failure abort; integration: censor visible to a second
   session's frontier).
3. S3 merge: censor preservation across rebuild (D5) +
   `--import-censor` (D6) (unit: preservation, idempotence, stale-import
   max-rule; integration: rebuild-with-censor, R5-shaped delete+re-merge).
4. S4 retire `proofdb_ledger_union` (binary, module, tests, AGENTS.md).
5. S5 docs: spec additive v2 amendment; manual §2.1/§4/§7/R4/R5 +
   quickstart; AGENTS.md.
6. S6 standing-layer migration: run `proofdb_merge --import-censor
   data/proofdb/work.json` on the production layer; record the imported
   record count (expect 3,548) and a census in the report; delete
   `data/proofdb/work.json` afterwards.
7. S7 rebuild-gate re-pin: re-run
   `tests/proofdb.rs::standing_layer_rebuilds_byte_identical`; the
   fixture DB bytes change (new table) — re-pin the `0d929f4c…` digest
   wherever the manual/report record it; the fixture carries no censor
   rows, so the test's equality semantics need no relaxation (verify).
8. S8 gate: `cargo fmt`, `cargo clippy`, `cargo doc`, `make test`.
9. **Final task:** write `report20.md` (tools used, problems, unresolved
   parts, missing tests, next steps) and update `initiative.md` (item 17
   added/closed, the log entry).

## 5. Risks

- **Schema v2 churn:** the fixture DB and every committed digest pin
  change once; the re-pin is mechanical but touches the manual in several
  places — S7 enumerates them.
- **The carve-out (D3) is a real contract change:** documented in the
  spec amendment and the manual's single-writer paragraph; the item 18
  plan will further rewrite these paragraphs — accept interim wording.
- **Legacy `work.json` copies in committed measurements** (`measurements/
  plan10/ledger_union.json` etc.) are historical records — untouched;
  `ledger_union` deletion does not invalidate committed digest pins of
  *ledgers* (only its tooling disappears).
- **u64 → SQLite INTEGER width:** `work_done` values are child-eval
  counts (≤ ~10¹⁰ observed); `i64` saturating-clamp on import, assert on
  bump — regression-tested at the boundary.
