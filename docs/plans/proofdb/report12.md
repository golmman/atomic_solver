# Report 12 — item 8: tooling completeness audit + hardening (executed)

Executes `plan12.md`. Session type: execution. All plan-time audit numbers
were re-run against the working tree; every pinned expectation held.

## 1. What was done (per plan task order)

### Task 1 — D1: G1 fix + regression test

- `examples/proofdb/db.rs`: the stale-DB error path truncated the DB-stored
  `built_from` with `&built_from[..16]`. Added a length-safe `short()`
  helper (`s.get(..16).unwrap_or(s)` — char-boundary-safe) and switched the
  message to `short(&built_from)` / `short(manifest_sha256)`; values are
  now printed untruncated when shorter than 16 bytes.
- Sibling truncation sites audited (`grep -rn '\.\.16'`): only
  `harvest.rs:87` (`digest_hex` output, always 64-hex),
  `proofdb_merge.rs:334` and `proofdb_harvest.rs:83` (both
  `manifest.sha256_hex`, always 64-hex by construction in
  `read_manifest`) — all safe, left unchanged.
- Regression test `tests/proofdb.rs::stale_db_guard_is_length_safe_on_
  malformed_built_from`: tampered `meta.built_from` with `"deadbeef"`,
  `""`, and a >16-byte non-ASCII string over the fixture DB; asserts the
  `Err` names `built_from`, contains the stale-DB abort text, and prints
  the value in full.
- Malformed-DB probe re-run pre/post: pre-fix panic at `db.rs:50`
  (`end byte index 16 is out of bounds for string of length 8`), exit 134,
  core dump; post-fix `proofdb_flip: DB built_from deadbeef != manifest
  digest e91ad57b44008fee (stale DB or stale manifest — re-merge the
  standing shard set first)`, exit 1, no panic.

### Task 2 — D3: derived-DB rebuild regression test

- `tests/proofdb.rs::standing_layer_rebuilds_byte_identical`: asserts the
  committed shard set is present (clear message if run outside the
  checkout), then re-runs the `proofdb_merge` pipeline verbatim (validate
  → replay-validate → outcome cross-check → startpos replay → graft →
  overlay → finalize → per-graft skeleton re-validation → `write_db`) into
  a temp DB, and asserts sha256 `0d929f4c3c62e4bbb87e9b043b582716297b23e2f
  7332a36e82763b1486070b8`, 55,703 nodes, 55,628 proven. Runtime ≈ 1.2 s,
  inside the `make test` budget.
- Red-on-drift verified per plan: mutated the pinned digest constant →
  test red (4 failure lines), restored → green.

### Task 3 — D2: operator runbook

- `docs/proofdb_pipeline.md` (13.2 KB, docs-exempt from the 10 KB rule):
  pipeline map, clean-checkout quickstart with pinned expected results,
  layer layout + ledger pick-up semantics (G5 decision: ledger stays
  scheduling state; fresh or seed-from-snapshot), full CLI reference for
  the four tools (every option, default, exit code), soundness contract in
  operator terms, known limitations (G6 a–c), history pointer.
- Gate 5: the quickstart was executed verbatim after drafting (fresh
  `/tmp/proofdb_quickstart`, shard set copied, outputs to /tmp). All
  pinned expectations held; two expectation strings were corrected to the
  observed bytes (`root Null` not `root None`; job lines carry
  `"outcome": "censored"`, not a `censored` boolean) — the committed
  runbook reflects the executed outputs.

### Task 4 — D4/D5: AGENTS.md, initiative.md, index

- AGENTS.md: four one-line Examples entries (`proofdb_flip`,
  `proofdb_harvest`, `proofdb_ledger_union`, `proofdb_merge`) + runbook
  pointer; Architecture `src/proof_tree/` bullet extended with the
  proofdb tooling map + `docs/proofdb_pipeline.md` /
  `docs/spec/global_proof_store.md` pointers.
- `initiative.md`: backlog row #8 → done (plan12); new History entry;
  plan12-drafted entry demoted below it.
- `docs/plans/README.md`: proofdb row's current-focus updated (item 8
  closed → items 5/6 next). Closing rule §4 applies: no new M-sized gap
  found, **item 8 closes with plan12**.

