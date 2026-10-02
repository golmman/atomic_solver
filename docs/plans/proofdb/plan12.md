# Plan 12 — item 8: tooling completeness audit + hardening (executes)

**Item:** proofdb backlog **#8** (tooling completeness audit + hardening).
**Plan numbering:** plan11 was pre-registered (in report10/initiative.md) for
the parked 18B `g1f3` rung; per the session protocol that number is retired,
so this is plan12. This plan supersedes the parked rung idea (it stays parked
unless re-framed as tooling validation).

**Type:** execution session (audit probes already run at plan time — this
plan pins their results as the audit record; the execution session re-runs
the probes, fixes what they found, and writes the docs).

## 0. Scope

Audit the pipeline **end-to-end as a third party would run it from a clean
checkout**: shards → merger → DB → harvest loop → flip/ledger tooling. The
pivot (2026-10-02) defines the deliverable as the *tooling*; harvesting runs
justify themselves only as validation. Product solver (`src/`), DB schema,
and `docs/spec/global_proof_store.md` are **untouched** (constraint 5).

## 1. The audit (measured 2026-10-02, plan session; all probes read-only,
run against the committed tree from a /tmp sandbox)

### 1.1 What a third party gets from a clean checkout (verified sound)

- **Durable layer is complete and self-consistent.**
  `docs/plans/proofdb/shards/` holds 262 `.bin` shards + `manifest.json`
  (1.3 MB): 262 unique tags, 262 unique paths, every `validate == "ok"`,
  every manifest `file` present on disk, zero orphan bins. The 95 seed
  shards (measurements/plan1 `manifest_seed.json`) are all included in the
  standing set. Manifest duplicate tag/path checks are enforced at read
  *and* write time (`examples/proofdb/manifest.rs`), as the spec requires.
- **The merger reproduces the standing DB byte-identically** (the derived-DB
  gate, constraint 3, holds from a clean checkout):
  `proofdb_merge --manifest docs/plans/proofdb/shards/manifest.json
  --shard-dir docs/plans/proofdb/shards --db rebuilt.db --dump …`
  → census `262/262 shards; 55,703 nodes (55,628 proven, 75 open);
  55,360 overlay + 866 deduped + 342 ancestors`, DB sha256
  `0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8`
  (= the standing `data/proofdb.db`), in **1.2 s**.
- **The harvest loop starts cleanly from nothing:** a missing ledger file is
  an empty (fresh) ledger (`examples/proofdb/ledger.rs::load` — documented
  behavior); a 2-job smoke run (`--policy and-close --budget-evals 200000
  --max-jobs 2`) censors both jobs, rewrites the manifest byte-identically
  (digest `e91ad57b…` unchanged), exits 0, writes no shards.
- **Read-only tooling works:** `proofdb_flip` over the rebuilt DB →
  `open_rows 75, flips 0, root Null` (matches report10);
  `proofdb_ledger_union` N=1 over the committed plan10 snapshot
  (`measurements/plan10/ledger_union.json`) normalizes to 8,446 records.
- **Error paths that already fail cleanly:** missing `built_from` meta row →
  `ABORT: meta row "built_from": Query returned no rows` (harvest + flip);
  `validate != "ok"` manifest entry → merger refuses; missing shard file →
  merger refuses; missing CLI argument → usage + exit 1.

### 1.2 Gaps found (the plan's work)

- **G1 (defect, fix):** `examples/proofdb/db.rs:50` — the stale-DB error path
  truncates the DB-stored `built_from` with `&built_from[..16]`. The
  manifest-side value is always 64-hex (`digest_hex`), but the DB-stored
  value is arbitrary: a foreign/corrupted DB with a short `built_from`
  **panics** (`end byte index 16 is out of bounds for string of length 8`,
  exit 134, core dump) instead of the intended clean
  `CONFLICT/DEFECT`-style abort. A third party's first mistake (pointing at
  the wrong DB) produces a Rust panic — unacceptable for the tooling
  surface. Fix: print the values untruncated (they are 64-hex when
  well-formed) or truncate with a length-safe helper; add a regression test
  (malformed `built_from` → non-zero exit + the stale-DB message, no panic).
- **G2 (docs, fix):** there is **no operator runbook**. The only "how to
  run" text lives in per-plan `measurements/*/README.md` command tables and
  plan/report prose — scattered, process-vocabulary-laden, and not written
  for a third party. The consumer-side spec (`docs/spec/global_proof_store.md`)
  deliberately declares production out of scope. Deliverable:
  `docs/proofdb_pipeline.md` — a standalone operator manual (see §2 D2).
- **G3 (docs, fix):** `AGENTS.md`'s Examples list omits all four proofdb
  tools (`proofdb_merge`, `proofdb_harvest`, `proofdb_flip`,
  `proofdb_ledger_union`) — the repo's own map does not show the pipeline.
  Add four one-line entries + a pointer to the runbook.
- **G4 (test, fix):** the clean-checkout reproduction (§1.1, the pivot's
  "derived-DB rebuild is a gate, not an aspiration") exists only as plan10's
  H2 gate narrative — no repeatable artifact. Add an integration test that
  merges the committed manifest into a temp DB and asserts the byte-identical
  digest + node count (1.2 s, inside the `make test` budget; the 1.3 MB
  shard set is already committed). This pins the durable layer against drift
  and is the third party's "does my checkout reproduce the DB?" check.
