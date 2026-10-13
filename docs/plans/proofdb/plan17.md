# plan17 — crash-consistent manifest: per-job durable manifest sync in the harvest session

**Initiative:** `proofdb` · **Executes:** backlog item 14 (added with this
plan) · **Size:** S (example-side session/manifest change + tests + docs;
**no `src/` changes**) · **Date drafted:** 2026-10-13 (plan session,
docs-only; machinery verified by code read, no probes needed)

> **Owner framing (2026-10-13 design dialogue):** after every finished
> search (= ledger update) the set of shards must be consistent, so that
> unexpected crashes or ctrl-c exits only lose the latest job's work and
> subsequent `proofdb_merge` runs add the collected shards (minus the last
> one) just fine. **No dedicated SIGINT handler.** Verbatim owner intent:
> *"i want that after every finished search (= ledger update) the set of
> shards is consistent. that way unexpected crashes or ctrl-c exits only
> lose the latest batch of work and subsequent `proofdb_merge` runs add
> the collected shards (minus the last one) just fine. i don't need a
> dedicated sigint handler though."*

## 0. State this plan starts from

- Items 5, 8, 9, 10, 13 closed (plan12–16): pipeline pinned, operator+user
  manual at `docs/proofdb_pipeline.md`, production boot self-bootstrapping
  (plan15), the `frontier-walk` policy shipped (plan16/report16).
- **Machinery inventory (verified by code read this session):**
  - **The manifest is batch-end-only today.** `Session::produce_shard`
    (`examples/proofdb/session.rs`) exports, validates, writes the `.bin`
    to the shard dir **immediately per decisive job**, updates the
    `known_tags`/`known_paths` collision sets, and pushes the entry into
    the in-memory `entries: Vec<ShardEntry>`. The on-disk manifest is
    written exactly once, by `Session::rewrite_manifest`, called from the
    single CLI exit path (`proofdb_harvest.rs:300`) after the batch loop.
    **Consequence (the defect this plan closes):** a crash or ctrl-c
    mid-session orphans *every* shard exported in that session, not just
    the in-flight job's — decisive jobs leave no ledger record, so the
    next run re-solves all of them from scratch.
  - **The ledger is already crash-safe per censor** (the target shape):
    `examples/proofdb/ledger.rs` saves via tmp-file + `fs::rename`
    (`.json.tmp`, lines 182–186) after every bump. This is the pattern to
    reuse, not to invent.
  - **`write_manifest` is deterministic but not atomic**
    (`examples/proofdb/manifest.rs::write_manifest`): sorted entries,
    required fields only, fixed field order, digest over the exact bytes —
    but it writes with a plain `std::fs::write`. A crash *during* a write
    would truncate the manifest; irrelevant today (one write per batch),
    load-bearing once writes become per-job.
  - **One choke point, one constructor.** All five policy drivers
    (`batch.rs` screen+heavy, `pns/driver.rs`, `and_close/driver.rs`,
    `descend.rs`, `walk/driver.rs`) funnel every job through
    `Session::run_job_with_budget` → `produce_shard`; `Session::new` has
    exactly one call site (`proofdb_harvest.rs:104`). A session-level
    change therefore covers every policy at once.
  - **Censors dominate sessions**, so per-job manifest rewrites are
    per-*decisive*-job events (censors touch only the ledger). Reference
    boot: 53 decisive among 553 jobs (report16).
- **Digest pins that must not move** (byte-determinism gates): the
  standing fixture rebuild `0d929f4c…` / 55,703 nodes (R1,
  `tests/proofdb.rs::standing_layer_rebuilds_byte_identical`) and the
  plan16 walk-boot manifest digest `8a7644b1…` (report16 §3). Both are
  content functions of the entry set / shard set — untouched by this plan
  if the manifest *format and bytes* are untouched.

## 1. Goal and non-goals

**Goal.** After every finished job of a harvest session (decisive or
censored), the durable layer on disk is **consistent**: every `.bin` the
session has written so far is referenced by the on-disk manifest. An
unexpected crash or ctrl-c then loses at most the in-flight job's work
(and at most the manifest entry of the one job whose manifest write was
interrupted — atomic rename makes each write all-or-nothing). Every
subsequent bare `proofdb_merge` folds in everything collected up to the
last finished job; the retry deterministically overwrites any single
orphan from an interrupted job (existing §10 contract).

