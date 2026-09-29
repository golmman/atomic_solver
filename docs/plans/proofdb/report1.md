# Report 1 — merger MVP + SQLite schema + `global_proof_store.md` spec + seed graft

Executes `plan1.md` (item 1). Session resumed after a crash mid-implementation;
the crashed session had left the spec, the merger code, the tests, and the
manifest driver in place but nothing run and no report. This session verified
the implementation, found and fixed one merger defect and one driver defect
via the pre-registered gates, rebuilt the seed DB, and closed all gates.

## Deliverables

- **D1** `docs/spec/global_proof_store.md` — schema v1, semantics/invariants,
  manifest contract; standalone (only out-of-dir reference is
  `docs/spec/proof_tree_dump.md`). One clarification added this session:
  invariant 7 now states the row-id order is a depth-first lexicographic walk
  (the elementwise/prefix-before-extension wording was technically correct but
  ambiguous — see finding 3).
- **D2** `examples/proofdb_merge.rs` + `examples/proofdb/` (`mod.rs` manifest
  protocol, `schema.rs` DB/dump writers, `merge.rs` path tree + overlay,
  `merge/depth.rs` refinement fixpoint + row ids, `merge/skeleton.rs`
  skeleton reassembly, `merge/tests.rs` unit tests). All files ≤ 10 KB.
- **D3** `measurements/plan1/`: `proofdb_seed.db` (12,081 nodes),
  `nodes_seed.txt` (canonical dump), `manifest_seed.json`,
  `build_seed_manifest.py`, `README.md` (provenance), `env.json`.
- **D4** this report.

## Census (gate G1)

- 95/95 shards parsed, replay-validated (`validate_proof_tree`), manifest
  cross-checked (outcome vs tree root), and hash-fidelity-checked (replayed
  startpos path == manifest FEN == tree root FEN, by Zobrist hash).
- Node arithmetic, reproducible from the dump: shard tree nodes 12,592 =
  11,905 new + 687 deduped; merged 12,081 = 1 root + 11,905 overlay +
  175 open-ancestor insertions; 89 open nodes upgraded by cross-route proofs;
  provenance conflicts: 0.
- Merged node outcomes: 11,994 proven (5,996 win bound; 5,667 loss exact at
  rule-terminals; 331 loss bound) + 87 open.
- Wall ≈ 15 s for the full pipeline including re-validation and DB write.

## Gate verdicts

