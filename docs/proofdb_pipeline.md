# proofdb pipeline — operator manual

How to run the startpos proof-line database pipeline (shards → merger → DB →
harvest loop) as shipped in this repository. The four tools live under
`examples/` (`proofdb_merge`, `proofdb_harvest`, `proofdb_flip`,
`proofdb_ledger_union`); the DB schema and its semantics are the sole
external contract and are normative in `docs/spec/global_proof_store.md` —
this manual is about *operating* the pipeline, not about the schema.

## 1. Pipeline map

```
durable truth                     derived view                 selection state
─────────────                     ────────────                 ───────────────
docs/plans/proofdb/shards/        proofdb.db (SQLite,          data/proofdb_work.json
  *.bin  (proof trees)              schema v1)                 (work ledger: per-path
  manifest.json (entries +        ▲                              censor history; NOT
   sha256 of the manifest)        │                              part of the DB)
        │                         │
        └── proofdb_merge ────────┘
                    ▲
                    │ (merge after each batch — proofdb_harvest never merges)
        proofdb_harvest ── writes new *.bin shards + manifest entries + ledger
                    │      (fresh roots, child-eval budgets, censored jobs)
        proofdb_flip ──── read-only analysis over the grown DB (implied flips)
        proofdb_ledger_union ── N-way merge of work ledgers (selection state)
```

- **Durable layer**: the validated shard set (`*.bin` binary proof trees)
  plus its manifest. Append-only: harvesting adds shards and manifest
  entries; nothing else ever writes there.
- **Derived view**: one SQLite file (`proofdb.db`, schema v1 per the spec).
  Rebuildable from the shards at any time, deterministically and
  byte-identically (the quickstart below pins this).
- **Selection state**: the work ledger holds *scheduling* state only —
  which frontier paths are known-open, how much work failed at them, how
  many passes. It never enters the DB or the durable layer.

## 2. Clean-checkout quickstart

From a fresh clone, this exercises the whole pipeline in seconds and checks
that your checkout reproduces the standing DB. Outputs go to `/tmp`; the
shard set is copied so the quickstart never writes into the checkout.

```bash
cargo build --release --examples

rm -rf /tmp/proofdb_quickstart && mkdir /tmp/proofdb_quickstart
cd /tmp/proofdb_quickstart
REPO=<absolute path of your atomic_solver checkout>
cp -r "$REPO/docs/plans/proofdb/shards" .

# 1. Merge the standing shard set into a fresh DB.
"$REPO/target/release/examples/proofdb_merge" \
    --manifest shards/manifest.json --shard-dir shards \
    --db proofdb.db --dump nodes.txt

# 2. Pin the rebuild: digest and census must match the standing DB.
sha256sum proofdb.db
```

Expected merge output (pinned 2026-10-02, standing set; see §5 for the
regression test that keeps this true):

```
census: shards merged 262/262; shard tree nodes 56226; overlay new 55360; dedupes 866;
        open-ancestor insertions 342; open nodes upgraded 268; merged nodes 55703
        (proven 55628, open 75); skeleton re-validations 262
node arithmetic: 55703 = 1 root + 55360 overlay + 342 ancestors;
                 shard nodes 56226 = 55360 new + 866 deduped
db: proofdb.db (nodes 55703), manifest built_from e91ad57b44008fee
```

`sha256sum proofdb.db` must print
`0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8` — the
digest of the standing `data/proofdb.db`. Any other digest means your shard
set or toolchain diverges from the pinned state.

```bash
# 3. Smoke harvest: 2 jobs, tiny budget — both are expected to censor.
"$REPO/target/release/examples/proofdb_harvest" \
    --db proofdb.db --manifest shards/manifest.json --shard-dir shards \
    --policy and-close --budget-evals 200000 --max-jobs 2 \
    --ledger work.json

# 4. Flip analysis over the (unchanged) DB.
"$REPO/target/release/examples/proofdb_flip" \
    --db proofdb.db --manifest shards/manifest.json --out flips.json
```

Expected smoke-harvest output: two `job: {…}` lines with `"outcome":
"censored"` and `"kind": null`, then
`harvest: policy and-close stop=max-jobs jobs 2 decisive 0 censored 2 … new_shards 0`,
exit 0; `shards/manifest.json` is rewritten **byte-identically** (digest
`e91ad57b44008fee…` unchanged — censored jobs write no facts, hence no
shards and no manifest change). `work.json` now exists (the fresh ledger
pick-up semantics, §3). Expected flip output:
`flip_analysis: open_rows 75 flips 0 verified 0 root Null (fixpoint 1 rounds)`
— the standing set has no implied flips and the root is undecided.

