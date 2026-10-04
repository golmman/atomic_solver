# plan13 — item 5: website handoff + user manual

**Initiative:** `proofdb` · **Executes:** backlog item 5 (website handoff), plus the
owner-requested user manual · **Size:** S (docs + measurement artifacts only; no `src/` or
`examples/` code changes) · **Date drafted:** 2026-10-02 (plan session, docs-only)

## 0. State this plan starts from

- Item 8 (tooling completeness audit + hardening) closed with plan12:
  the pipeline is pinned (standing DB `0d929f4c…`, 55,703 nodes, rebuild
  byte-identical, regression test in the default gate), the operator
  runbook `docs/proofdb_pipeline.md` exists, and the remaining levers are
  items 5 (website handoff, S) and 6 (parallel harvesters, M).
- `docs/spec/global_proof_store.md` (schema v1, 141 lines) is the sole
  external contract; verified standalone this session (no references to
  `docs/plans/`, reports, or repo process vocabulary).
- The standing working layer `data/proofdb.db` re-verified this session:
  `sha256 = 0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8`
  (matches the plan12 pin).
- Item 5's text: "ship `global_proof_store.md` to the external website
  project; agree on read-only DB consumption". The *agreement* with the
  external project is an owner action outside any session; this plan
  makes the handoff **shippable**: everything the website project needs,
  packaged, pinned, and documented, so the owner can hand over a single
  directory.
- The container has no `sqlite3` CLI; consumer-side sanity probes in this
  plan use Python's stdlib `sqlite3` module (the website project can be
  expected to read SQLite programmatically anyway).

## 1. Decisions pre-registered

1. **User manual scope: the proofdb user surface.** The manual covers the
   pipeline a consumer/operator of the proof DB actually touches: shard
   production context (`reconstruct_pt`, `inspect_pt` — how shards come to
   exist), the four pipeline tools (`proofdb_merge`, `proofdb_harvest`,
   `proofdb_flip`, `proofdb_ledger_union`), the data model (path identity,
   AND/OR roles, open nodes, depth vocabulary), and worked recipes. The
   full `examples/` catalog (benchmark, replay, twin_stats, …) is **out of
   scope** — AGENTS.md already carries that catalog, and a second catalog
   would double the maintenance surface. If the owner wants a whole-repo
   manual later, it is a separate follow-up.
2. **One canonical doc, no new manual file** (owner decision 2026-10-02,
   amending the originally drafted `docs/plans/proofdb/manual.md`): the
   user-manual content is folded into the existing operator runbook
   `docs/proofdb_pipeline.md`, which already has the right skeleton (§4
   CLI reference with option tables and exit codes; §5 soundness contract;
   §6 limitations). The doc is retitled to cover both roles (operator +
   user) and gains a TOC; the existing §1–§3 (pipeline map, quickstart,
   layer layout) stay byte-stable where possible so plan12's pins remain
   verifiable, and the §7 history gains a plan13 note. Rationale: a single
   source of truth cannot drift against itself, and AGENTS.md already
   points at this doc twice — no reference updates and no file that sits
   awkwardly under `docs/plans/`. Accepted cost: the doc grows to roughly
   600–800 lines (no `docs/` size limit; TOC keeps it navigable).
3. **Doc placement:** `docs/proofdb_pipeline.md` stays where it is
   (top-level `docs/`, outside `docs/plans/`) — it is already the
   referenced doc (AGENTS.md ×2) and the natural home for both operator
   and consumer readers. No file is added or removed.
4. **Spec change is additive only.** One new section
   ("Example consumer queries", 3–5 read-only SQL examples) is appended to
   `docs/spec/global_proof_store.md` as §7. It references nothing outside
   `docs/spec/` (standalone rule); it adds no normative requirement —
   schema, invariants, and meta rows are unchanged. The full query/recipe
   set lives in the manual, which is repo-internal.
5. **The handoff artifact is a fresh, digest-pinned DB copy**, rebuilt by
   this session from the committed shard set (not copied from `data/`),
   committed under `measurements/plan13/` per the measurement conventions
   (validated DB artifacts are committable; transcripts are not).
6. **Handoff = one directory.** The package the owner forwards is
   `docs/plans/proofdb/measurements/plan13/handoff/`: the DB, its census,
   the spec (verbatim copy or pointer), and a short HANDOFF.md naming the
   read-only consumption contract. Nothing external-facing is added
   outside `docs/spec/` (which is already the contract surface). The
   consumer recipes/queries live in the extended `docs/proofdb_pipeline.md`
   (repo-internal; decision 4 applies to the spec only), and `HANDOFF.md`
   points at both.

## 2. Deliverables

