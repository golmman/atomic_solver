# Report 13 — item 5: website handoff + user manual (executed)

Executes `plan13.md`. Session type: execution. Docs + measurement artifacts
only — no `src/`, `examples/`, or `tests/` changes; gate stayed green.

## 1. What was done (per plan task order)

### Task 1 — the rebuilt, digest-pinned DB + read-only probe

- `proofdb_merge` rebuilt the DB from the committed manifest (`e91ad57b…`,
  262 entries) + shard dir into
  `docs/plans/proofdb/measurements/plan13/handoff/proofdb.db`:
  **sha256 `0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b148607
  0b8` — exactly the plan12 pin**, census 262/262 shards, 55,703 nodes
  (proven 55,628, open 75), skeleton re-validations 262.
- Read-only probe (Python stdlib `sqlite3`, per plan): all five `meta`
  rows present with `schema_version = 1`; root `id = 0, parent_id = NULL`;
  recursive-CTE path reconstruction verified (node 55416 →
  `e2e4 h7h6 d1h5 e7e5 h5f7`, matching an independent `--sample-lines`
  walk of the merger); census queries verified (55,628 proven = 27,768 win
  + 27,860 loss; depth_status 30,795 `bound` / 24,833 `exact`; max ply 48;
  24,833 rule-terminal rows).
- **Plan-item correction (finding 1):** the plan's probe item "root has no
  parent; ≥1 proven child" is half-wrong for the standing state — the root
  has **no proven direct child** (all 7 stored ply-1 rows are open; the
  root is undecided, as pinned since plan10). The accurate sanity fact —
  55,628 proven nodes exist deeper in the tree, and the root's children
  carry resolved reply grandchildren — is recorded in
  `measurements/plan13/README.md`. No DB defect; the probe item, not the
  DB, was over-optimistic.

### Task 2 — D1: the runbook became the operator + user manual

`docs/proofdb_pipeline.md` (was 259 lines, now ~770): retitled, TOC added,
and extended with:

- **§4 Data model primer (consumer)** — path identity, the derivable
  AND/OR roles table, the two consumer traps spelled out (partial tree:
  absence of a child row is not evidence; `NULL` covers both
  "searched-undecided" and "unsearched"), depth vocabulary, provenance,
  replay-verifiability, `meta` as a version stamp — consumer-level prose
  pointing at the spec as normative.
- **§5 Shard provenance** — how a shard comes to exist (job → solve →
  export/validate → manifest → double validation at merge), a verbatim
  manifest entry, `inspect_pt`/`pt_keys` inspection examples (real
  outputs, shard `h_013861d97dce5d48.bin`).
- **§6 CLI reference** — the four tools as full per-tool chapters:
  purpose, prerequisites, usage, option tables (kept verbatim), exit codes
  (kept), and **output line grammars**: the merge census/`node
  arithmetic`/`db:`/`sample` lines; the harvest stderr session-start
  block, the `job: {…}` field table (from
  `examples/proofdb/session/record.rs` — 16 fields incl. the PNS-only
  `number`/`kind`), censored (this session) and decisive (pinned plan10
  census) examples, the `manifest:`/`harvest:` summary lines per policy
  family, the flip line + JSON shape, the ledger-union census lines.
- **§7 Recipes** — operator (R1 rebuild, R2 grow+merge+flip, R3 flip
  check, R4 ledger normalize/merge, R5 working-dir retirement) and
  consumer SQL (path resolution both directions incl. the recursive-CTE
  path reconstruction, line membership, children, open frontier, shortest
  proven lines, provenance/shard audit, census) — all queries executed
  against the standing DB this session, result counts stated.
- **§8 Soundness contract** (was §5, text unchanged), **§9
  Troubleshooting** (was §6, extended: stale-DB guard with the real abort
  message, exit-2 merge aborts, orphan shards, fresh-vs-seeded ledger,
  digest drift after growth), **§10 History** (+ plan13 note).
- §1–§3 kept byte-stable (diff-verified against the pre-edit file), **with
  one deliberate correction (finding 2):** §2's smoke-harvest expectation
  said the `job:` lines carry `"kind": null` — the observed lines **omit**
  `number`/`kind` entirely (`and-close` emits `pass`/`work_before` only);
  corrected to the observed bytes, consistent with report12's own
  expectation-fixing precedent.