**Non-goals.**

- **No SIGINT handler, no signal handling of any kind** (owner decision):
  the stop conditions (`--max-total-evals`, `--max-jobs`, `--max-runtime`,
  `touch STOP`) stay the only graceful exits; this plan only narrows what
  an *unclean* exit costs.
- **No manifest format change.** The v1 required-fields-only JSON (one
  object, sorted, deterministic bytes) is the external contract
  (`docs/spec/global_proof_store.md` reader side, `proofdb_merge` input);
  no journal, no JSONL, no format version bump.
- **No ledger changes** — its per-censor atomic save is already the model.
- **No merge-side or guard changes.** `built_from` semantics, the
  stale-DB abort, and R1 are untouched: the manifest on disk changes
  mid-session exactly as it changes at batch end today, and the next
  session start (and `proofdb_flip`) still requires a merge after any
  decisive job — the *operator discipline* ("merge after a batch") is
  unchanged; only the crash window shrinks.

## 2. Design decisions

- **D1 — sync point: `Session::produce_shard`.** Immediately after
  `self.entries.push(entry.clone())`, the session durably rewrites the
  manifest. This is the single choke point (§0), so every policy gains
  the property without driver changes. The rewrite is **silent**: no new
  stdout/stderr lines (the `job:` record already carries `tag`; the
  output grammar of §7.2 does not change).
- **D2 — `Session` learns its manifest path.** `Session::new` gains a
  `manifest_path: PathBuf` parameter (the one call site at
  `proofdb_harvest.rs:104` passes `args.manifest.clone()`), stored on the
  session; `produce_shard` uses it. The end-of-batch
  `rewrite_manifest(&args.manifest, …)` call keeps its signature (the
  stored path is the same one; no behavior difference).
- **D3 — atomic manifest write.** `write_manifest` switches from
  `std::fs::write` to the ledger's pattern: serialize to bytes → write
  `manifest.json.tmp` (same directory, `path.with_extension("json.tmp")`)
  → `fs::rename` over the target. The digest is computed over the
  serialized bytes as today (rename does not change them). The tmp name
  must not collide with a shard file (`*.bin`) or the ledger's tmp — it
  lives in the shard dir next to `manifest.json`, suffix `.json.tmp`,
  disjoint from both. A leftover `.tmp` from a killed write is never
  read by anyone and is overwritten by the next write; no cleanup needed.
- **D4 — write ordering and the crash window, precisely.** Per decisive
  job: (1) export + validate, (2) write `*.bin` (as today), (3) update
  collision sets + in-memory entries (as today), (4) atomically rewrite
  the manifest. Crash windows: before (2) — nothing durable, pure loss of
  the in-flight job's evals; between (2) and (4) — exactly one unreferenced
  shard (the layer stays mergeable; the retry overwrites it
  deterministically, §10); during (4) — rename keeps the previous
  complete manifest, that job's shard is the one orphan. So the on-disk
  manifest is always a **prefix of the session's decisive sequence**:
  "minus the last one" in the owner's framing, and never worse. A write
  failure (I/O error) at (4) aborts the session (`fail()`) like every
  other durable-layer failure — exit 2, durable layer consistent.
- **D5 — end-of-batch rewrite unchanged.** `rewrite_manifest` still runs
  at batch end and prints the same `manifest: N entries, digest …` /
  `manifest: unchanged (…)` lines. Since `write_manifest` bytes are a
  pure function of the entry set, the end-of-batch rewrite after per-job
  rewrites is an idempotent no-op byte-wise — kept for the stdout grammar
  and for the `new_shards == 0` branch (which still avoids touching the
  file). The redundant I/O is accepted, not optimized: decisive jobs are
  a small minority (53 of 553 in the reference boot) and manifest sizes
  stay small on the production horizon; if it ever matters, a
  "serialize, skip write when bytes equal the last written set" guard is
  a two-line follow-up — deliberately out of scope here.