## 2. Pinned audit record (re-run 2026-10-02, this session)

| Probe | Result |
|-------|--------|
| Clean-checkout merge (262 shards, 1.3 MB manifest) | `262/262 shards; 55,703 nodes (55,628 proven, 75 open); 55,360 overlay + 866 deduped + 342 ancestors`, 1.2 s |
| Rebuilt DB digest | `0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8` (= standing `data/proofdb.db`) |
| Manifest digest (262 entries) | `e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd` (full value pinned here for the first time) |
| Smoke harvest (`and-close`, base 200k, max-jobs 2) | 2/2 censored (`g1f3 d7d6`, `g1f3 e7e5`), exit 0, manifest rewritten byte-identically, 0 new shards, fresh ledger pick-up |
| Flip over rebuilt DB | `open_rows 75 flips 0 verified 0 root Null (fixpoint 1 rounds)` |
| Ledger union N=1 over plan10 snapshot | 8,446 records |
| Malformed DB (`built_from='deadbeef'`) | pre-fix: panic/exit 134/core dump → post-fix: clean abort, exit 1 |

## 3. Gate results (§3)

1. Post-fix probes: merge digest unchanged (`0d929f4c…`); harvest smoke
   unchanged (2/2 censored, byte-identical manifest); malformed-DB case
   aborts cleanly (exit 1, no panic/core). ✔
2. `make test` green (58 proofdb integration tests + unit tests; includes
   both new tests). ✔
3. `cargo clippy --all-targets` clean (one `unused_mut` in the new test
   fixed), `cargo fmt --check` clean, `cargo doc` clean. ✔
4. No `src/` changes; `git status` shows only `AGENTS.md`,
   `examples/proofdb/db.rs`, `tests/proofdb.rs`,
   `docs/proofdb_pipeline.md` (+ the three docs updates). ✔
5. Runbook quickstart executed verbatim post-drafting. ✔

## 4. Tools used

Plain `cargo`/`make`, `python3` + the `sqlite3` Python module (no `sqlite3`
CLI in the container) to tamper `meta.built_from` for the G1 probe,
`sha256sum`. No perf/`perf` needed; no external crates added.

## 5. Problems encountered

- The in-test merge replication first produced a different DB digest —
  cause: I moved the shard's `root_fen` out (for graft cross-checks)
  before writing the `ShardRow`, whereas the merger stores
  `entry.fen` there. Fixed by mirroring the merger exactly; a useful
  demonstration that the byte-identity gate catches replication drift.
- `cargo fmt` reformatted both new tests after the first write; clippy
  flagged one `unused_mut` (the merger keeps `shard` mutable for
  `root_fen`, the test does not).
- An `edit` to `initiative.md` initially clipped the plan12-drafted
  History bullet's header (matched only its first line); repaired in a
  follow-up edit — final text verified.

## 6. Unresolved parts / missing tests

- None from the plan's gap list (G1–G6 all addressed: G1–G4 fixed,
  G5/G6 recorded).
- Finding (S-sized, for a future plan): `tests/proofdb.rs` is now ~35 KB —
  past the 20 KB split threshold. It is the integration harness that
  `#[path]`-includes the example modules (their inline unit tests only run
  through this target), so splitting is non-trivial (module-level split of
  the include surface, not just of test functions). Not done here: the
  plan explicitly prescribed adding both tests to this file.
- `docs/proofdb_pipeline.md` is a snapshot in time: its pinned digests
  change with every standing-layer advance. The runbook says so; if the
  standing layer grows again, update §2's pinned values (or better: have a
  future plan decide whether the quickstart should pin a manifest-digest
  → expected-DB-digest table).

## 7. Next steps

Per the closing rule, item 8 is closed. The initiative's next levers:

1. **Item 5 — website handoff**: ship `docs/spec/global_proof_store.md`
   plus the runbook to the external website project; the tooling surface
   is now stable and documented.
2. **Item 6 — parallel harvesters**: per-worker frontier partitions +
   `proofdb_ledger_union` merge; report4 finding 3 (work-aware selection
   key) is the known prerequisite for making large batches worth scaling.
3. Optional S-sized hygiene: split `tests/proofdb.rs` (see §6).