| gate | verdict | notes |
| --- | --- | --- |
| G1 graft census | **pass** | all 95 validate; arithmetic reconciles (above) |
| G2 merged-tree validation | **pass with deviation** | implemented as per-graft skeleton re-validation (95 legs) over the *final merged* structure, not one startpos-rooted `ProofTree`: the startpos root is `open`, so it cannot be the proven root of a `ProofTree`, and the 87 open ancestor nodes are not skeleton material. Per-graft coverage is a superset: every proven merged node lies under some graft root, and each skeleton is reassembled from the post-`finalize` (refined, order-independent) structure. Deviation recorded in the CLI module header. |
| G3 determinism | **pass** | two full runs → byte-identical DB and dump |
| G4 replay spot-checks | **pass** | 10 sample lines printed; `c2c3 g7g5` reads `win, depth_bound=7, status=bound` (matches the manifest's `dtm_length` 7); the `c2c3 g7g5 d1a4` (2...g5 3.Qa4) node reads `loss, bound 6` |
| G5 hygiene | **pass** | `make test` green (240 unit + all fast integration, incl. 10 new proofdb tests); `cargo clippy` / `cargo fmt --check` clean; no `src/` changes |

Additional integrity check over the final DB: 0 bad parent links and 0
orphans across all 12,081 rows (every `parent_id` resolves and every parent
is exactly one ply above its child).

## Findings

1. **Merger defect caught by the gates: parent-row id inversion.** The first
   full-seed DB was wrong: `g7g5` under `c2c3` was missing and unrelated
   nodes appeared as children. Root cause: `row_of` (the lexicographic
   rank→index order vector) was also used as an index→rank lookup for parent
   row ids — correct only when the two orders coincide, which the single-chain
   fixture masked. Fixed by keeping two mappings (`row_order` for row
   iteration, `row_of` for parent lookups) plus a regression test
   (`parent_rows_follow_lexicographic_order`) that forces a non-identity
   order. Note the pre-registered G2 as originally worded (whole-tree
   re-validation) operates on the in-memory tree and would **not** have caught
   this; it was the DB-level G4 spot-check that did. The gates are the
   defect-finding mechanism precisely as plan §4 intended.
2. **Manifest driver nondeterminism.** `build_seed_manifest.py`'s summary
   dict iterated a set for `path_sources`, so manifest bytes (and hence the
   DB's `built_from` digest) differed between runs. Fixed with `sorted()`;
   two runs are now byte-identical (sha256 `92e49fcb…`).
3. **Row-id order clarified.** "Lexicographic path order" is depth-first
   (each node sorts before its extensions): root, `a2a3`, `a2a3 d8h4`,
   `b2b3`, `b2b3 d8h4`. Spec invariant 7 now says so explicitly.
4. **Depth-status mapping (plan task 1) resolved structurally.** Plan4's
   `dtm_length` comes from the proof trees' root depths (`report4.md`), not
   from a documented minimality proof, so per plan §3 the seed maps
   conservatively. The implementation is stronger than a per-node guess:
   `finalize` recomputes every bound bottom-up over the merged structure
   (order-independent, always ≤ every contributing shard's claim), and
   `exact` is only claimed where structurally forced — rule-terminals
   (depth 0 by definition). Every non-terminal proven node is therefore
   `bound` (histogram above); the future DTM-upgrade pass (item 3) is the
   intended `exact` source.
5. **`merge.rs` split.** The file had grown to 27 KB; split into
   `merge/{depth,skeleton,tests}.rs` submodules (all ≤ 10 KB), mirroring the
   repo's `*/tests.rs` pattern. `tests/proofdb.rs` includes the modules via
   `#[path]` so the unit tests run in the default gate.

## Tools and dependencies

- New dev-dependencies (examples/tests only, no product impact): `rusqlite`
  (bundled — no system libsqlite3 in this container), `sha2`.
- Gate checks used the `sqlite3` Python stdlib module (read-only queries).
- No product (`src/`) changes; `Cargo.toml`/`Cargo.lock` touched for the
  dev-dependencies only.

## Problems encountered

- Session crash mid-implementation: recovered state from the working tree
  (`git status` + reading the left-behind code) rather than re-planning; the
  plan was self-contained enough to resume against.

## Unresolved parts / missing tests

- No black-box CLI test of the `proofdb_merge` binary itself (argument
  parsing, exit codes on conflict/defect); the end-to-end fixture test drives
  the same pipeline in-process. The conflict-abort path is unit-tested at the
  `PathTree` level (`outcome_contradiction_aborts`).
- `build_seed_manifest.py` (Python, stdlib-only) has no unit tests; its
  output is verified by replay inside the driver and re-checked by hash in
  the merger.
- The manifest `path` column stores space-separated UCI; no consumer exists
  yet (website handoff is item 5) — the schema is exercised only by the
  merger's own writers and the gate queries.

## Next steps

1. Harvest loop (item 2): campaign-worker-shaped shard production; the merger
   runs after each batch (determinism gate re-run per batch).
2. DTM-upgrade pass (item 3): promote `bound` → `exact` where affordable;
   `exact` claims then flow through the existing `finalize` consistency
   checks.
3. Website handoff (item 5): ship `docs/spec/global_proof_store.md` and the
   seeded DB.

SESSION COMPLETE
- report1.md written; seed DB + dump + manifest + provenance committed under
  `measurements/plan1/`; gates G1–G5 pass; index + initiative updated
  (item 1 done)
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan2.md — the harvest loop
   (item 2): campaign-worker-shaped shard production with per-batch merger
   runs. Draft plan2.md first if it does not exist yet."
2. Alternative: hand the seeded DB + `docs/spec/global_proof_store.md` to
   the external website project now (item 5) so schema feedback arrives
   before the DB grows under the harvest loop.