- **D6 — per-job cost note (accepted).** Each decisive job now pays one
  full-manifest serialize + write (O(entries)). At the standing scale
  (262-entry fixture; walk boots in the hundreds of entries) this is
  microseconds against multi-second jobs; a future session with tens of
  thousands of entries and thousands of decisive jobs would pay
  megabytes-to-gigabytes of rewrite I/O over hours of wall time — noted
  here so the cost model is on record, not metered.
- **D7 — determinism preserved.** Manifest bytes stay a pure function of
  the entry set; no digest pin moves (§0). The walk's byte-determinism
  gate (report16 §3: manifest digest, ledger bytes, shards, merged DB)
  must reproduce unchanged.

## 3. Tasks

1. **`manifest.rs`:** atomic write per D3 (tmp + rename); unit tests: a
   rewrite over an existing manifest is byte-identical (already covered)
   plus: no `.json.tmp` residue after a successful write; an I/O failure
   (e.g. read-only directory via permission) leaves the previous manifest
   file intact and returns the error.
2. **`session.rs`:** `Session::new` gains the manifest path (D2);
   `produce_shard` performs the per-job atomic rewrite after the entry
   push (D1, D4); a failed manifest write aborts the session. File-size
   justification header updated if the module crosses a threshold note
   (it will not at this size).
3. **Tests (fast tier, `tests/proofdb.rs` and/or
   `examples/proofdb/session` tests):**
   - after a *decisive* job (tiny decisive fixture, e.g. the walk tests'
     20k-budget win), the **manifest on disk already contains the new
     entry before `rewrite_manifest` is ever called** — the core new
     invariant;
   - after a *censored* job, the manifest file bytes are unchanged;
   - a session aborted between the `.bin` write and the manifest write
     (simulate by constructing the shard file, then failing the manifest
     write via an unwritable manifest path) exits non-zero with the
     previous manifest intact and exactly one orphan — the documented
     D4 window;
   - the existing byte-determinism and rebuild tests still pass
     untouched (no new expectations).
4. **Docs:** `docs/proofdb_pipeline.md` — §10 "Orphan shards after a
   harvest abort" rewritten to the narrowed window (in-flight job only;
   mid-session crash leaves a mergeable layer; merge after any batch
   still required before the next session); §7.2 output/exit note gains
   the "manifest is durably current after every finished job" sentence;
   §3.2 operational note ("Censor records are saved … abort-safe")
   updated to name the manifest sync too. `AGENTS.md`
   `proofdb_harvest` line: per-job manifest sync (crash-consistent
   durable layer). `docs/plans/proofdb/initiative.md`: item 14 row +
   changelog entry (drafted/executed).
5. **Gate:** `make test` (all unit + fast integration tests, fixture
   rebuild digest pin included). `cargo clippy` / `cargo fmt` clean.
6. **Final task:** write `docs/plans/proofdb/report17.md` (what changed,
   test evidence incl. the pinned digests, problems/deviations, missing
   tests, next steps) and update the `docs/plans/README.md` row only if
   the initiative's status line changes (item 14 added/closed — the
   initiative stays open, item 6 remains the next lever).

## 4. Pre-registered gates

- (a) All new tests pass; the default gate stays green.
- (b) **Digest pins unchanged:** fixture rebuild `0d929f4c…` (55,703
  nodes) byte-identical; if the executor re-runs a walk boot, its
  manifest digest must equal `8a7644b1…` (report16) — the manifest bytes
  are format-frozen.
- (c) No `src/` changes; `git status` carries no `data/` litter (any
  probe runs in `/tmp` layers, per the plan14+ convention).

## 5. Out of scope (recorded follow-ups)

- The "skip redundant rewrite" I/O guard (D5) if manifest scale ever
  demands it.
- Item 6 (parallel harvesters): per-worker durable layers will want the
  same per-job consistency — this plan's mechanism composes (each worker
  owns its own manifest/shard dir), noted there when item 6 is planned.
- A `--manifest-sync` toggle: none — the property is unconditional; a
  toggle would create two durable-layer shapes to test and document for
  no operator need.
