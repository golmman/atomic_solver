# report22 — DB as the source of truth: incremental fold, deletable shards, `proofdb_verify`

**Plan:** `plan22.md` (executes backlog item 18) · **Date:** 2026-10-16
**Gate:** `make test` green (36 test targets, 526 tests, 0 failures) ·
`cargo fmt` / `clippy --all-targets` / `cargo doc` clean. No `src/`
changes anywhere (design constraint 5 holds).

**Session note:** the original execution session crashed partway through
task 7 (tests). This session recovered the state (implementation tasks
1–6 were complete and the release build was clean), finished the test
tier, the docs patch, and the report.

## 1. What shipped

Exactly the D1–D10 rulings of plan22:

- **D1 — sidecar durable layer** (`examples/proofdb/sidecar.rs`, new):
  `SidecarEntry` (`tag`/`file`/`fen`/`moves`/`outcome`/`sha256`),
  deterministic pretty serialization, `write_sidecar` (atomic tmp +
  rename; a write failure aborts the session with the bin as the one
  orphan), `read_sidecar` (strict field checks), `scan_shard_dir` (bins
  vs. sidecars; `.tmp` residue and unrelated files ignored). The session
  (`session.rs`) writes bin first, sidecar second — the sidecar is the
  commit marker; the sidecar's `sha256` is set from the serialized bin.
- **D2 — manifest retired to a legacy source.** `manifest.rs` keeps only
  `read_manifest`; the writer and its tests are deleted.
  `proofdb_harvest --manifest` refuses with a migration pointer (the
  `--ledger` precedent). At merge time a present manifest backfills
  sidecars for unmerged tags lacking one; sidecar metadata wins.
- **D3 — the incremental fold** (`examples/proofdb/merge/fold.rs`, new):
  `load_standing_tree` (replay-validated DFS loader over the `nodes`
  rows — ply, parent-chain, and `move_code` cross-checks make the fold
  self-validating on load), `load_shard` (the old merge pipeline
  verbatim + sidecar sha cross-check), graft/overlay (agreement,
  refinement, contradiction semantics inherited bit-for-bit),
  `finalize`, per-graft skeleton re-validation, and `write_fold_delta`
  (one transaction: appended node rows, compare-and-update UPDATEs for
  refinements/provenance unions, one `shards` insert per folded tag,
  meta refresh). Idempotent by tag skip with a standing-sha256
  cross-check (same tag, different bytes → hard abort). Conflicts roll
  back the whole invocation. `--import-censor` works in place under the
  max rule inside the same transaction. Recorded limitation: the load
  holds the whole merged tree in RAM (O(N) ≈ ~200 B/node) — the
  streaming row-surgery fold is the pre-registered future step.
- **D4 — row ids preserved and appended.** `RowIds::fold` appends new
  ids past `max(id)` in lex path order; a fresh-init fold assigns ids
  globally in lex path order — id- and value-identical to the retired
  full rebuild (pinned by R1). The byte-identical-rebuild contract is
  retired deliberately (constraint 3 amended).
- **D5 — `built_from` = the fold digest:** sha256 over the sorted
  `tag sha256` lines of the post-fold `shards` table. Fixture pin:
  `14fd07530996eb9eddaa2b5d3e112d51d57339a892f7f8d0bb166205180a04e2`
  (55,703 nodes, 55,628 proven — the standing-layer census pins carry
  over).
- **D6 — unmerged-shard gate** (`db.rs::check_unmerged_shards`):
  sidecar-without-row → refuse ("run proofdb_merge first", names the
  tag); sidecar-without-bin → refuse as a transport defect;
  bin-without-sidecar → the ignored orphan. Wired into
  `proofdb_harvest` (exit 1) and `proofdb_flip` (new `--shard-dir`);
  `load_db_rows` drops the manifest-digest parameter and the stale-DB
  guard (plan12's G1 length-safety regression retires with the panic
  class it guarded).
