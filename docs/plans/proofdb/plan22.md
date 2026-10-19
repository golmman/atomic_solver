# plan22 — DB as the source of truth: incremental fold, deletable shards, `proofdb_verify`

**Initiative:** `proofdb` · **Executes:** backlog item 18 (added 2026-10-15,
owner direction) · **Size:** M (examples-side merger rewire + two new
modules + one new tool + gate swap across three CLIs + docs patches; **no
`src/` changes**) · **Date drafted:** 2026-10-16 (plan session, docs-only)

> **Owner framing (2026-10-15 pivot, from `initiative.md`):** the shard set
> will not scale to the campaign sizes the owner targets (10⁵–10⁷ shards;
> the per-decisive-job manifest rewrite is O(N²) over a campaign; a
> million-file durable layer fails on filesystem grounds). The **DB becomes
> the single source of truth** — one `proofdb.db` growing to multiple GB —
> and shards are reclassified as ephemeral transport, deletable after merge.
>
> **Item 18's own text:** `proofdb_merge` becomes an incremental fold:
> validate a shard, graft into the *standing* DB in one transaction keyed on
> the `shards`-table sha256 (idempotent, conflicts abort without writes);
> the byte-identical full-rebuild contract (constraint 3) is retired and
> replaced by `proofdb_verify` (replay-validate the whole DB tree read back
> from the DB itself — validation is path replay, it needs no shards) plus a
> documented `VACUUM INTO` offsite-backup duty; the `built_from`
> manifest-digest staleness gate is replaced by unmerged-shard detection.
>
> This plan settles the two decisions item 18 left open: the **manifest
> retirement path** (D1/D2 — per-shard sidecar; shard-format v2 rejected) and
> the **DB→shardset export** (D9 — not built, rationale recorded).

## 0. State this plan starts from (verified by code read this session)

- **Durable layer today:** `data/proofdb/shards/*.bin` (validated proof
  trees, `docs/spec/proof_tree_dump.md` v1 format) + `manifest.json`
  (required per-entry fields `tag/file/fen/moves/outcome/validate`,
  written deterministically and atomically by `manifest.rs::write_manifest`,
  re-synced per decisive job by plan17's `Session::sync_manifest`, rewritten
  at batch end by `Session::rewrite_manifest`). The fixture shard set is
  `docs/plans/proofdb/shards/` (262 bins + manifest) — a validation fixture,
  never read by the production defaults.
- **`proofdb_merge` (`examples/proofdb_merge.rs` + `proofdb/merge*`)** is a
  **full rebuild**: read the whole manifest → load/validate each shard
  (`load_shard`: validate=="ok" gate, binary parse, `validate_proof_tree`,
  outcome cross-check, startpos replay + hash fidelity) → in-memory
  `PathTree` (`graft_path` open ancestors + `overlay_subtree` union) →
  `finalize()` (bottom-up depth fixpoint + lexicographic row-id assignment,
  `merge/depth.rs`) → per-graft skeleton re-validation (`reassemble_skeleton`
  + `validate_proof_tree`) → `write_db` **replaces the DB file**
  (`schema.rs::write_db`), re-inserting preserved/imported `censor` rows
  (plan20 D5/D6). `meta.built_from` = manifest sha256.
