# report17 — crash-consistent manifest: per-job durable manifest sync

Executes `plan17.md` (backlog item 14, the owner's 2026-10-13 crash-
consistency request). **All plan tasks completed; all pre-registered
gates green.** Zero `src/` changes (the product solver is untouched); all
work is example-side plus docs.

## 1. What was built

- **`examples/proofdb/manifest.rs`** — `write_manifest` is now atomic
  (plan17 D3, the ledger's tmp + rename pattern): serialize → write
  `manifest.json.tmp` (same directory, `path.with_extension("json.tmp")`)
  → `fs::rename` over the target. The digest is still computed over the
  exact serialized bytes — **the manifest format and bytes are untouched**
  (D7), only the write mechanism changed. A leftover `.tmp` from a killed
  write is never read by anyone and is overwritten by the next write; no
  cleanup needed.
- **`examples/proofdb/session.rs`** —
  - `Session::new` gains the `manifest_path: PathBuf` parameter (D2; the
    single CLI call site at `proofdb_harvest.rs` passes
    `args.manifest.clone()`), stored on the session.
  - `produce_shard` performs the per-job durable sync (D1/D4)
    immediately after the entry push: export → validate → write `*.bin` →
    collision sets + in-memory entry → **atomic manifest rewrite**. The
    rewrite is silent (no stdout line — the `job:` record already carries
    the tag; the §7.2 output grammar is unchanged). A write failure
    aborts the session via `fail()` (exit 2) like every other
    durable-layer defect.
  - The sync lives in a new `sync_manifest` (`pub(crate)`, error-returning
    so the D4 window is testable in-process; `fail()` itself exits the
    process and cannot be unit-tested). The end-of-batch
    `rewrite_manifest` keeps its signature and stdout lines unchanged
    (D5) — after per-job syncs it is an idempotent no-op byte-wise, kept
    for the stdout grammar and the `new_shards == 0` branch.
  - Module doc records the crash-consistency contract (D4 window
    semantics verbatim: the on-disk manifest is always a prefix of the
    session's decisive sequence).
- **`examples/proofdb_harvest.rs`** — the `Session::new` call site plus
  module-doc update (the unclean-exit cost sentence replacing the old
  "aborts before the manifest rewrite" wording).
- **Call-site churn (mechanical)** — the new `Session::new` parameter
  propagated to all 11 call sites: 1 CLI + 8 in `tests/proofdb.rs` + 2 in
  `examples/proofdb/walk/tests.rs` (the plan counted the CLI site; the
  test sites are the expected mechanical fallout, all passing the same
  `dir.join("shards/manifest.json")` the tests' batch-end rewrites use).
- **Docs** — `docs/proofdb_pipeline.md`: §3.2 operational note now names
  the per-job manifest sync alongside the ledger's atomic censor saves;
  §7.2 `--manifest` row ("rewritten atomically after every decisive job
  and at batch end"), exit-codes paragraph rewritten to the narrowed
  window (at most one orphan; no signal handling; stop conditions remain
  the only graceful exits), and the `manifest:` output-grammar note (the
  batch-end line is the idempotent confirmation, not the first write);
  §10 "Orphan shards after a harvest abort" rewritten (mid-session crash
  leaves a mergeable layer; merge after any decisive batch still
  required); §11 history. `AGENTS.md` `proofdb_harvest` line gains the
  per-job sync sentence.

## 2. Tests (task 3) — all green, fast tier

- **`manifest.rs` unit tests (new):**
  - `write_manifest_leaves_no_tmp_residue` — no `.json.tmp` residue after
    a successful write.
  - `write_manifest_io_failure_leaves_previous_file_intact` — the tmp
    write failure (injected root-proof: a directory occupying the tmp
    path; permission bits are not portable under a root test runner)
    returns the error and leaves the previous manifest byte-identical.
- **`tests/proofdb.rs` (new):**
  - `session_censored_jobs_leave_manifest_bytes_untouched` — a censor-
    only session (5 walk jobs at a 1k budget on the fixture, all
    censored) leaves the manifest file bytes untouched and no `.tmp`
    residue: censors live in the ledger only (D4 corollary).
  - `session_manifest_write_failure_leaves_previous_manifest_and_one_orphan`
    — the documented D4 interrupted-write window: shard `.bin` written,
    entry in memory, manifest write fails → error returned, previous
    manifest intact, exactly one unreferenced orphan `*.bin` (test
    comment documents that the process-exit abort itself is `fail()`,
    untestable in-process; the error path of the sync carries the
    contract).
- **Core invariant asserted in the two existing boot tests** (zero extra
  run cost): `descend_boot_from_empty_layer` and
  `walk_boot_from_empty_layer` now read the manifest from disk *before*
  the batch-end `rewrite_manifest` call and assert it already references
  every shard the batch wrote (`entries.len() == summary.decisive`) plus
  no `.tmp` residue — the plan's "manifest on disk already contains the
  new entry before `rewrite_manifest` is ever called" invariant, proven
  for the whole decisive sequence of a session.
- Existing byte-determinism and rebuild tests untouched and passing; the
  writer's deterministic-bytes test (out-of-order construction, round
  trip, duplicate rejection) is unchanged.

## 3. Pre-registered gates — GREEN

- (a) **Default gate (`make test`)**: all suites green (83 proofdb
  integration tests incl. the new ones; unit tests unchanged).
- (b) **Digest pins**: the standing fixture rebuild test
  (`standing_layer_rebuilds_byte_identical`) passes — `0d929f4c…`,
  55,703 nodes, byte-identical. The manifest *bytes* are format-frozen:
  `write_manifest` serialization is untouched, and the writer test still
  pins rewrite-on-unchanged-set byte identity, so the plan16 walk-boot
  pin `8a7644b1…` cannot move (a full 2G-eval boot re-run was not
  required and not spent).
- (c) **No `src/` changes; no `data/` litter**: `git status` shows only
  the eight intended files (examples/, tests/, docs/, AGENTS.md).

## 4. Problems and deviations

1. **`fail()` is not unit-testable** — the abort path of the manifest
   write failure exits the process, so the plan's "session aborted
   between the `.bin` write and the manifest write … exits non-zero" test
   shape is realized as: the sync extracted into an error-returning
   `sync_manifest` (the same code `produce_shard` fails on), tested
   directly for the window semantics (previous manifest + one orphan).
   The process-exit wiring is the pre-existing `fail()` pattern shared
   with every other durable-layer defect; not separately testable
   without a child-process harness (out of proportion for this change).
2. **I/O-failure injection via a directory at the tmp path**, not
   directory permissions — `cargo test` runs as root in this container,
   where a read-only directory is still writable; the directory-at-tmp
   injector fails the same `fs::write` call portably. Documented in the
   test.
3. The plan's task-1 test list said "a rewrite over an existing manifest
   is byte-identical (already covered)" — confirmed, left as is.
4. No clippy or fmt findings (both clean); `Session::new` now takes 5
   arguments — below the `too_many_arguments` threshold, no lint allow
   needed.

## 5. Missing tests / unresolved parts

- The per-job sync is asserted after a full batch (pre-rewrite read) in
  the boot tests and directly in the sync's error path; a *mid-batch*
  observation (manifest current after job k but not job k+1) would need a
  driver hook or child process — the current evidence (sync inside
  `produce_shard`, the single choke point) makes the per-job property
  structural rather than observed per job.
- No test pins the tmp-name collision freedom against shard files
  (`*.bin`) or the ledger's tmp (`work.json.tmp`) — they live in
  different directories by construction (manifest in the shard dir vs
  ledger in `data/proofdb/`; even co-located, `.json.tmp` vs `.bin`
  never collide). Noted as a design argument, not a test.
- The D6 cost model (per-decisive-job full manifest rewrite, O(entries))
  is accepted and unmetered per plan D5/D6; the "skip redundant rewrite"
  guard stays a recorded follow-up if manifest scale ever demands it.

## 6. Next steps

1. **Item 6 (parallel harvesters)** — the standing next lever; per-worker
   durable layers will want this same per-job consistency, which composes
   (each worker owns its own manifest/shard dir) per plan17 §5.
2. Optional: the `skip redundant rewrite` I/O guard (plan17 §5) if a
   session ever combines tens of thousands of manifest entries with
   thousands of decisive jobs.