- **G5 (decision to record, not fix):** the standing work ledger
  (`data/proofdb_work.json`, `de690bc1…`) is scheduling state, not truth —
  it is gitignored and survives only as plan artifacts
  (`measurements/plan10/ledger_union.json`). From a clean checkout a third
  party either starts fresh (sound; censors re-run at base budget, lower
  yield) or seeds the ledger from the committed snapshot. **Decision:** keep
  the ledger out of the durable layer (constraint: durable truth = shards +
  manifest); the runbook documents both options. No code change.
- **G6 (known limitations to document, not fix):** (a) item 7's
  `--root-fen` subtree scoping stays parked — harvest is startpos-rooted
  only; (b) shards written before a mid-batch abort remain as unreferenced
  orphans (deterministically overwritten by retry — harvest module doc);
  (c) `proofdb_harvest --out-db/--dump` are recorded in the summary only,
  the caller runs `proofdb_merge` separately (by design; the runbook states
  the merge-after-batch step explicitly).

## 2. Deliverables

- **D1 — G1 fix:** length-safe `built_from`/digest rendering in
  `examples/proofdb/db.rs` (+ any sibling truncation site found by
  `grep -n "\[\.\.16\]" examples/`); regression test in `tests/proofdb.rs`
  asserting the malformed case aborts cleanly (non-zero, message contains
  `built_from`, no panic). ~15 lines.
- **D2 — `docs/proofdb_pipeline.md`** (the audit's main deliverable), a
  standalone operator manual containing:
  1. the pipeline map (shards + manifest = durable truth → `proofdb_merge`
     → SQLite DB = derived view → `proofdb_harvest` (fresh roots, budgeted,
     censored) → merge again; `proofdb_flip` / `proofdb_ledger_union` as
     analysis / selection-state tools);
  2. the clean-checkout quickstart with the **pinned expected results** from
     §1.1 (build → merge → digest `0d929f4c…`, 55,703 nodes → smoke harvest
     → flip census `75/0/Null`);
  3. the standing-layer layout: committed durable layer
     (`docs/plans/proofdb/shards/`), gitignored working layer (`data/`,
     DB + ledger), ledger pick-up semantics (missing = fresh) and the two
     seed options (G5);
  4. a CLI reference for the four tools (every option, defaults, exit
     codes);
  5. the limitations (G6) and the soundness contract in operator terms
     (only `validate: ok` shards enter; conflicts abort and are never
     patched; the DB is rebuildable at any time).
  It may reference `docs/spec/global_proof_store.md` and the four tool
  usages; it must not depend on per-plan process vocabulary for its
  normative content (history/pointers to `docs/plans/proofdb/` are allowed —
  the docs/spec standalone rule applies to `docs/spec/` only).
- **D3 — G4 test:** `tests/proofdb.rs::standing_layer_rebuilds_byte_identical`
  — merge the committed manifest (relative path from the crate root; assert
  the shard set is present first, with a clear message if run outside the
  checkout) into a temp DB, assert sha256 `0d929f4c…` and 55,703 nodes.
  Runs in the default gate.
- **D4 — G3:** AGENTS.md Examples entries for the four tools (one line
  each, house style) + a pointer to `docs/proofdb_pipeline.md` in the
  Architecture section's proof_tree bullet area (minimal diff).
- **D5 —** update `initiative.md` backlog row #8 (status per §4's closing
  rule) and add a History line; final task: write `report12.md` (audit
  findings as the record, fixes, gate results, tools used, next steps).

## 3. Gates

1. Re-run the §1.1 probes post-fix: merge digest unchanged; harvest smoke
   unchanged; the G1 malformed-DB case now aborts cleanly (exit ≠ 0, no
   panic/core).
2. `make test` green (incl. D3 + the D1 regression test).
3. `cargo clippy --all-targets`, `cargo fmt --check`, `cargo doc` clean.
4. No `src/` changes; files ≤ 10 KB (D2 is docs — exempt); `git status`
   shows only the intended files (docs, examples fix, tests, AGENTS.md).
5. The runbook's quickstart is executed verbatim by the execution session
   exactly as written (from a fresh `/tmp` working copy of the outputs —
   the commands run against the committed tree, outputs to /tmp) — the
   runbook must not be committed with an untested command sequence.

## 4. Closing rule for item 8

The audit found one defect (G1) and doc/test gaps (G2–G4); all are fixed by
this plan; G5/G6 are recorded decisions/limitations, not open work. **If
execution surfaces no new M-sized gap, item 8 closes with plan12** (row
update + history line in the same session) and the initiative's next levers
become items 5 (website handoff — now has a stable surface) and 6 (parallel
harvesters). A newly found gap goes into `report12.md` as a finding and item
8 stays open with the follow-up named.

## 5. Task order

1. D1 fix + regression test; re-run the malformed-DB probe.
2. D3 test (verify it fails if the shard set drifts — mutate a copy of the
   manifest digest expectation, observe red, restore).
3. D2 runbook, drafted from the §1 record; execute its quickstart verbatim.
4. D4 AGENTS.md entries; D5 initiative.md row + history; `report12.md`.
5. Gates (§3); session closure block.