- **D1 — user manual content, folded into** `docs/proofdb_pipeline.md`
  (decision 2):
  - TOC + retitle (operator and user manual).
  - New data-model primer section (path identity, AND/OR roles, open
    semantics, depth vocabulary — consumer-level prose pointing at the
    spec as normative).
  - §4 CLI reference expanded into full per-tool chapters
    (`proofdb_merge`, `proofdb_harvest`, `proofdb_flip`,
    `proofdb_ledger_union`): purpose, prerequisites, full option table
    (kept), exit codes (kept), **output line grammar** (census line, the
    `job: {…}` JSON fields as emitted by
    `examples/proofdb/harvest.rs`, the harvest summary line, flip JSON
    shape, ledger union census), worked invocations with real pinned
    outputs from the quickstart.
  - New shard-provenance section: how a shard comes to exist (harvest job
    → solve → reconstruct → validate → manifest entry), and how to
    inspect one with `inspect_pt`/`pt_keys` (read-only consumers).
  - New recipes section: rebuild the DB; grow it by one batch and merge;
    check for implied flips; normalize/merge ledgers; answer the
    questions a website would ask (children of a node, replay a line,
    list the open frontier, shortest proven line at ply k — SQL over the
    schema).
  - §6 limitations extended into troubleshooting (stale-DB `built_from`
    guard, exit-2 merge aborts, orphan shards, fresh-vs-seeded ledger,
    digest drift).
  - Header cross-link to `docs/spec/global_proof_store.md` (normative
    schema) — already present; verify it stands.
- **D2 — handoff package** `docs/plans/proofdb/measurements/plan13/handoff/`:
  - `proofdb.db` — rebuilt this session from the committed manifest +
    shard dir; sha256 must equal the plan12 pin (`0d929f4c…`); census
    recorded beside it.
  - `HANDOFF.md` — one page: what this is, the contract pointer
    (`docs/spec/global_proof_store.md`), the digest + node census, the
    rebuild guarantee (byte-identical from the same shard set), and the
    read-only consumption statement (schema v1, no writes, `built_from`
    digest identifies the manifest).
- **D3 — spec §7** "Example consumer queries" in
  `docs/spec/global_proof_store.md` (additive, standalone; 3–5 queries,
  e.g. children of a path, open frontier at ply k, provenance of a node).
- **D4 — measurements/plan13 README + env.json** per the measurement
  conventions (provenance table, environment snapshot).
- **D5 — bookkeeping:** initiative.md backlog row for item 5 → done (with
  the "agreement with the external project is the owner's follow-up"
  caveat), history entry, `docs/plans/README.md` proofdb row updated
  (item 5 closed; next lever item 6). **Final task: `report13.md`** —
  including tools used, problems, and next steps.

## 3. Tasks (execution session)

1. Rebuild the DB from the committed shard set into
   `measurements/plan13/handoff/proofdb.db`; verify sha256 == `0d929f4c…`;
   record the census (expected pinned values from the runbook §2:
   262/262 shards, 55,703 nodes, proven 55,628 / open 75). Probe the DB
   read-only with Python `sqlite3`: `meta` rows present, `schema_version=1`,
   sample path replay sanity (root has no parent; ≥1 proven child).
2. Extend the runbook into the user manual (D1). Every claimed output
   line must be taken from a
   real run in this session (re-run the runbook §2 quickstart steps 1–4 as
   the source of pinned transcripts; do not transcribe from memory).
3. Write spec §7 (D3); re-verify standalone (grep for `docs/plans`,
   `report`, `plan[0-9]` → no hits); confirm schema untouched (diff shows
   one appended section only).
4. Write `HANDOFF.md`, `measurements/plan13/README.md`, `env.json` (D2/D4).
5. Bookkeeping (D5): initiative.md + plans/README.md rows, history entry.
6. **Final task: write `report13.md`.**
7. Gate: `make test` must stay green (no code changed; the
   `standing_layer_rebuilds_byte_identical` gate re-pins itself).
   Measurement hygiene: `git add --dry-run` check — only the files of
   D1–D5 may appear; no transcripts (`*.out`/`*.log`), no `__pycache__`.

## 4. Success criteria

- The extended `docs/proofdb_pipeline.md` covers every option, exit code,
  and output line of the four tools, with at least one worked invocation
  each, all from this session's runs; the plan12-pinned quickstart
  sections (§1–§3) are unchanged where claimed.
- `handoff/` contains a digest-pinned rebuilt DB + HANDOFF.md; a reader
  with only that directory + the spec can start consuming (no repo
  knowledge required).
- Spec is still standalone and normatively unchanged apart from §7.
- Item 5 marked done in the backlog with the external-agreement caveat;
  the initiative's next lever is unambiguously item 6.

## 5. Out of scope

- Any code change (`src/`, `examples/`, `tests/`), any CLI option change.
- The external agreement itself (owner forwards the package).
- Item 6 (parallel harvesters) / item 7 (subtree scoping) — untouched.
- DB growth: no harvest batch beyond the quickstart smoke (2 censored
  jobs), consistent with the pivot (harvesting = tooling validation only).