- **D7 — `proofdb_verify`** (`examples/proofdb/verify.rs` +
  `examples/proofdb_verify.rs`, new): streaming O(depth) DFS with a lib
  `Position`; replay legality, AND/OR structure (verify enforces the
  accurate ≥ 1 proven-loss-child form — the spec's invariant-2 wording
  is fixed in this plan's spec patch), terminal claims, depth
  monotonicity on the retreat, provenance non-emptiness, reachability,
  meta census (`check_meta` lives in the library so tests exercise it).
  `--backup` = `VACUUM INTO` offsite copy on a green verdict; exit 2 on
  defects.
- **D8 — deletion is the operator recipe** (manual §8.1 R6): fold →
  verify → `VACUUM INTO` backup → delete; no `--prune` flag.
- **D9 — no DB→shardset export** (rationale in the plan; non-goal).
- **D10 — docs patch** (nothing actively wrong left): spec §2.2 (fold
  digest), §5 restructured (5.1 sidecar contract, 5.2 legacy manifest),
  §6 rewritten (source of truth, integrity instrument, fold discipline
  §6.1), invariant 2 (≥ 1 proving child), invariant 4 (min over
  proven-loss children), invariant 7 (stable surrogate keys), invariant
  8 (fold leaves censor rows); manual §1–§11 patched (pipeline map,
  artifacts table, tool catalog + new §7.4, quickstart re-pinned from
  live runs on this container, CLI grammars, recipes R0–R6, soundness
  contract, troubleshooting, plan22 history); `AGENTS.md` proofdb
  bullets; `initiative.md` constraint-3 amendment + item-18 row +
  history entry; `docs/plans/README.md` row updated.

## 2. Re-pinned values (this container)

- Bootstrap fold: `fold digest e3b0c44298fc1c14` (the empty-set digest);
  smoke harvest echoes `fold digest e3b0c44298fc1c14`.
- Fixture fold (262 shards, manifest backfill): census as in manual
  §7.1; `backfill: 262 sidecars written`; `verify: ok — nodes 55703
  (proven 55628, open 75)`; re-fold → `folded 0 (262 already merged)`.
- After deleting the folded bins+sidecars: the fold reports
  `folded 0 (0 already merged)` (skip by tag) and the harvest gate
  passes on the empty dir; a sidecar-leftover dir aborts
  (`sidecar without bin — durable-layer defect`) — all verified live.
- Unmerged gate live: `proofdb_flip: unmerged shard h_2fd82f9a8230a668
  — run proofdb_merge first` (exit 1), harvest same; after the fold the
  gate passes and `verify: ok — nodes 336`.

## 3. Test tier (task 7)

- New target `tests/proofdb_fold.rs` (R1–R8 + a seed-derivation test):
  R1 migration equivalence (fresh fold ≡ retired rebuild over the
  committed 262-shard fixture, dump-byte-identical, verify-clean,
  re-fold idempotent), R2 deterministic fold, R3 delta fold (ids
  preserved; provenance may widen — that is D3's annotation union),
  R4 conflict aborts without writes (same-tag/different-bytes; stored-
  vs-shard outcome contradiction), R5 crash windows (orphan skip,
  sidecar-without-bin, bin/sidecar sha mismatch), R6 the gate matrix,
  R7 censor rows survive the fold + max-rule import idempotence,
  R8 verify green + six corrupted-DB reds (missing child, flipped
  outcome, illegal move, ply break, node_count census, empty
  provenance).
- `tests/proofdb.rs`: plan17 manifest-sync tests replaced by the
  plan22 sidecar contract (censor-only sessions touch no durable file;
  produced pairs are self-describing with a matching digest); the
  standing-layer rebuild pin and the stale-DB guard test retired with
  their contracts (pointer comment to `proofdb_fold.rs`); the fresh
  boot test reworked to the fold world (bootstrap fold → session →
  durable pairs → fold); `walk/tests.rs` and the fixture updated to the
  new `Session::new`/`DbContent` APIs.
- `tests/proofdb_merge_cli.rs`: censor-census assertions re-pinned to
  the fold grammar (`1 already merged`; `censor: N imported records
  (max rule, in place)`); new CLI test driving `proofdb_verify`
  (exit 0/exit 2 + `--backup` readable copy).
- `tests/proofdb_harvest_cli.rs`: fixture converted to the sidecar
  world (bin + backfilled sidecar copied per test; no `--manifest`
  flag).

## 4. Problems encountered / resolved during recovery

- The crash left `fixture.rs` mid-edit (tuple return type, body
  updated) and a tag-mismatched sidecar failure-injection test
  (`h_x` vs `h_abc`) — both fixed.
- The transposition trap: the plan's R3 draft assumed the reordered
  path `g2g4 e7e6 f2f3` reaches the TACTIC position — it does not (the
  en-passant field/halfmove clock differ, and the fold's hash-fidelity
  check correctly refuses it). R3/R7 use a second *valid* tactic line
  instead: `1.f3 e5 2.g4` (path `f2f3 e7e5 g2g4`, same `Qd8-h4`
  win-in-1) — disjoint graft into the shared `f2f3`/`g2g4` ancestors.
- R4(b)'s contradiction needed the standing row corrupted first (a
  loss-vs-win overlay), with the batch-1 bin restored so the abort
  surfaces at the overlay, not at the earlier standing-sha cross-check.
- `meta.node_count` verification lives in `check_meta` (now a public
  `verify.rs` helper, driven by the CLI before `verify_db`) — moved out
  of the example so R8 can exercise it.
- r3's "batch-1 rows unchanged" had to tolerate provenance widening
  (the new tag joins the annotation set — D3's intended semantics); the
  test now pins value-column stability and provenance retention
  separately.

## 5. Missing tests / known gaps

- The plan17 in-process orphan-window test is gone with
  `sync_manifest`; the plan22 orphan window is covered structurally
  (R5's bin-without-sidecar fold skip + the manual contract) but the
  *session-abort* path (`fail()` on a sidecar-write failure) remains
  untestable in-process (documented in the test header).
- R1 runs the full 262-shard fixture fold + rebuild + verify (~1 s
  here) — well inside the fast gate.
- The slow tier (`make test-full`) was not run this session (docs +
  example-side changes only; no search/TT/proof-tree surface touched);
  run it before the next release as usual.
- The standing layer (`data/proofdb/`) has not been migrated to the
  fold world yet — that is item 19's R1-recipe run, after the spec v2
  rewrite.

## 6. Next steps

- **Item 19** (next session): spec v2 standalone rewrite, manual
  rewrite, standing-layer migration (R1 with production paths), AGENTS/
  website re-agreement. plan22's patches keep everything consistent in
  the meantime.
- Later: streaming row-surgery fold (D3's recorded limitation) only if
  multi-GB layers materialize; closure promotion (item 12) and parallel
  harvesters (item 6) unchanged.
