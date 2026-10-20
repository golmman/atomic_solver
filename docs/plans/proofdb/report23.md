# report23 — plan23: contract migration (spec v2, manual rewrite, standing-layer consolidation)

**Date:** 2026-10-20 · **Executes:** `plan23.md` (initiative item 19) ·
**Scope:** docs + data migration only, **no code changes** (as planned; D6 held).

## 1. The migration run (plan §3; every step executed in the pinned order)

| step | command | result vs. pre-registered expectation |
| --- | --- | --- |
| 1. in-place fold | `proofdb_merge --shard-dir data/proofdb/shards-bak` | **match.** `census: folded 0 shards (405 already merged, 0 orphans skipped) … merged nodes 43092 (proven 43028, open 64); skeleton re-validations 0`; `db: … fold digest 47b25861d0f56776` |
| 2. verify + offsite backup | `proofdb_verify --backup data/proofdb-backups/proofdb-47b25861-2026-10-20.db` | **match.** `verify: ok — nodes 43092 (proven 43028, open 64)`; backup written (3.16 MB) |
| 3. flip sanity | `proofdb_flip --shard-dir data/proofdb/shards-bak` | **match.** `flip_analysis: open_rows 64 flips 0 verified 0 root Null (fixpoint 1 rounds)` |
| 4. archive + delete | `tar czf data/proofdb-backups/shards-bak-2026-10-20.tar.gz -C data/proofdb shards-bak && rm -rf …` | archive written (122 KB gzipped — the plan's prose guessed "1.8 MB"; the 405 bins + manifest compress well; harmless); 407 archive entries = 405 bins + manifest + dir entry |
| 5. recreate shard dir | `mkdir -p data/proofdb/shards` | done (the probe finding: a missing dir aborts every tool) |
| 6. bare post-checks | `proofdb_merge`, `proofdb_flip` | **match.** `folded 0 shards (0 already merged …) merged nodes 43092 (proven 43028, open 64)`; `fold digest 47b25861d0f56776` unchanged; `flips 0 verified 0`; both exit 0 |

**Pins:**

- `nodes`/`censor`/`shards` rows **byte-equal before/after** (exact
  row-by-row dump diff, and again after step 6): digest `1824aebd4b8993ae`.
- `built_from`: `70a160a65c9b…` → **`47b25861d0f56776e04dc534cd8c781fdc45e180a54898e179264fbc6c3c5d72`**.
- Census (unchanged by construction): **43,092 nodes = 43,028 proven
  (21,514 win + 21,514 loss) + 64 open**; 405 `shards` rows; 3,617 censor
  rows; 20,220 terminals; max ply 22.
- Standing DB file sha256 at delivery/pin time:
  **`375a2b1fc8a2959e3d60aa6737ba6b479988f23de3e953a3233fbae72b69f430`**
  (see Deviations for why this is not the fold-moment sha).
- Backup artifacts: `data/proofdb-backups/proofdb-47b25861-2026-10-20.db`
  (`VACUUM INTO` offsite copy, verify-green) and
  `data/proofdb-backups/shards-bak-2026-10-20.tar.gz` (the archived
  transport, operator-taste insurance).
- Missing-dir behavior re-confirmed live before documenting: bare
  `proofdb_merge` exit 2 / `proofdb_flip` exit 1 with
  `cannot read shard dir …`; recreated dir → green.
- Spec §7 census query executed against the migrated layer (D4 item 6):
  `(43092, 43028, '47b25861d0f56776e04dc534cd8c781fdc45e180a54898e179264fbc6c3c5d72')`.

## 2. Spec coherence pass (D4 checklist)

`docs/spec/global_proof_store.md` — all six items done:

1. §2 subsection numbering: the censor subsection is now `### 2.1.`; §2.2
   (fold digest) moved before the meta-rows table so the numbered
   subsections run in order.
2. §5.1: new audit-trail statement — after transport deletion, a `shards`
   row is the **permanent provenance record**, the only evidence a shard
   ever existed (cross-referenced §6).
3. §5.2: the legacy manifest is explicitly transitional (migration source
   only; layers under this contract never produce one).
4. §6/§6.1 deletion/backup policy: verified present verbatim (no change).
5. Standalone-ness re-grepped clean after the edits (no `docs/plans/`
   refs, no process vocabulary; the remaining `docs/spec/` reference is
   permitted).
6. §7 census query executed against the migrated DB (output pinned above).

## 3. Manual pass (D5)

`docs/proofdb_pipeline.md`:

- §6 sidecar-anatomy provenance phrasing de-transitioned ("backfilled from
  the legacy manifest by recipe R1's fold" → verbatim example + a factual
  historical note that fresh sidecars are harvest-written).
- R1's standing-layer-migration paragraph replaced by a pointer to §11's
  history entry (the migration is done, not a recipe).
- §2.1 shard-pairs bullet gains the dir-must-exist rule (missing dir = hard
  abort; empty dir = bootstrap state and post-deletion steady state).
- R6 gains the probe finding: after deleting the pairs the *dir itself*
  must exist (or be recreated) — with the `mkdir -p` one-liner.
- §10 gains a "Missing shard dir" troubleshooting bullet.
- §11 history gains the plan23 entry. No grammar/output-format text was
  touched (plan22's live-run pins stand).

## 4. Handoff re-cut (D3) + provenance row

`docs/plans/proofdb/measurements/plan13/handoff/` re-cut in place against
the migrated layer:

- `proofdb.db` = byte-copy of the migrated standing DB (sha256
  `375a2b1f…`, `built_from 47b25861…`).
- `global_proof_store.md` = copy of the post-pass repo spec, with the same
  inline edits as the previous cut (the `move_code` bit layout inlined;
  plus now §5.1's format reference inlined as well — the package greps
  clean of `docs/spec`/`proof_tree_dump` references).
- `HANDOFF.md`: supersession note ("any copy of the 2026-10-16
  *pre-migration* re-cut is superseded; identified by sha256 `02fc7d60…` /
  `built_from 70a160a6…` — the manifest-digest era"), new stamp, and the
  provenance paragraph corrected to v2: `built_from` = the fold digest
  over the `shards` provenance set; the DB is **append-only truth, not
  "deterministically rebuildable"** (the retired claim); future deliveries
  are new folds — growth, not drift.
- `BROWSER_BRIEF.md`: stamps re-pinned; the v2 semantics corrections
  (invariant 2's "at least one proven-loss child", invariant 4's min-over
  proving children, principal-line wording, no-rebuild phrasing).
- `measurements/plan13/README.md`: provenance table + a new "Re-cut
  2026-10-20 (plan23)" header block; the superseded 10-16 re-cut is
  explicitly identified.

Forwarding the package remains the owner's follow-up (unchanged).

## 5. Housekeeping

- `.gitignore`: the `data/` block comment de-transitioned — backup duty is
  now `data/proofdb/proofdb.db` (was "backing up `data/proofdb/shards/`"),
  and the "derived proof DB rebuilt at any time" / "work ledger" phrasing
  replaced by the v2 reality (source-of-truth DB + censor state).
- AGENTS.md: verify-only read; the proofdb paragraph (fold, sidecar pairs,
  verify, manual/spec pointers) is already accurate — untouched.

## 6. Gate

`cargo fmt` clean; `cargo clippy --all-targets` clean; `cargo doc` clean;
`make test` (`CARGO_PROFILE_RELEASE_LTO=thin cargo test --release`) — all
suites green, 0 failures (as expected: docs + data only; the fold/verify
machinery the migration exercised is covered by `tests/proofdb_fold.rs`).

## 7. Deviations and findings

1. **File sha vs. fold-moment sha.** Immediately after the migration fold
   the file sha was `f5052686…`; the bare step-6 `proofdb_merge` (an
   idempotent no-op fold) still commits a transaction, which rewrote file
   bytes (rows unchanged) → the standing and delivered file is
   `375a2b1f…`. The plan anticipated exactly this ("the plan probe cannot
   pin it — the live fold's SQLite write ordering may differ"), but the
   mechanism was the post-check re-commit, not write-ordering. Worth
   knowing for future digest pinning: pin the sha *after* the last write.
2. **Tarball size** 122 KB, not the plan's "1.8 MB" guess (compression).
   No consequence.
3. **Census drift risk** (plan §5, risk 3) did not materialize: no harvest
   ran between plan and execution; censor rows still 3,617.
4. **Observation (out of scope):** the committed development fixture at
   `docs/plans/proofdb/shards/` (263 bins + manifest, the manual's §2.1
   development/validation fixture) is untouched and remains correct under
   the v2 contract (explicit-flags development mode).

## 8. Unresolved parts / missing tests

- None in scope. The plan's D6 (no code changes, no new tests) held; the
  missing-dir abort remains documented behavior (§10/R6), not a defect.
- The 60 s regression/stress tier (`make test-full`) was not run — plan §4
  requires only the fast gate for a docs/data-only change.

## 9. Next steps

1. **Owner:** forward the re-cut handoff package
   (`docs/plans/proofdb/measurements/plan13/handoff/`) — the pre-migration
   10-16 re-cut is superseded; copies identified by `built_from 70a160a6…`
   must be discarded.
2. **Owner/operator:** the standing layer is in its final fold-world shape;
   next growth (any new harvest batch) is a separate session and will
   simply extend the DB (R2), no migration involved.
3. Initiative backlog remains: item 6 (parallel harvesters), item 11
   (ledger-leaf policy), item 12 (closure promotion) — all low prio; the
   17→18→19 pivot program is closed.

## Tools used

`cargo build/test/fmt/clippy/doc`, the four proofdb example binaries,
`sqlite3` via Python stdlib (row-equality pinning, census), `tar`, `diff`.
No new scripts committed (the row-diff was a throwaway `/tmp` probe).