### Task 3 — D3: spec §7, additive-only

`docs/spec/global_proof_store.md` §7 "Example consumer queries" appended
(59 insertions, 0 deletions per `git diff`): path reconstruction
(recursive CTE), children, open frontier, shallowest proven wins, census,
plus the app-side note that a UCI line resolves by one indexed lookup per
move (no path column by design). Standalone check: `grep -nE
'docs/plans|report|plan[0-9]|proofdb|measurements|initiative'` → no hits;
schema/invariants/meta rows untouched.

### Task 4 — D2/D4: handoff package + measurement README/env

`measurements/plan13/handoff/` — the single directory the owner forwards:

- `proofdb.db` (3.6 MB, digest-pinned as above),
- `HANDOFF.md` (what this is; the contract; the version stamp incl.
  `sha256`/`built_from`/census to verify after receiving; the read-only
  consumption statement: partial tree, absence ≠ evidence, `NULL` =
  undecided, spec §4 as the guarantee surface),
- `global_proof_store.md` — **verbatim copy** of the spec
  (diff-verified identical), making the package self-contained.

`measurements/plan13/README.md` (provenance table, session-run record,
probe results incl. finding 1, hygiene) and `env.json` (rev `17212bc4…`,
rustc 1.98.1, input/output state).

### Task 5 — D5 bookkeeping

- `initiative.md`: item 5 row → done with the caveat ("the agreement with
  the external project itself is the owner's follow-up — forward the
  package"); history entry added.
- `docs/plans/README.md`: proofdb row updated — item 5 closed, next lever
  unambiguously item 6 (parallel harvesters).

## 2. Tools used

`cargo build --release --examples`; the four proofdb binaries;
`inspect_pt`/`pt_keys` (read-only shard inspection); Python stdlib
`sqlite3` (no `sqlite3` CLI in the container, as the plan pre-registered);
`sha256sum`; `diff` for the byte-stability and spec-verbatim checks.

## 3. Problems encountered

- **Finding 1 (plan-item, not product):** the pre-registered root-probe
  item ("≥1 proven child") misread the standing state; corrected in the
  README (see Task 1). Nothing to fix in the pipeline.
- **Finding 2 (runbook):** §2's `"kind": null` expectation string was a
  plan12 transcription slip (report12 fixed two sibling slips but missed
  this one); fixed to observed bytes. §2's cross-reference "see §5 for
  the regression test" was updated to §8 as a mechanical consequence of
  the section renumbering — together these two are the only §1–§3
  deviations from byte-stability (diff-verified).
- The handoff DB is 3.6 MB — the largest committed measurement artifact
  so far (plan1's seed DB is 744 KB). Consistent with the conventions
  (validated DB artifacts are committable); noted for future growth
  planning: if the DB grows two orders of magnitude, the handoff package
  needs an alternative delivery channel (out of plan13 scope).

## 4. Unresolved parts / missing tests

- None in plan13's deliverable list; all success criteria met.
- Known drift risk (inherited from report12): the manual's §2 pins
  (`0d929f4c…`, 55,703 nodes) change with every standing-layer advance;
  §9's "digest drift after growth" troubleshooting entry now tells the
  operator how to distinguish growth from drift, but a future plan should
  decide whether the quickstart should pin a manifest→DB digest table.

## 5. Next steps

1. **Owner action (the only remaining piece of item 5):** forward
   `docs/plans/proofdb/measurements/plan13/handoff/` to the external
   website project; the agreement on read-only consumption is theirs to
   make against the packaged spec.
2. **Item 6 — parallel harvesters** (the initiative's next lever;
   per-worker frontier partitions + `proofdb_ledger_union`, with report4
   finding 3's work-aware selection key as the prerequisite for making
   large batches worth scaling).
3. Optional S-sized hygiene: split `tests/proofdb.rs` (report12 finding,
   untouched).

## 6. Gate and hygiene

- `make test` green (all suites; `standing_layer_rebuilds_byte_identical`
  re-pinned `0d929f4c…` / 55,703 nodes inside the gate).
- `git add --dry-run`: exactly the deliverable set (manual, spec,
  initiative/index, plan13 measurements + handoff) — no transcripts
  (`*.out`/`*.log`), no `__pycache__`, no `*.tt*`; runs executed in
  `/tmp/proofdb_quickstart` (regenerable).
