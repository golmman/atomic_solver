# plan14 — centralized generated-data layout: `data/proofdb/`

**Initiative:** `proofdb` · **Executes:** backlog item 9 (owner request,
2026-10-02: "all generated proofdb data centralized in `data/proofdb/` by
default") · **Size:** S (defaults + docs; no `src/` changes, no pipeline
logic changes) · **Date drafted:** 2026-10-02 (plan session, docs-only)

> **Amended 2026-10-02 (owner clarification, same session):** the committed
> shard set at `docs/plans/proofdb/shards/` is a **development/validation
> fixture** documenting the implementation — not a production artifact, and
> not the durable layer of any future production run. Production has not
> started; when it does, **all** generated proofdb data — shards and
> manifest included — lives under `data/proofdb/`, and production runs
> ignore `docs/plans/proofdb/shards/` entirely. Consequence: `--manifest`
> and `--shard-dir` gain defaults too (decision 1 superseded; see the
> amendment markers inline).

## 0. State this plan starts from

- Items 8 and 5 closed with plan12/plan13: the pipeline is pinned
  (standing DB `0d929f4c…`, 55,703 nodes, byte-identical rebuild in the
  default gate), the operator+user manual `docs/proofdb_pipeline.md`
  exists with per-tool CLI tables, and the handoff package shipped.
- Generated (non-committed) proofdb state is scattered:
  - `proofdb_merge --db` defaults to `proofdb.db` **in the cwd**
    (`examples/proofdb_merge.rs:69`);
  - `proofdb_harvest --db` and `proofdb_flip --db` are **required**;
  - `proofdb_harvest --ledger` defaults to `data/proofdb_work.json`
    (flat under `data/`, `examples/proofdb/harvest_args.rs:55`);
  - `proofdb_flip --out` is **required**.
- The committed shard set + manifest live at `docs/plans/proofdb/shards/`
  and are pinned by `tests/proofdb.rs::standing_layer_rebuilds_byte_identical`
  (explicit paths — unaffected by default changes). **Amendment:** their
  role is reclassified to development/validation fixture (see the banner).
- `docs/spec/global_proof_store.md` mentions no file paths (verified this
  session): where the DB file lives is *not* part of the external
  contract, so the spec needs no change.
