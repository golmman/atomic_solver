# plan23 — Contract migration: spec v2, manual rewrite, standing-layer consolidation

**Initiative:** `proofdb` · **Executes:** backlog item 19 (added 2026-10-15,
owner direction; runs after item 18/plan22) · **Size:** S–M (one pinned
operator migration run + a docs consolidation pass; **no code changes**) ·
**Date drafted:** 2026-10-16 (plan session, docs-only; all probes read-only
against copies in `/tmp`)

> **Item 19's own text:** "`docs/spec/global_proof_store.md` rewritten as
> the v2 contract (DB = truth; `censor` table semantics; per-shard
> provenance as the audit trail; deletion/backup policy) — standalone, no
> plans vocabulary; `docs/proofdb_pipeline.md` rewritten around the new
> artifact graph (R-recipes: backup replaces rebuild; no more
> re-merge-from-shards); AGENTS.md; migration of the standing layer (fold
> existing shards once, adopt the new contract)."

**Honest scoping note (read first).** Plan22's D10 already patched the spec
and manual so *nothing is actively wrong*: the repo spec carries the fold
digest, the sidecar contract, DB-as-truth, the amended invariants 2/4/7/8,
and the fold discipline; the manual already runs the fold world (R0–R6,
§7.4 `proofdb_verify`). A from-scratch rewrite would re-litigate settled
semantics. What item 19 actually still owns — verified this session — is:

1. the **standing-layer migration run** (still outstanding: report22 §5);
2. a **final coherence pass** over spec and manual so they read as if v2
   were the original design, not a patch series (numbering, transitional
   phrasing, one new audit-trail statement);