```bash
# 5. Optional: normalize the committed ledger snapshot (N = 1).
"$REPO/target/release/examples/proofdb_ledger_union" \
    --out work_union.json \
    "$REPO/docs/plans/proofdb/measurements/plan10/ledger_union.json"
```

Expected: `ledger_union: work_union.json -> 8446 records`.

## 3. Layer layout and ledger pick-up

| Path | Layer | Committed? |
|------|-------|------------|
| `docs/plans/proofdb/shards/*.bin` + `manifest.json` | durable truth | yes |
| `data/proofdb.db` | derived view | no (`.gitignore`d) |
| `data/proofdb_work.json` | work ledger | no (`.gitignore`d) |

**Ledger pick-up semantics:** a *missing* ledger file loads as an empty
(fresh) ledger — a new working directory starts a harvest from scratch
without any setup. Two ways to come back to the standing state:

1. **Fresh** (sound by construction): every frontier reply is re-censored
   at the base budget before the ladder resumes; lower yield on the first
   revisit, no other effect. This is what the quickstart does.
2. **Seed from the committed snapshot**: copy
   `docs/plans/proofdb/measurements/plan10/ledger_union.json` (8,446
   records) to `data/proofdb_work.json`, or pass it via `--ledger`. Use
   `proofdb_ledger_union` to merge several ledger inputs (e.g. per-worker
   ledgers, or a snapshot plus newer work) into one.

The ledger is deliberately **not** part of the durable layer: it is
scheduling state, and its durable residue is the committed snapshots under
`docs/plans/proofdb/measurements/`.

## 4. CLI reference

### `proofdb_merge` — shards → SQLite (single writer)

    usage: proofdb_merge --manifest <index.json> --shard-dir <dir>
                         [--db <out.db>] [--dump <nodes.txt>] [--sample-lines <n>]

| Option | Default | Meaning |
|--------|---------|---------|
| `--manifest` | required | manifest JSON (entries sorted by tag internally) |
| `--shard-dir` | required | directory holding the `*.bin` shard files |
| `--db` | `proofdb.db` | output SQLite path (overwritten) |
| `--dump` | off | canonical text dump of the merged tree |
| `--sample-lines` | `0` | print `n` random root-to-leaf walks as spot-checks |

Exit codes: `0` merged and written; `1` usage error; `2` CONFLICT/DEFECT
abort (non-`ok` manifest entry, unreadable/unparseable shard, replay
validation defects, manifest/tree cross-check mismatch, hash-fidelity
failure, proven-outcome conflict, depth-fixpoint error, skeleton
re-validation failure, DB write failure). An abort writes nothing —
conflicts are never patched.

### `proofdb_harvest` — grow the shard set

    usage: proofdb_harvest --db <proofdb.db> --manifest <manifest.json>
                           --shard-dir <dir> [--policy <name>] [--ledger <path>]
                           [--budget-evals <n>] [--max-total-evals <n>]
                           [--heavy-budget-evals <n>] [--heavy-sample <n>]
                           [--tt-mb <mb>] [--max-jobs <n>] [--max-runtime <s>]
                           [--stop-file <path>] [--pns-config <file>]
                           [--and-close-order <completion|fresh>]
                           [--and-close-max-budget <evals>]
                           [--out-db <grown.db>] [--dump <nodes.txt>]

| Option | Default | Meaning |
|--------|---------|---------|
| `--db` | required | the merged DB to work against (read-only) |
| `--manifest` | required | manifest to extend (rewritten after the batch) |
| `--shard-dir` | required | where new shard files are written |
| `--policy` | `breadth-pns` | `breadth-pns` \| `and-close` \| `open-deepest` \| `sharp-siblings` \| `sharp-heavy-tail` |
| `--ledger` | `data/proofdb_work.json` | pick-up state; a missing file is a fresh ledger |
| `--budget-evals` | policy default | base child-eval budget per job (breadth-pns base: 4,000,000); screens for the legacy policies |
| `--max-total-evals` | `0` (unlimited) | session cap across jobs |
| `--heavy-budget-evals` | `40,000,000` | heavy-job budget (`sharp-heavy-tail`) |
| `--heavy-sample` | `5` | heavy jobs per screen pass (`sharp-heavy-tail`) |
| `--tt-mb` | `128` | per-session transposition table (MB) |
| `--max-jobs` | `0` (unlimited) | stop after n jobs |
| `--max-runtime` | `0` (unlimited) | stop after n seconds |
| `--stop-file` | `STOP` | batch stops (gracefully, after the in-flight job) when this file appears |
| `--pns-config` | compiled defaults | breadth-pns mechanism knobs (TOML); effective only under `breadth-pns` |
| `--and-close-order` | `completion` | `completion` \| `fresh` (effective only under `and-close`) |
| `--and-close-max-budget` | `0` (unlimited) | per-job budget cap under `and-close` |
| `--out-db` / `--dump` | off | recorded in the summary only — **the caller runs `proofdb_merge` separately** (by design; this CLI never merges) |