- `data/` is wholly gitignored (`.gitignore` comment block "Working layer
  of the proofdb initiative…"); this checkout currently has no
  `data/proofdb*` files (clean migration).

## 1. Proposed layout (the deliverable)

```
data/proofdb/                  # default root for ALL generated proofdb state
├── proofdb.db                 # derived SQLite working DB → default --db (merge, harvest, flip)
├── work.json                  # harvest sidecar work ledger → default --ledger
├── flip.json                  # flip report → default --out (proofdb_flip)
└── shards/                    # production shard set → default --shard-dir
    └── manifest.json          # production manifest → default --manifest
```

Roles are unchanged (manual §2): shards+manifest the durable truth,
`proofdb.db` the derived view, `work.json` the selection state,
`flip.json` a throwaway report. Only the *default locations* centralize —
and with the amendment, a production run needs **no path flags at all**:
`proofdb_merge` and `proofdb_harvest` invoked bare operate wholly inside
`data/proofdb/`.

## 2. Decisions pre-registered

1. **Superseded (amendment 2026-10-02): defaults for the shard layer too.**
   The original decision — `--shard-dir`/`--manifest` stay required so the
   durable layer is deliberately explicit — rested on the committed set
   *being* the production durable layer. With the owner's clarification it
   is not: it is a development/validation fixture, and production runs
   ignore it entirely. New decision: all four tools default the shard
   layer into `data/proofdb/`:
   - `--shard-dir` → `data/proofdb/shards/`
   - `--manifest`  → `data/proofdb/shards/manifest.json` (co-located with
     the shards, mirroring the fixture layout and the quickstart)
   The fixture at `docs/plans/proofdb/shards/` keeps its role for tests
   and validation batches (those run with **explicit** flags, as every
   existing plan measurement did); the regression test is untouched.
   A production run never reads it, by default or otherwise.
2. **Default changes (the whole code surface):**
   - `proofdb_merge --db`: `proofdb.db` → `data/proofdb/proofdb.db`;
   - `proofdb_harvest --db`: required → default `data/proofdb/proofdb.db`;
   - `proofdb_flip --db`: required → default `data/proofdb/proofdb.db`;
   - `proofdb_harvest --ledger`: `data/proofdb_work.json` →
     `data/proofdb/work.json`;
   - `proofdb_flip --out`: required → default `data/proofdb/flip.json`.
   Unchanged on purpose: `--dump` (off; recipes that use it write into
   `data/proofdb/`), `--stop-file` (stays `STOP` in the cwd — it is a
   control *input*, not generated state), `--pns-config`, all
   budget/policy options.
3. **Parent-directory creation.** Tools create the default directories on
   first write (`create_dir_all` on the output path's parent — the ledger
   writer already does this, `examples/proofdb/ledger.rs:160`; merge DB
   and flip `--out` gain it; `proofdb_harvest` creates the shard dir when
   writing its first shard / the manifest rewrite). Reading never creates
   anything: a missing default DB or manifest is a clean abort naming the
   path, with the manual's bootstrap recipe as the fix (§8, task 2).
4. **Breaking-change acceptance.** The old defaults are documented in the
   manual §7 (and `proofdb.db` in the cwd was plan12/plan13-era recipe
   prose); this is an accepted breaking change to CLI defaults (owner
   request). No compatibility shim, no path probing. The manual's §11
   history records the switch.
5. **Spec untouched.** `docs/spec/global_proof_store.md` carries no
   paths; the DB location is operator-side, not contract-side.
6. **Quickstart §3 becomes the production-shaped walkthrough.** Today it
   runs in /tmp "so nothing is written into the checkout"; with defaults
   inside `data/proofdb/` it is simpler *and* validates the new defaults:
   run from the repo root with **no path flags** (bootstrap: create the
   empty manifest at `data/proofdb/shards/manifest.json`), cleanup =
   `rm -rf data/proofdb`. All pinned expectations (censuses, digests,
   `job:` lines) re-verified verbatim during execution. Development-mode
   runs against the committed fixture stay available and are documented
   as the **explicit-flags** variant (recipe R1 and the fixture seed,
   task 2).
7. **Migration is a documented recipe, not an executed step.** This
   checkout has no standing working-layer files; operators with one run
   the manual's new one-liner (added to §4 and R5):
   `mkdir -p data/proofdb && mv data/proofdb.db data/proofdb/proofdb.db
   && mv data/proofdb_work.json data/proofdb/work.json`. Renaming does
   not affect the `built_from` staleness guard (digest-based, not
   path-based).
8. **No `src/` changes, no pipeline-behavior changes.** Merges,
   harvests, ledger semantics, digests, and exit codes are untouched —
   R1 over the committed fixture must still produce `0d929f4c…`
   (55,703 nodes); only the output path in the `db:` line changes.
9. **Production durability lives outside git (accepted, owner-decided).**
   `data/` is gitignored, so the production shard set — now the durable
   truth — is not version-controlled. The derived-DB discipline is
   unaffected (the DB stays rebuildable from `data/proofdb/shards/`),
   but **backup of `data/proofdb/shards/` becomes the operator's
   responsibility**; the manual says so in §2 and §9. The manifest digest
   (`built_from`) remains the integrity stamp for any backed-up copy.
   Validation/development shards keep their committed home under
   `docs/plans/proofdb/shards/` (explicit flags, measurement
   conventions), so no committed evidence is lost to this change.

## 3. Tasks

1. **Code (defaults + usage strings + docs-comments):**
   - `examples/proofdb_merge.rs` — defaults `--db`
     `data/proofdb/proofdb.db`, `--manifest` `data/proofdb/shards/manifest.json`,
     `--shard-dir` `data/proofdb/shards/`; create the DB's parent dir
     before the DB write; usage line + header comment;
   - `examples/proofdb/harvest_args.rs` — defaults `--db`
     `data/proofdb/proofdb.db`, `--manifest`
     `data/proofdb/shards/manifest.json`, `--shard-dir`
     `data/proofdb/shards/`, `--ledger` `data/proofdb/work.json`; usage
     line; shard-dir creation on first shard/manifest write (session.rs);
   - `examples/proofdb_flip.rs` — defaults `--db`
     `data/proofdb/proofdb.db`, `--out` `data/proofdb/flip.json`
     (`--manifest` follows the shared default), usage line, header
     comment;
   - header/doc-comment path references:
     `examples/proofdb/ledger.rs` ("kept next to the working DB under
     `data/`"), `examples/proofdb_ledger_union.rs`
     (`data/proofdb_work.json`);
   - a small unit test pinning the compiled defaults (paths + that the
     defaults are directory-qualified), placed with the tool's existing
     test module or `tests/proofdb.rs`.
2. **Manual `docs/proofdb_pipeline.md`:**
   - §2.1 artifact table paths (`data/proofdb/…`), gitignore column
     (all production rows now "yes" via `data/`; the committed fixture
     row noted as development-only), and the §2.1 shard-set bullet plus
     decision 9's backup responsibility;
   - a short **production vs development** paragraph in §2: production
     = bare defaults wholly inside `data/proofdb/` (fixture ignored);
     development/validation = explicit flags against
     `docs/plans/proofdb/shards/`;
   - §3 quickstart → production-shaped, runs in the checkout with no
     path flags (decision 6), expectations re-pinned;
   - §4 ledger pick-up: new default path + the migration one-liner;
   - §7 all four CLI tables/usage lines/output-grammar examples
     (`db: proofdb.db (nodes …)` → `db: data/proofdb/proofdb.db (nodes …)`,
     the `harvest: db …` stderr line likewise);
   - §8 recipes: R1 (rebuild the **fixture** DB, explicit flags,
     digest `0d929f4c…` pin retained), R2/R3/R5 migrated to defaults,
     new first-production-start bootstrap (empty manifest at
     `data/proofdb/shards/manifest.json`), and an optional recipe to
     seed a fresh production run from the validated fixture
     (`cp docs/plans/proofdb/shards/* data/proofdb/shards/` then R1
     against the defaults);
   - §10 troubleshooting examples (stale-DB message shows the new path;
     missing-default-manifest abort names the bootstrap recipe);
   - §11 history: one plan14 note.
3. **AGENTS.md** — the four `proofdb_*` tool entries gain the default
   path in one clause each (e.g. "DB defaults to `data/proofdb/`").
4. **`.gitignore`** — comment only: name `data/proofdb/` as the
   working-layer root (the `data/` ignore rule itself stays).
5. **Verification:** re-run the quickstart verbatim (pins from task 2),
   recipe R1 over the committed fixture with explicit flags (digest
   `0d929f4c…`, node counts unchanged), and the default-path smokes:
   bare `proofdb_merge` against an empty `data/proofdb/shards/manifest.json`
   writes `data/proofdb/proofdb.db` (creating the directories); bare
   `proofdb_flip` and bare `proofdb_harvest` with no manifest abort
   cleanly naming `data/proofdb/shards/manifest.json`.
6. **`report14.md`** (final task): findings, deviations, gate results;
   update the initiative backlog row and `docs/plans/README.md` only if
   the item closes this session.

## 4. Gates

- `make test` green (includes `standing_layer_rebuilds_byte_identical`).
- `cargo clippy --release --examples`, `cargo fmt --check`, `cargo doc`
  clean.
- Quickstart §3 re-executed verbatim with the new pinned expectations.
- R1 rebuild digest unchanged: `0d929f4c3c62e4bbb87e9b043b582716297b23e2
  f7332a36e82763b1486070b8`, 55,703 nodes.
- `git status` clean of `data/` litter (the whole tree is gitignored;
  nothing new should ever appear).

## 5. Risks and non-goals

- **Risk: stale operator scripts** that rely on the old defaults
  (`proofdb.db` in cwd, `data/proofdb_work.json`, explicit
  manifest/shard-dir). Mitigation: manual §11 history + migration
  one-liner; the failure mode is a clean fresh-ledger/fresh-DB start or
  a named-path abort, never a soundness issue (§9: wrong ledger costs
  work, not correctness).
- **Risk: unversioned production truth** (decision 9). The production
  shard set is the durable layer outside git; a lost
  `data/proofdb/shards/` is unrecoverable except from backups. The
  manual states the backup responsibility; making `data/proofdb/shards/`
  committable (a targeted `.gitignore` exception) is deliberately **not**
  planned — it would mix generated and committed state, the exact
  separation this item removes.
- **Non-goal:** moving the committed fixture, changing the spec,
  defaulting `--pns-config`/`--stop-file`/`--dump`, subdirectory
  taxonomy beyond `shards/` (flat files are the current artifact set).
- **Non-goal:** `--root-fen` subtree harvesting (item 7) and per-worker
  ledgers (item 6) — item 6's per-worker ledger files will naturally live
  under `data/proofdb/` when it lands, but no scaffolding is added now.