3. the **external agreement re-cut** — and this is bigger than the item
   text anticipated: the handoff package at
   `measurements/plan13/handoff/` was re-cut **today, post-plan22 but
   pre-migration**, against the *pre-fold* standing DB with a spec copy
   that still carries v1 semantics (`built_from` = manifest digest,
   invariant 2 "exactly one proving child", censor "preserved across its
   full rebuild") and a HANDOFF.md that calls the `nodes` content
   "deterministically rebuildable" — a claim v2 deliberately retired.
   That package is the current external agreement; it must be superseded
   by a post-migration, spec-v2 re-cut within this plan.

## 0. State this plan starts from (verified this session)

- **Standing layer** `data/proofdb/`: `proofdb.db` (schema v2, 43,092
  nodes / 43,028 proven / 64 open, 405 `shards` rows, 3,617 censor rows,
  `built_from = 70a160a6…` — the pre-plan22 *manifest-digest* stamp), plus
  `flip.json`. The shard transport sits renamed at `shards-bak/`:
  405 bins + the legacy `manifest.json` (v1 entry format, `validate: "ok"`,
  no `sha256`), **no sidecars** — the pre-plan22 layout, parked by the
  owner's restore (report21 §2/§6 history). The DB is therefore still the
  old rebuild's DB: the bin sha256s match the stored rows (spot-checked),
  but `built_from` ≠ the fold digest.
- **The migration recipe is verified at plan time** (§1 probes): an
  in-place fold of the standing DB over `shards-bak` folds zero shards
  (all 405 tags already in `shards`, sha256 cross-checks pass) and
  re-stamps `built_from` to the fold digest — `47b25861d0f56776…` — with
  `nodes`/`censor`/`shards` rows byte-equal before/after, then
  `verify: ok — nodes 43092 (proven 43028, open 64)` and
  `flip_analysis: open_rows 64 flips 0 verified 0`.
- **Probe finding (new, goes into the manual):** a *missing* shard dir
  aborts all three tools (`scan_shard_dir` → "cannot read shard dir",
  merge exit 2 / harvest+flip exit 1); an existing empty dir is the
  bootstrap state. So deleting `shards-bak/` wholesale leaves the layer
  wedged until `mkdir -p data/proofdb/shards` — the manual's R0 already
  says this for first start, but R6 (deletion) and the migration recipe
  must say it too.
- **The handoff package** `measurements/plan13/handoff/` was re-cut
  2026-10-16 15:22–15:24 (documented in `measurements/plan13/README.md`):
  `proofdb.db` byte-identical to the current standing layer
  (sha256 `02fc7d60…`, `built_from 70a160a6…`), a spec copy with v1
  semantics (diff vs. the repo spec: `built_from` = "SHA-256 of the
  manifest file", no §2.2, invariant 2 "exactly one proving child", censor
  "preserved across its full rebuild"), `HANDOFF.md` stamping that DB and
  describing `nodes` as "a derived view, deterministically rebuildable
  from the same source set", and `BROWSER_BRIEF.md` pinning the same
  sha256/census. All of it is internally consistent — with the *v1*
  contract. The migration fold will change `built_from` (and the file
  sha256) out from under it.
- **Docs state:** spec (`docs/spec/global_proof_store.md`, 14.5 KB) —
  v2-accurate, standalone (no plan vocabulary; grep clean); structural
  nits only (subsection numbering: the censor subsection lacks its "2.1."
  label; §2.2 sits after the meta-rows table). Manual
  (`docs/proofdb_pipeline.md`, 1,392 lines) — fold-world structure in
  place; residual transitional phrasing (§5's sidecar anatomy example
  "backfilled from the legacy manifest by recipe R1's fold"; R1's
  standing-layer-migration paragraph to be replaced by history after the
  run; §11 history needs the plan23 entry). `AGENTS.md` — already updated
  by plan22 (fold, verify, sidecars, deletable shards); verify-only.
- **`.gitignore` prose drift:** the `data/` block still says "Backing up
  `data/proofdb/shards/` is the operator's responsibility" — pre-pivot
  wording; the backup duty is now the DB file (§6 of the spec). One-line
  Boy-Scout fix.

## 1. Design rulings (proposed; owner may veto any)

- **D1 — The migration is an in-place no-op fold, not a fresh rebuild.**
  Sequence (§3 task 1; expected outputs pre-registered from today's
  probes): `proofdb_merge --shard-dir data/proofdb/shards-bak` →
  `folded 0 shards (405 already merged, 0 orphans skipped); … merged
  nodes 43092 (proven 43028, open 64)`, fold digest `47b25861…`. No
  `--manifest`: every tag is already merged, so the legacy manifest is
  never consulted; it stays untouched for the backup archive. *Rejected:
  fresh-init fold into a new DB.* It would need the 3,617 censor rows
  carried over (no DB→DB censor copy exists) and would renumber nothing
  anyway (fresh-init ids are identical by R1's guarantee) — the in-place
  fold is strictly simpler and preserves the censor table in place.
- **D2 — Backup precedes deletion; the transport is archived, then
  removed.** Order: fold → `proofdb_verify --backup` → `proofdb_flip`
  → archive `shards-bak/` into the backup dir (`tar czf`) → delete →
  `mkdir -p data/proofdb/shards` → post-checks. The backup goes to
  `data/proofdb-backups/` (outside `data/proofdb/` so a layer wipe cannot
  take it; `data/` is wholly gitignored). The 1.8 MB tarball is
  operator-taste insurance, not contract: the contract says folded pairs
  are deletable, and after deletion the original bins are unrecoverable
  from the DB (plan22 D9 — no export) — the offsite DB backup is the only
  recovery artifact. This is the one irreversible step of the plan; the
  archive keeps it reversible in practice at zero contract cost.
- **D3 — The handoff re-cut supersedes today's re-cut.** The package is
  re-cut again in place (`measurements/plan13/handoff/` — the delivery
  directory the external agreement points at; precedent set by today's
  re-cut): migrated DB copy (new file sha256, `built_from 47b25861…`),
  the spec-v2 copy (redo the one inline edit — inline the `move_code`
  bit layout — over the *repo* spec), `HANDOFF.md` with the corrected
  provenance paragraph (`built_from` = the fold digest over the shard
  provenance set; the DB is append-only truth, **not** "deterministically
  rebuildable"; future deliveries are new folds, growth not drift) and
  the new stamp, `BROWSER_BRIEF.md` re-pins. `measurements/plan13/
  README.md` provenance row updated. The superseded 15:22 package's
  wording ("manifest digest", "exactly one proving child") is exactly
  what the item means by "the external agreement tracks the reality".
- **D4 — Spec: final coherence pass, not a from-scratch rewrite** (the
  semantics were settled by plan22's D10). Checklist, each item verified
  by a full read at execution:
  1. consistent subsection numbering under §2 (label the censor
     subsection `### 2.1.`; the fold digest is `### 2.2.`);
  2. the `shards` table gets its audit-trail statement: after transport
     deletion it is the *permanent provenance record* — the only evidence
     a shard ever existed (new sentence in §5.1 or §6);
  3. §5.2's legacy manifest is clearly marked transitional (migration
     source; new layers never produce one);
  4. §6/§6.1 carry the deletion/backup policy verbatim (already do —
     verify);
  5. standalone-ness: no `docs/plans/` references, no process vocabulary
     (grep-verified; keep clean through the edits);
  6. §7 example queries still run against the migrated DB (execute §7's
     census query against the migrated layer and pin the output).
- **D5 — Manual: targeted rewrite of the residual transitional seams**
  (not a restructure — plan22 did the artifact-graph restructure):
  §5's sidecar anatomy note (the example sidecar's provenance phrasing);
  R1's last paragraph ("Migrating a standing layer to the fold world…")
  becomes a pointer to §11's history entry (the migration is then done,
  not a recipe); R6 gains the probe finding — after deleting the pairs,
  the *dir itself* must exist (or be recreated) before the next bare run;
  §2.1 artifacts table re-checked against the post-migration reality;
  §11 history gains the plan23 entry. No grammar/output-format text
  changes (those were re-pinned by plan22 from live runs).
- **D6 — No code changes.** The missing-dir abort is documented behavior
  (R0's `mkdir`), not a defect; the R1/R2/R8 regression tier already
  covers the fold machinery the migration exercises. `src/` untouched,
  `examples/` untouched.

## 2. Tasks (execution order)

1. **Migration run** (D1/D2, in the pinned order; every expected output
   is in §3). If any probe line deviates: stop, investigate, amend the
   plan before proceeding past the backup step.
2. **Spec coherence pass** (D4 checklist; the census-query run included).
3. **Manual pass** (D5).
4. **Handoff re-cut** (D3) + `measurements/plan13/README.md` update.
5. **Housekeeping:** `.gitignore` backup-duty prose; AGENTS.md
   verify-only read.
6. **Gate:** `make test` (docs + data only, but run it), `cargo fmt`,
   `cargo clippy --all-targets`, `cargo doc`.
7. **`report23.md`** (final task): the migration census/digests, the
   re-cut stamps, deviations, next steps. Update `initiative.md`
   (item-19 row → done; history entry) and the `docs/plans/README.md`
   proofdb row (the pivot program 17→18→19 closes with this plan).

## 3. The migration run — pre-registered script and pins

```bash
BIN=target/release/examples   # after cargo build --release --examples

# 1. Fold the standing layer over the parked transport (in place).
$BIN/proofdb_merge --shard-dir data/proofdb/shards-bak
# expect: census: folded 0 shards (405 already merged, 0 orphans skipped);
#         … merged nodes 43092 (proven 43028, open 64); skeleton re-validations 0
# expect: db: data/proofdb/proofdb.db (nodes 43092), fold digest 47b25861d0f56776

# 2. Replay-validate + the offsite backup (the duty, before any deletion).
$BIN/proofdb_verify --backup data/proofdb-backups/proofdb-47b25861-$(date +%F).db
# expect: verify: ok — nodes 43092 (proven 43028, open 64)

# 3. Flip sanity (also exercises the unmerged gate over the parked dir).
$BIN/proofdb_flip --shard-dir data/proofdb/shards-bak
# expect: flip_analysis: open_rows 64 flips 0 verified 0 (fixpoint 1 rounds)

# 4. Adopt the contract: archive, then delete the transport.
tar czf data/proofdb-backups/shards-bak-$(date +%F).tar.gz -C data/proofdb shards-bak
rm -rf data/proofdb/shards-bak

# 5. Recreate the (empty) production shard dir — a missing dir aborts
#    every tool (probe finding).
mkdir -p data/proofdb/shards

# 6. Post-checks on the adopted layer (bare invocations).
$BIN/proofdb_merge    # expect: folded 0 (405 already merged); digest unchanged
$BIN/proofdb_flip     # expect: flips 0
```

Pins to re-verify at execution: node/censor/shard-row equality with the
pre-fold DB (a `sqlite3`/python check like the plan probe), the census
strings above, and the new DB file sha256 (recorded in the report and the
handoff re-cut; the plan probe cannot pin it — the live fold's SQLite
write ordering may differ from the copy's).

## 4. Test plan

- No new tests (D6): the fold/verify/gate machinery the migration runs is
  covered by `tests/proofdb_fold.rs` (R1–R8) and the CLI targets.
- The gate is `make test` + fmt/clippy/doc; nothing in the committed
  fixture world changes.
- The §7 spec census query against the migrated DB is executed and its
  output recorded in the report (D4 item 6).

## 5. Known risks

- **The migration is irreversible after step 4** (append-only truth; no
  DB→shard export). Mitigations: the plan-time probe already validated
  the whole sequence on a copy; the VACUUM INTO backup and the transport
  archive precede deletion; any deviation from the expected lines stops
  the run before the backup.
- **The handoff re-cut races external consumption.** Today's 15:22
  package may already have been forwarded. The re-cut therefore must
  carry an explicit supersession note (like today's does for the 10-02
  package): "any copy of the 2026-10-16 *pre-migration* re-cut is
  superseded; `built_from 70a160a6…` (manifest-digest era) identifies
  the superseded file". Forwarding is the owner's follow-up, as before.
- **Censor rows grow between plan and execution** if a harvest runs in
  between (3,617 today). The migration is insensitive to that (fold
  touches censor only via `--import-censor`, which the recipe omits);
  only the census strings could drift — re-read them live and re-pin.

## 6. Out of scope (this plan)

- Any new harvest batch or DB growth (tooling validation batches are
  separate sessions).
- Streaming fold, closure promotion (item 12), parallel harvesters
  (item 6), the website implementation itself (external project).
- The spec-copy "inline edit" tooling — it is a one-off sed-grade edit,
  done by hand and diff-verified.