- **The `built_from` gate:** `proofdb/db.rs::load_db_rows(db, manifest_sha)`
  aborts unless `meta.built_from == manifest_sha` — used by
  `proofdb_harvest` and `proofdb_flip` (both read the manifest only to get
  the digest + seed `Session`'s tag/path collision sets).
- **`proofdb_harvest`:** censor connection RW (the only DB writes: `censor`
  rows, plan20 D3 carve-out); `Session` holds the manifest entries as
  collision state; a decisive job → `shard_export::export_validated_shard` →
  write `<tag>.bin` → push `ShardEntry` → per-job atomic manifest sync
  (plan17). Crash window today: at most the in-flight job's bin is an
  unreferenced orphan.
- **Determinism gate today:** the `tests/proofdb.rs::
  standing_layer_rebuilds_byte_identical` regression merges the committed
  fixture manifest into a temp DB and pins the canonical-dump digest
  `406752cc…` (55,703 nodes). This test's *contract* dies with the rebuild.
- **Spec:** `docs/spec/global_proof_store.md` (schema v2) still says §2
  `built_from` = manifest sha256, §5 the manifest contract, §6 "the durable
  truth is the shard set plus its manifest". Constraint 3 (`initiative.md`)
  says the DB is a derived rebuildable cache. Both are amended by this plan
  (§5 below); item 19 owns the standalone v2 rewrite.
- **Row-id semantics:** `finalize()` assigns ids in lexicographic path order
  (spec invariant 7); the root is id 0. The `shards` table stores per-shard
  provenance (tag, file, sha256, root_fen, path, outcome, depth_bound,
  n_nodes) — written by merge only.
- **Merge/harvest concurrency:** sequential operator discipline (merge after
  the batch); SQLite's write lock is the mechanical backstop. Unchanged.

## 1. Design rulings (proposed by this plan session; owner may veto any)

- **D1 — Manifest retired; per-shard sidecar.** Each shard becomes
  self-describing: at produce time the harvest writes `<tag>.bin` then
  `<tag>.meta.json` (atomic tmp+rename, `manifest.rs`'s pattern; tmp name
  `<tag>.meta.json.tmp`). Fields: `tag`, `file`, `fen`, `moves` (UCI array),
  `outcome`, `sha256` (the bin digest — a corruption cross-check; the merge
  refuses a bin/sidecar sha mismatch). No `validate` field: sidecars exist
  only for validator-clean shards (the produce pipeline guarantees it).
  Write order is the commit marker: **a sidecar present ⇒ its bin is
  complete; a bin without a sidecar is the plan17 orphan** (skip-with-note
  everywhere; the retry overwrites determinically — tag = hash of path);
  **a sidecar without a bin is a defect** (abort with a message). The tag
  and file name remain derivable (`h_<sha16(path)>.bin`) but are stored in
  the sidecar for self-containment.
  - *Rejected: shard-format v2 with a graft-path header.* That reopens the
    `proof` initiative's "dump format v1 unchanged; no reopening" constraint
    and touches the product solver's `src/proof_tree/binary.rs` — violating
    design constraint 5 (examples-side tooling only).
  - *Rejected: the harvest writes the `shards` row at produce time.* It
    widens the plan20 writer carve-out (two DB writers again) and creates a
    bin+row state whose fold has not happened — the unmerged state would
    then be invisible to tag-keyed detection. The sidecar keeps the DB
    single-writer and makes "unmerged" a pure dir↔table comparison (D6).
- **D2 — `--manifest` becomes a legacy metadata source (migration +
  fixtures), never a requirement.** At merge time, if the manifest file
  exists, its entries serve as metadata for unmerged tags that lack a
  sidecar **and** the merge backfills their sidecars; sidecar metadata wins
  when both exist. A manifest entry whose bin is missing stays a hard abort
  (as today). With no manifest file, the merge is pure sidecar mode. The
  standing-layer migration is therefore just: run bare `proofdb_merge`
  (backfills 262 sidecars, folds everything, verifies), then delete the
  folded bins (D8 recipe). `proofdb_harvest` **drops `--manifest`
  entirely** — the flag refuses with a pointer to `proofdb_merge` (the
  `--ledger` precedent).
- **D3 — `proofdb_merge` becomes the incremental fold: one transaction per
  invocation.** Open the standing DB read-write (missing file → fresh
  schema, everything folds; a schema-v1 DB aborts with the plan20 rebuild
  hint). **Load** the merged tree from the `nodes` rows into `PathTree` via
  a new replay-validated loader (`merge/fold.rs`): a DFS from the root with
  a lib `Position` (do/undo), re-deriving every stored move with
  `uci_to_move` (None ⇒ abort), cross-checking `ply`, parent-chain
  integrity, and `move_code == move_to_bits(move)` — so **the fold is
  self-validating on load**: a structurally broken standing DB aborts
  before any write. Restores `outcome`, `provenance` (comma-split), and
  `exact_claims` (non-terminal rows with `depth_status='exact'` claim their
  stored bound). Then, for each **unmerged** tag (shard-dir bins sorted by
  tag whose tag is absent from the `shards` table): the current `load_shard`
  pipeline verbatim (validate gate, parse, replay-validate, sidecar
  cross-checks, replay + hash fidelity) → `graft_path` + `overlay_subtree`
  (the existing union/refinement/contradiction machinery — agreement,
  refinement, and annotation semantics are inherited bit-for-bit) →
  `finalize()`. **One transaction** writes the delta: new node rows, UPDATEs
  for existing rows whose recomputed `(outcome, depth_bound, depth_status,
  provenance)` differ (upgrades, provenance unions, refinements — this
  re-derives "stored values == finalized values" on every fold), one
  `shards` insert per folded tag, meta refresh (`node_count`, `built_from`
  = D5's fold digest, `generator`). **Idempotent by tag skip:** a tag
  already in `shards` is skipped after cross-checking its stored sha256
  against the bin on disk — a mismatch is a hard abort (same tag, different
  bytes ⇒ corruption or a re-exported different tree: never patched).
  **Conflicts abort without writes:** any defect rolls the whole invocation
  back (batch atomicity — the item text's "one transaction per shard/batch"
  read as batch; a rolled-back fold re-runs from scratch idempotently).
  Censor rows are untouched in place (plan20 D5 becomes vacuous);
  `--import-censor` keeps working, upserting in place under the max rule.
  *Recorded limitation:* the load holds the whole merged tree in RAM
  (O(N) ≈ ~200 B/node ⇒ ~2 GB at 10⁷ nodes) — two orders away from today's
  55,703-node layer; a streaming row-surgery fold is the future step if
  multi-GB layers materialize (noted, not built).
- **D4 — Row ids are preserved and appended (no renumbering).** Existing
  rows keep their ids forever; new rows get `max(id)+1…` assigned in
  lexicographic path order within the fold (parents sort before children;
  the root keeps id 0). **Fresh-init carve-out:** when the fold starts from
  an empty DB, ids are assigned over the *whole* tree in global
  lexicographic path order (`finalize`'s existing order) — a fresh fold is
  therefore id-identical to the retired full rebuild (the migration-fold
  equivalence R1 relies on); incremental folds use the append scheme. Spec invariant 7's "ids in lexicographic path order" is
  amended to "ids are stable surrogate keys; identity is the path" in this
  plan's spec patch (item 19 finalizes). **The byte-identical-rebuild
  contract (constraint 3) is retired deliberately** — its replacement is
  `proofdb_verify` + the backup duty (D7/D8). The *deterministic fold*
  survives: the same standing DB + the same shard set ⇒ identical bytes
  (kept as a test); two *different* fold sequences may differ in ids only —
  documented.
  - *Rejected: renumber-on-fold.* It would keep invariant 7 verbatim, but
    rewrites every row (and every parent link) per invocation — O(N) writes
    per merge forever; the append scheme is strictly cheaper and the spec
    change was already owed to the pivot.
- **D5 — `built_from` redefined as the fold digest:** sha256 over the
  sorted `tag sha256` lines of the post-fold `shards` table. Same meta row,
  never NULL (consumers don't break), deterministic (the new test pin),
  honest (a provenance-set digest, not a manifest digest).
- **D6 — Unmerged-shard detection replaces the `built_from` gate
  (harvest + flip).** New check (in `db.rs`): scan the shard dir —
  (i) a **sidecar** whose tag is absent from `shards` ⇒ refuse, exit 1,
  "unmerged shard `<tag>` — run proofdb_merge first"; (ii) a bin without a
  sidecar ⇒ an in-flight orphan, **ignored** (plan17's loss window: a
  crashed session must not wedge the next harvest; the retry overwrites);
  (iii) a sidecar without a bin ⇒ refuse as a durable-layer defect. The
  manifest-digest staleness story ("DB older than the manifest") is
  subsumed: any shard collected after the last fold is a sidecar without a
  row. `proofdb_flip` gains `--shard-dir` (default `DEFAULT_SHARD_DIR`) for
  the check. The loader's `root_fen` check stays. `load_db_rows` drops its
  manifest-digest parameter (and the stale-DB `built_from` guard with its
  length-safe rendering — plan12's G1 regression test retires with it; the
  panic class it guarded is gone since nothing renders a foreign digest
  anymore).
- **D7 — `proofdb_verify`: new example binary; streaming, O(depth) RAM.**
  Reads the DB alone (no shards), `--db` (default), `--max-report` (defect
  cap), and `--backup <path>` (= `VACUUM INTO` — the offsite-backup duty,
  one flag so the recipe is executable). One DFS from the root with a lib
  `Position` (do/undo; one indexed children query per node): per node,
  replay legality (`uci_to_move`, ply = parent+1, move_code agreement),
  then the structural invariants — proven **AND**: the child set equals the
  legal-reply set (movegen) and every child is `win`; proven **OR**: ≥1
  proven-loss child and `depth_bound = min(loss-child bound) + 1`; proven
  **terminal**: `depth_bound = 0`, no children, and **statically terminal**
  in the replayed position (lib terminal classification); open nodes: no
  structural requirement (annotation/graft territory); every row:
  non-empty `provenance`; plus `meta.node_count == COUNT(nodes)`. Depth
  monotonicity is checked locally on the retreat (the finalize fixpoint
  rule, per node). Output: `verify: ok — nodes N (proven P, open O)` or
  `verify: FAILED k` + up to k defect lines, exit 2 on defects.
  *Recorded limitation:* non-terminal `exact` minimality is not re-derivable
  from the DB alone — trusted as a search fact (the same class of trust the
  shard validator already extends to search-computed bounds); the spec's
  invariant-2 "exactly one proving child" wording is looser than the merged
  reality (alternative proven-loss children are annotations) — verify
  enforces the accurate ≥1 form; the wording fix lands in item 19's spec
  v2.
- **D8 — Deletion is operator-driven; no `--prune` flag.** After a
  successful fold, folded `*.bin` + `*.meta.json` pairs are deletable
  (every gate tolerates their absence: the fold skips tags with rows; the
  harvest/flip gates key on unmerged sidecars). Deletion itself stays a
  manual recipe — **fold → verify → `VACUUM INTO` backup → delete** —
  because it is irreversible (append-only truth: a buggy fold can no longer
  be excised by deleting a shard and rebuilding — the accepted loss in the
  item text; pre-backup deletion must never be one flag away). Patched into
  the manual now; formalized by item 19's rewrite.
- **D9 — No DB→shardset export.** Portability = the single DB file (a copy
  is the export); audit = `proofdb_verify` replays from the DB alone; and an
  export would be per-graft skeletons re-derived from *refined* merged facts
  (a different artifact class, not the original shards — which are
  unrecoverable after deletion either way). The building block exists if a
  future need materializes (`reassemble_skeleton` at every graft root —
  already run at fold time). Recorded as a non-goal.
- **D10 — Docs scope: leave nothing actively wrong; the canonical rewrite
  is item 19.** This plan patches: spec §2 (`built_from` semantics), §5
  (manifest → legacy/migration source + a sidecar subsection), §6 (DB =
  truth; shards ephemeral; verify + backup), invariant 7 wording;
  `docs/proofdb_pipeline.md` §2.1 (sidecar artifact), §3 quickstart re-pins,
  §7.1 (merge = fold; new census grammar), §7.2/§3.2 (harvest: no manifest),
  §8.1 recipes (R1 rebuild → R1 fold+verify+backup; the deletion recipe),
  §10 troubleshooting; `AGENTS.md` proofdb bullet (fold, verify, sidecars,
  deletable shards); `initiative.md` constraint-3 amendment + item-18 row +
  history entry (closure tasks). No plan-vocabulary enters the spec.

## 2. Implementation map

- **`examples/proofdb/sidecar.rs` (new, < 5 KB):** `SidecarEntry`
  (`tag/file/fen/moves/outcome/sha256`), `write_sidecar` (tmp+rename),
  `read_sidecar`, `sidecar_path(tag)`; sha cross-check helper. Unit tests
  (determinism, tmp residue, round-trip).
- **`examples/proofdb/merge/fold.rs` (new, < 10 KB):**
  `load_standing_tree(conn) -> Result<(PathTree, Vec<ShardRow>, usize
  max_row_id), String>` (the replay-validated DFS loader),
  `write_fold_delta(conn, &tree, rows…, digest)` (the single-transaction
  delta writer: ensure-schema for fresh DBs, node upsert/updates, shard
  inserts, meta refresh), `fold_digest(shard_rows)`, and the id-assignment
  rules (D4: global lex order for fresh-init, append for incremental).
- **`examples/proofdb/schema.rs`:** `read_shard_rows(conn)`; keep
  `write_db`/`canonical_dump` as the fixture/legacy fresh-init helper
  (documented as such); `built_from` parameter semantics = fold digest.
- **`examples/proofdb/db.rs`:** `load_db_rows(db)` without the digest gate
  + `read_shard_rows` + `check_unmerged_shards(shard_dir, shard_rows)` (D6).
  The length-safe `short()` helper retires with the guard.
- **`examples/proofdb_merge.rs` (CLI):** fold mode (D3) + `--manifest`
  legacy source/backfill (D2) + `--import-censor` in place + new census
  lines; `load_shard` reused verbatim; skeleton re-validation stays
  (per folded graft, as today). Preserved census counters
  (overlay/dedupe/ancestor/upgrades) + new `folded/already/orphans`
  counters.
- **`examples/proofdb/session.rs`:** drop `manifest_path`/`entries`/
  `sync_manifest`/`rewrite_manifest` (plan17 machinery); `produce_shard`
  writes bin + sidecar (sidecar failure aborts the session — the plan17
  analog); `Session::new` seeds collision sets from the DB shard rows.
  Job-record grammar unchanged.
- **`examples/proofdb/harvest_args.rs` / `examples/proofdb_harvest.rs`:**
  `--manifest` refusal (D2); main reads shard rows, runs the D6 gate, drops
  the manifest read/rewrite and the `manifest:` stdout line; the banner
  prints `fold digest <16>` instead of `built_from <16>`.
- **`examples/proofdb_flip.rs`:** drop the manifest; `--shard-dir` flag +
  D6 gate.
- **`examples/proofdb_verify.rs` (new CLI, < 10 KB):** D7.
- **`examples/proofdb/manifest.rs`:** keep `read_manifest` + `Manifest`
  (legacy source); **delete `write_manifest`** and its unit tests (no
  writer remains).
- **`examples/proofdb/mod.rs`:** export `sidecar`; defaults unchanged
  (`DEFAULT_MANIFEST` stays as merge's legacy default path).

## 3. Tasks (execution order; each step keeps the tree compiling)

1. `sidecar.rs` + unit tests.
2. `merge/fold.rs`: loader + delta writer + fold digest (+ id-append rule).
3. Rewire `proofdb_merge` (fold mode, legacy manifest source + backfill,
   in-place `--import-censor`, census grammar).
4. Rewire harvest (`db.rs` gate swap, `Session` sidecar production, args,
   banner/summary lines).
5. Rewire `proofdb_flip` (+ `--shard-dir`).
6. `proofdb_verify` (DFS validator + `--backup`).
7. Tests (§4): replace the retired pins, add R1–R10; delete
   `standing_layer_rebuilds_byte_identical`, the stale-DB guard test, and
   the plan17 manifest-sync tests (their contracts die with the manifest);
   update the two CLI test fixtures to the sidecar world.
8. Docs patch (D10) + quickstart re-pins against this container.
9. Full gate: `make test`, `cargo clippy`, `cargo fmt`, `cargo doc`; no
   `src/` changes anywhere.
10. `report22.md` (final task) + `initiative.md` updates (constraint 3
    amendment, item-18 row, history entry). `docs/plans/README.md` row is
    already post-pivot; touch only if wording is now stale.

## 4. Test plan (pre-registered pins; exact bytes re-pinned at execution)

- **R1 — Migration equivalence (the strongest check):** a fresh fold of the
  committed fixture shard set (manifest present ⇒ backfill) must produce a
  `nodes` table **identical to today's standing layer** — same ids (the D4
  fresh-init carve-out assigns global lex order, exactly the old rebuild's
  assignment), same depths, statuses, provenance — and the same censor
  rows; the canonical dumps agree except the `built_from` line (fold digest
  ≠ `406752cc…`). Pin the new fold digest + node count. Then: `verify: ok`;
  re-fold ⇒ `folded 0`, dump byte-identical.
- **R2 — Deterministic fold:** two fresh folds from the same shard set ⇒
  byte-identical dumps (the surviving determinism gate).
- **R3 — Delta fold:** fold batch 1, add a new sidecar shard sharing a
  graft path (dedupe + agreement + refine exercised), fold batch 2 ⇒
  existing ids unchanged, new ids > max, node arithmetic line consistent.
- **R4 — Conflict abort without writes:** a contradicting shard ⇒ exit 2,
  DB dump byte-identical to pre-invocation.
- **R5 — Crash windows:** bin-without-sidecar ⇒ merge note + skip, harvest
  gate passes; sidecar-without-bin ⇒ merge refuses; bin/sidecar sha
  mismatch ⇒ abort.
- **R6 — Unmerged gate:** sidecar-without-row ⇒ harvest exit 1 + flip exit
  1 (message names the tag); after fold ⇒ pass.
- **R7 — Censor discipline:** seeded censor rows byte-equal after a fold;
  `--import-censor` max-rule idempotent in place.
- **R8 — `proofdb_verify`:** green on the fixture DB; red on corrupted
  copies (missing reply child / flipped outcome / illegal stored move /
  ply break / `node_count` mismatch), each with exit 2; `--backup` writes a
  readable copy.
- **R9 — Session durability:** decisive job ⇒ bin+sidecar pair on disk;
  sidecar-write failure ⇒ session abort, previous durable state intact, the
  bin the one orphan; censored jobs touch no durable file.
- **R10 — CLI fixtures:** `proofdb_merge_cli` (censor preservation via
  fold; import) and `proofdb_harvest_cli` (scope/exhaustion shapes) rebuilt
  on the sidecar fixture; harvest runs with no `--manifest`.

## 5. Known risks

- The fold's in-RAM whole-tree load is the deliberate correctness-first
  choice (overlay semantics inherited bit-for-bit); the streaming fold is
  the pre-registered future step (D3 limitation) — do not build it here.
- Removing the manifest gate widens what a *stale restored backup* can hide
  (a DB older than its shard dir with the shards already deleted is
  invisible to every gate) — documented in troubleshooting; the backup duty
  is the mitigation.
- `tests/proofdb.rs` keeps growing (report12 finding): new merger tests go
  into the existing target only if they fit; larger additions prefer a new
  `tests/proofdb_fold.rs` target on the same `#[path]` pattern.

## 6. Out of scope (this plan)

- Spec v2 standalone rewrite, manual rewrite, standing-layer migration
  run, AGENTS/website re-agreement — item 19 (runs immediately after).
- `--prune` automation (D8: operator recipe only), DB→shardset export
  (D9), streaming fold (D3 limitation), closure promotion (item 12),
  parallel harvesters (item 6).