Exit codes: `0` batch completed (then the manifest is rewritten); `1` usage
error; `2` ABORT on any defect — the session aborts *before* the manifest
rewrite, so the durable layer never becomes inconsistent. Shards of jobs
already completed when an abort hits remain as unreferenced orphan files
and are deterministically overwritten by the retry. Stop reasons in the
summary: `max-jobs`, `max-total-evals`, `max-runtime`, `stop-file`,
`exhausted`.

Every job prints one `job: {…}` JSON line; a **censored** job (draw from
any cause, budget exhausted) records a fact of absence only — no shard, no
manifest entry, a ledger bump.

### `proofdb_flip` — implied-flip analysis (read-only)

    usage: proofdb_flip --db <grown.db> --manifest <manifest.json>
                        --out <flip_analysis.json>

For every open row whose legal replies are all resolved (stored or
implied), computes the implied outcome + bound, iterated to fixpoint; every
reported flip is then recomputed by an independent recursive verifier.
`--out` writes the JSON analysis; `--db`/`--manifest` are required. Exit
`0` normally (also with 0 flips), `1` on usage error, DB load failure
(including the stale-DB `built_from` guard), or any unverified flip.

### `proofdb_ledger_union` — N-way ledger merge

    usage: proofdb_ledger_union --out <ledger.json>
                                [--expect <file>=<sha256>]... <input.json>...

Keeps, per path, the record with the highest `passes_failed` (ties: higher
`work_done`). `N ≥ 1` inputs; `N = 1` normalizes (deterministic rewrite).
`--expect` pins an input's sha256 digest (repeatable; verified before the
merge). Output bytes are deterministic (sorted); a per-input census goes to
stderr. Exit `0` merged, `1` on usage/load/digest-mismatch errors.

## 5. Soundness contract (operator terms)

- Only shards whose manifest entry says `validate: "ok"` enter a merge, and
  each is replay-validated again at merge time and once more as a
  re-assembled skeleton after the depth fixpoint.
- Proven-outcome conflicts between shards abort the merge (exit 2). They
  are soundness bugs, never patched over.
- The DB is a derived view: delete it and rebuild with `proofdb_merge`
  anytime — same shard set, byte-identical result. The rebuild is a
  standing regression test (`tests/proofdb.rs::
  standing_layer_rebuilds_byte_identical`) and the quickstart's `sha256sum`
  step.
- A DB must carry `meta.built_from` = the sha256 of the manifest it was
  built from; `proofdb_harvest`/`proofdb_flip` refuse a DB whose digest
  differs (stale DB or stale manifest — re-merge first).
- Censored (draw) results are facts of absence only: no shard, no DB row,
  a ledger record. Non-terminal draws are not representable as facts in
  this tree model.

## 6. Known limitations

- **Startpos-rooted only.** Subtree-scoped harvesting (`--root-fen`) is
  parked; every job replays from the standard start position.
- **Orphan shards after an abort.** A batch aborted mid-way leaves already
  written shard files unreferenced (the manifest rewrite is skipped on
  abort); the retry deterministically overwrites them.
- **Merge is a separate step.** `proofdb_harvest --out-db/--dump` only
  record the intended targets in the summary; after each batch, run
  `proofdb_merge` yourself (quickstart step 1, then the flip analysis).

## 7. History

The pipeline was built and hardened by the `proofdb` initiative
(`docs/plans/proofdb/`); the tooling-completeness audit that produced this
manual is plan12 (2026-10-02). Per-plan measurements and the pinned audit
results live under `docs/plans/proofdb/measurements/`.
