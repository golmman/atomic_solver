# proofdb pipeline — operator and user manual

How to run, consume, and extend the startpos proof-line database pipeline
(shards → merger → DB → harvest loop) as shipped in this repository. The four
tools live under `examples/` (`proofdb_merge`, `proofdb_harvest`,
`proofdb_flip`, `proofdb_ledger_union`); the DB schema and its semantics are
the sole external contract and are normative in
[`docs/spec/global_proof_store.md`](spec/global_proof_store.md). This manual
serves two readers:

- the **operator**, who runs the pipeline (merge, harvest, flip, ledger
  maintenance) — §1–§3, §6, §8, §9;
- the **user/consumer** (e.g. the website project), who reads the finished
  SQLite file and wants to understand its shape and query it — §4, §5, §7
  (plus the normative spec).

## Contents

1. [Pipeline map](#1-pipeline-map)
2. [Clean-checkout quickstart](#2-clean-checkout-quickstart)
3. [Layer layout and ledger pick-up](#3-layer-layout-and-ledger-pick-up)
4. [Data model primer (consumer)](#4-data-model-primer-consumer)
5. [Shard provenance — where a shard comes from](#5-shard-provenance--where-a-shard-comes-from)
6. [CLI reference (operator)](#6-cli-reference-operator)
   - [6.1 `proofdb_merge`](#61-proofdb_merge--shards--sqlite-single-writer)
   - [6.2 `proofdb_harvest`](#62-proofdb_harvest--grow-the-shard-set)
   - [6.3 `proofdb_flip`](#63-proofdb_flip--implied-flip-analysis-read-only)
   - [6.4 `proofdb_ledger_union`](#64-proofdb_ledger_union--n-way-ledger-merge)
7. [Recipes](#7-recipes)
   - [7.1 Operator recipes](#71-operator-recipes)
   - [7.2 Consumer recipes (SQL)](#72-consumer-recipes-sql)
8. [Soundness contract (operator terms)](#8-soundness-contract-operator-terms)
9. [Troubleshooting and known limitations](#9-troubleshooting-and-known-limitations)
10. [History](#10-history)

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

Expected merge output (pinned 2026-10-02, standing set; see §8 for the
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
"censored"` and the `"pass"`/`"work_before"` census fields, then
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

## 4. Data model primer (consumer)

This section is consumer-level prose. The normative contract is the spec
(`docs/spec/global_proof_store.md`) — schema §2, field semantics §3,
invariants §4. Everything below can be verified by SQL against the DB alone.

**Node identity is the move path.** Every row of `nodes` is a position
reached by a specific move sequence (UCI, space-separated along the
`parent_id` chain) from `meta.root_fen` — the standard start position. The
same position reached by two different routes is **two different rows**:
halfmove-clock-dependent repetition verdicts are part of what a path fixes,
and facts are never transferred across paths (spec invariant 6). Row `id`s
are assigned in lexicographic path order (spec invariant 7), so the root is
`id = 0` and a prefix path always sorts before its extensions.

**Partial AND/OR tree.** `outcome` is from the *side-to-move* perspective:

| `outcome` | role | what the tree stores below it |
| --- | --- | --- |
| `'win'` | OR-node | exactly one proving child (a `'loss'` child) — the refutation of the opponent's best try; the other replies are *not* rows |
| `'loss'` | AND-node | every legal, non-terminal reply as a child with `outcome = 'win'` (terminal replies are absent — terminality is derivable) |
| `NULL` | open | undecided; may still have stored children (resolved replies, or an open ancestor chain over a grafted subtree) |

Two consequences a consumer must not get wrong:

- **The tree is partial, and absence is not evidence.** A missing child row
  means "no proven fact recorded for that reply (yet)". It does *not* mean
  the move is illegal, decided, or refuted. The root, for example, has 7
  stored children (its two knight moves and the five most informative pawn
  pushes, all still open); the other ~34 legal replies simply have no rows.
- **Open is open.** `outcome IS NULL` covers both "searched and no decisive
  result within some bounded budget" and "not yet searched" — the DB does
  not distinguish the two (spec invariant 5). A non-terminal draw is also
  represented as `NULL`; no `draw` outcome exists in v1.

**Depth vocabulary: bound by default, exactness an annotation.** A proven
node's `depth_bound` proves *the decisive outcome within ≤ depth_bound
plies from this node*. `depth_status = 'bound'` is the default (the true
distance may be smaller); `'exact'` asserts the bound is the true
distance-to-terminal (spec invariant 4's minimality claim). Terminality is
derivable, never stored as a flag: `outcome IS NOT NULL AND depth_bound = 0`
is a rule-terminal node (its position statically yields its outcome).

**Provenance.** Each row's `provenance` lists (sorted, unique,
comma-separated) the shard tags that contributed any fact at or below it.
The root row aggregates essentially every shard; a deep fact row usually
names exactly one.

**Replay-verifiability.** For every row, replaying its `move_uci` chain
down the `parent_id` links from `meta.root_fen` on a rules-correct
atomic-chess implementation reproduces exactly the row's position (Zobrist
hash, halfmove clock included). A consumer that replays paths can therefore
trust every row's position — and conversely, a row whose replay fails is a
defect, not a feature to work around.

**The `meta` table.** `schema_version=1`, `root_fen`, `node_count`
(rows in `nodes`), `built_from` (the SHA-256 of the manifest the DB was
built from — identifies which shard set the DB reflects), `generator`
(free-form producer identification). Consumers should surface `built_from`
as a version stamp of the data they display.

## 5. Shard provenance — where a shard comes from

A **shard** is one validated binary proof tree (`proof_tree_dump.md` v1
format) rooted at a job position, plus its manifest entry. Shards are the
durable truth; the DB is derived from them. This section explains how a
shard comes to exist and how to inspect one — useful to an operator auditing
the durable layer, and to a consumer who wants to know what `provenance`
tags refer to.

**Production path (all inside `proofdb_harvest`):**

1. The policy selects a frontier path from the merged DB + work ledger
   (§6.2).
2. The job replays that path from the startpos, then runs a bounded
   solver session on the reached position (fresh TT per job, deterministic
   child-eval budget).
3. **Decisive result** → the solver's proof-tree reconstruction is exported
   as `*.bin`, replay-validated, and appended to the manifest; the census
   `job:` line carries the `tag` and `shard_nodes`.
   **Censored result** (draw from any cause, budget exhausted) → no shard,
   no manifest entry, a ledger record only.
4. After the batch, run `proofdb_merge` (§7.1, recipe R2): every shard is
   replay-validated *again* at merge time, grafted at its manifest path,
   and only then becomes DB rows.

**Manifest entry anatomy** (one standing-set entry, verbatim):

```json
{"fen": "1nbqkbnr/1ppppppp/8/8/8/2PPP3/5PPP/RNBQKBNR w KQk - 0 6",
 "file": "h_013861d97dce5d48.bin", "moves": ["a2a3", "a7a6", "b2b3", "a6a5",
 "c2c3", "a5a4", "d2d3", "a4b3", "e2e3", "a8a3"], "outcome": "win",
 "tag": "h_013861d97dce5d48", "validate": "ok"}
```

`moves` is the UCI path from the startpos to the shard root; `fen` is the
replayed shard-root position (the merger cross-checks it against the
replay); `outcome` is what the shard proves; `validate: "ok"` is the
validator verdict a shard needs to enter any merge. Tag conventions:
`h_*` harvest facts, `p2_*` the plan2 batch, `lad_*` ladder lines,
`sib_*` sibling refutations — provenance only, no semantic weight.

**Inspecting a shard read-only** (`examples/inspect_pt`, `pt_keys` — not
part of the merge/harvest loop):

```text
$ inspect_pt shards/h_013861d97dce5d48.bin
nodes: 68
root outcome: Win depth: 5
root children:
  d1h5 outcome=Loss depth=4 children=15
extract_ppv: d1h5 g7g6 h5d5 f8g7 d5f7
validate_ppv: true
```

`pt_keys` dumps each node's replayed Zobrist key, outcome, and depth — the
same replay the merger performs:

```text
$ pt_keys shards/h_013861d97dce5d48.bin | head -4
0 -1 540235772156762578 win 5 0
1 0 14893128882772848674 loss 4 231
66 1 13321113938175166713 win 1 3177
67 66 15716123331662465803 loss 0 2549
```

## 6. CLI reference (operator)

Every tool: `1` = usage error, `2` = defect abort where applicable, `0` =
success. Options are shown with their defaults; "required" options have no
default.

### 6.1 `proofdb_merge` — shards → SQLite (single writer)

**Purpose.** Build (or rebuild) the derived DB from a manifest + shard dir.
The only writer of the DB; deterministic and byte-identical for an
identical shard set. Never merges anything else; never patches conflicts.

**Prerequisites.** A manifest whose entries all say `validate: "ok"`, and
the shard files they name, readable from `--shard-dir`. No DB is needed —
`--db` is *overwritten*.

    usage: proofdb_merge --manifest <index.json> --shard-dir <dir>
                         [--db <out.db>] [--dump <nodes.txt>] [--sample-lines <n>]

| Option | Default | Meaning |
|--------|---------|---------|
| `--manifest` | required | manifest JSON (entries sorted by tag internally) |
| `--shard-dir` | required | directory holding the `*.bin` shard files |
| `--db` | `proofdb.db` | output SQLite path (overwritten) |
| `--dump` | off | canonical text dump of the merged tree |
| `--sample-lines` | `0` | print `n` random root-to-leaf walks as spot-checks |

**Exit codes.** `0` merged and written; `1` usage error; `2`
CONFLICT/DEFECT abort (non-`ok` manifest entry, unreadable/unparseable
shard, replay validation defects, manifest/tree cross-check mismatch,
hash-fidelity failure, proven-outcome conflict, depth-fixpoint error,
skeleton re-validation failure, DB write failure). An abort writes nothing —
conflicts are never patched.

**Output grammar (stdout, three lines + optional samples).**

```text
census: shards merged 262/262; shard tree nodes 56226; overlay new 55360; dedupes 866;
        open-ancestor insertions 342; open nodes upgraded 268; merged nodes 55703
        (proven 55628, open 75); skeleton re-validations 262
node arithmetic: 55703 = 1 root + 55360 overlay + 342 ancestors;
                 shard nodes 56226 = 55360 new + 866 deduped
db: proofdb.db (nodes 55703), manifest built_from e91ad57b44008fee
```

- `census:` — shard set coverage (merged/total), raw shard-tree nodes,
  how many became new overlay rows, how many were deduped against already
  stored facts, how many open ancestor rows were inserted to graft, how
  many open rows a graft upgraded to proven, and the final merged counts
  (proven = win+loss, open = `outcome IS NULL`), plus the per-graft
  skeleton re-validation count (one per shard, 262/262 must be clean).
- `node arithmetic:` — the bookkeeping identity restated; if the two sides
  of either equation disagree, the merge is defective.
- `db:` — where the DB went, its node count, and the 16-hex prefix of
  `meta.built_from` (the manifest digest; the full digest is in the DB).
- with `--sample-lines n` (printed *before* the census): `n` lines like
  `sample 1: e2e4 h7h6 d1h5 e7e5 h5f7 (5 plies, end: loss bound 0 (exact))`
  — random root-to-leaf walks as spot-checks.

**Worked invocation.** Quickstart §2 steps 1–2 (pinned standing set,
executed 2026-10-02; rerun in every plan session — the outputs above are
verbatim).

### 6.2 `proofdb_harvest` — grow the shard set

**Purpose.** Select frontier jobs, solve them under deterministic
child-eval budgets, and append validated shards + manifest entries for the
decisive ones. Censored jobs (no decisive result, budget exhausted) record
a fact of absence in the work ledger only.

**Prerequisites.** A merged DB whose `meta.built_from` equals the
manifest's digest (stale pair → clean abort); a writable shard dir; a
ledger path (missing file = fresh ledger, §3).

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

**Exit codes.** `0` batch completed (then the manifest is rewritten); `1`
usage error; `2` ABORT on any defect — the session aborts *before* the
manifest rewrite, so the durable layer never becomes inconsistent. Shards
of jobs already completed when an abort hits remain as unreferenced orphan
files and are deterministically overwritten by the retry. Stop reasons in
the summary: `max-jobs`, `max-total-evals`, `max-runtime`, `stop-file`,
`exhausted`.

**Output grammar.**

*stderr, session start* (informational; the numbers are the DB's frontier
shape and the policy's selected job set):

```text
harvest: db proofdb.db (55703 nodes, built_from e91ad57b44008fee) — policy and-close over frontier C1 75 / C2 951429 / C3 1724 (AND-checks 27860)
and-close: active rows 49, replies 1077 (fresh 1077, ledger-censored 0); order completion; base budget 200000
and-close-gradient: g1f3:3 e2e3:7 e2e4:7 a2a3 a7a6 …:11 root:13 …:17 d2d4:18
and-close-excluded: open rows 75 (active 49); proven-ancestor 26, implied-win 0, implied-loss 0
```

- the `harvest: db …` line: DB path, node count, `built_from` prefix, then
  the frontier classes extracted from the DB — **C1** open rows, **C2**
  unexpanded siblings of proving children, **C3** unexpanded children of
  open nodes, and the AND-completeness check count over proven loss nodes.
- the policy line: for `and-close`, the active (budget-eligible) rows and
  total fresh replies, the ordering, and the base budget. Under
  `--and-close-max-budget`, an additional `and-close-filter: jobs N of M`
  echo appears.
- `and-close-excluded:` — open rows excluded from selection, broken down
  (proven-ancestor / implied-win / implied-loss).

*stdout, per job* — one `job: {…}` JSON line per visit. Field table (as
emitted by `examples/proofdb/session/record.rs`; all jobs carry the first
twelve; `pass`/`work_before` are emitted by the PNS and `and-close`
policies, `number`/`kind` only by `breadth-pns` and omitted otherwise):

| field | meaning |
| --- | --- |
| `path` | UCI path of the job position (space-separated) |
| `policy` | the `--policy` used |
| `class` | frontier class of the job (`C1`/`C2`/`C3`; `and-close` jobs are `C3`) |
| `parent_bound` | the parent node's proven `depth_bound`, if proven |
| `tier` | job tier (`screen`/`heavy`/policy name — `and-close` labels all jobs `and-close`) |
| `budget` | child-eval budget granted for this visit |
| `child_evals` | child-evals actually spent |
| `wall_s` | job wall time (seconds) |
| `outcome` | `"win"` / `"loss"` (decisive, shard exported) or `"censored"` (no fact) |
| `exit_reason` | solver exit: `Complete` (decisive) or `BudgetExhausted` (censored) |
| `tag` | shard tag if decisive (manifest tag; matches `provenance` in the DB) |
| `shard_nodes` | exported shard's node count if decisive |
| `pass` | visit rung: `passes_failed + 1` (PNS and `and-close` census) |
| `number` | PNS effective number at selection time (PNS only) |
| `work_before` | ledger `work_done` recorded before this visit (PNS and `and-close`) |
| `kind` | PNS visit kind (`expand` \| `rung`) (PNS only) |

A **censored** job (this session, quickstart step 3, verbatim):

```text
job: {"budget":200000,"child_evals":200024,"class":"C3","exit_reason":"BudgetExhausted","outcome":"censored","parent_bound":null,"pass":1,"path":"g1f3 d7d6","policy":"and-close","shard_nodes":null,"tag":null,"tier":"and-close","wall_s":0.039135498,"work_before":0}
```

A **decisive** job (recorded in the plan10 census,
`measurements/plan10/census_armA.json`, pinned; the 10-ply
`a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 d2d3 a4b3 e2e3 g7g6` win):

```text
job: {"budget":24000162,"child_evals":8513692,"class":"C3","exit_reason":"Complete","outcome":"win","parent_bound":null,"pass":3,"path":"a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 d2d3 a4b3 e2e3 g7g6","policy":"and-close","shard_nodes":1394,"tag":"h_82e976f53648002b","tier":"and-close","wall_s":1.786593106,"work_before":12000081}
```

*stdout, after the last job* — the manifest line (printed only when the
manifest actually changed, else `manifest: unchanged (…)`), then the
summary line; `out-db:`/`dump:` echo lines follow when those options were
given:

```text
manifest: unchanged (262 entries, digest e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd)
harvest: policy and-close stop=max-jobs jobs 2 decisive 0 censored 2 evals 400037 new_shards 0 wall 3.2s
```

- `manifest:` — entries + full manifest digest after the rewrite
  (byte-identical when no decisive job occurred).
- `harvest:` — `stop=` reason; then per policy family: PNS
  (`pns_jobs N rungs M decisive D evals E new_shards S wall Ws`), and-close
  (`jobs N decisive D censored C evals E new_shards S wall Ws`), legacy
  screen/heavy policies (`screen_jobs/screen_decisive/screen_evals
  heavy_jobs/heavy_decisive new_shards wall`). `decisive` counts jobs that
  exported a shard; `censored` counts facts of absence.

*stderr, during jobs* — `[bounded_search] chunk done: …` progress lines
(one per budget chunk); informational, safe to ignore in scripts.

**Worked invocation.** Quickstart §2 step 3 (2 censored jobs at 200k
evals, manifest byte-identical, exit 0 — outputs above).

### 6.3 `proofdb_flip` — implied-flip analysis (read-only)

**Purpose.** Sanity check over a grown DB: for every open row whose legal
replies are all resolved (stored or implied), compute the implied outcome +
bound, iterated to fixpoint; every reported flip is then recomputed by an
independent recursive verifier. Reads the DB; writes only `--out`.

**Prerequisites.** A merged DB whose `built_from` matches the manifest
digest (the stale-DB guard, §9).

    usage: proofdb_flip --db <grown.db> --manifest <manifest.json>
                        --out <flip_analysis.json>

**Exit codes.** `0` normally (also with 0 flips), `1` on usage error, DB
load failure (including the stale-DB `built_from` guard), or any unverified
flip.

**Output grammar.** One stdout line plus the JSON file:

```text
flip_analysis: open_rows 75 flips 0 verified 0 root Null (fixpoint 1 rounds)
```

- `open_rows` — undecided rows in the DB; `flips` — rows whose implied
  outcome contradicts their stored outcome (0 in a sound DB); `verified` —
  flips confirmed by the independent verifier; `root Null`/`root <bound>` —
  the implied startpos verdict so far (`Null` = undecided, i.e. no handover
  trigger); `fixpoint N rounds` — iterations until stable.
- `--out` JSON shape (standing set, verbatim):

```json
{
 "flips": [],
 "flips_total": 0,
 "flips_verified_independently": 0,
 "open_rows": 75,
 "root_bound": null,
 "root_implied": null
}
```

**Worked invocation.** Quickstart §2 step 4 (0 flips over the standing
set, exit 0).

### 6.4 `proofdb_ledger_union` — N-way ledger merge

**Purpose.** Merge work ledgers (per-worker harvest state) into one.
Keeps, per path, the record with the highest `passes_failed` (ties: higher
`work_done`). `N ≥ 1` inputs; `N = 1` normalizes (deterministic rewrite).

**Prerequisites.** Ledger JSONs from `proofdb_harvest --ledger` runs (or
committed snapshots).

    usage: proofdb_ledger_union --out <ledger.json>
                                [--expect <file>=<sha256>]... <input.json>...

| Option | Default | Meaning |
|--------|---------|---------|
| `--out` | required | merged output ledger (deterministic, sorted bytes) |
| `--expect` | none | pin an input's sha256 digest (repeatable; verified before the merge) |
| `<input.json>…` | required | 1..N ledger files |

**Exit codes.** `0` merged, `1` on usage/load/digest-mismatch errors.

**Output grammar.** One stderr census line per input, then the stdout
summary:

```text
union: input ledger_union.json (8446 records): new_paths 8446, sole_pass_upgrades 0
union: 1 inputs -> 8446 records
ledger_union: work_union.json -> 8446 records
```

- per input: records, how many paths it contributed that no earlier input
  had, and how many of its records strictly upgraded an earlier input's
  pass count; final: total inputs → merged record count; last line names
  the output file and the record count.

**Worked invocation.** Quickstart §2 step 5 (N = 1 normalization of the
committed plan10 snapshot → 8,446 records).

## 7. Recipes

### 7.1 Operator recipes

**R1 — rebuild the DB from the durable layer.** Quickstart §2 steps 1–2:
`proofdb_merge --manifest … --shard-dir … --db proofdb.db`, then
`sha256sum proofdb.db` and compare with the previous DB's digest. A
rebuild is *always* allowed; it is also the response to a stale-DB abort
(§9). Byte-identical output for an identical manifest is the standing
regression test.

**R2 — grow the DB by one batch, then merge.**

```bash
# 1. Harvest (e.g. a small and-close batch; stop conditions as needed).
proofdb_harvest --db proofdb.db --manifest shards/manifest.json \
    --shard-dir shards --policy and-close --budget-evals 4000000 \
    --max-total-evals 10000000000 --ledger data/proofdb_work.json
# 2. Merge the grown set (the harvest never merges).
proofdb_merge --manifest shards/manifest.json --shard-dir shards \
    --db proofdb.db --dump nodes.txt
# 3. Flip sanity over the grown DB.
proofdb_flip --db proofdb.db --manifest shards/manifest.json --out flips.json
```

Note: after a *decisive* batch the manifest digest changes, so the new
DB's `built_from` — and therefore its byte content — differs from the old
DB. Byte-identity is guaranteed per manifest, not across growth steps.

**R3 — check for implied flips.** Recipe R2 step 3; expected on a sound
tree: `flips 0 verified 0`. Any flip is a soundness signal — stop and
investigate (§8), never re-merge to make it disappear.

**R4 — normalize or merge work ledgers.**

```bash
# N = 1: canonicalize a ledger (deterministic rewrite).
proofdb_ledger_union --out ledger.norm.json ledger.json
# N > 1: merge per-worker ledgers, digest-pinned.
proofdb_ledger_union --out merged.json \
    --expect w1.json=9c060103… --expect w2.json=4e14f46f… w1.json w2.json
```

Union is a conservative max-rule merge (`passes_failed` max, tie by
`work_done`) — it never invents facts; it only preserves the strongest
recorded censor history per path.

**R5 — retire an abandoned working directory.** The DB and ledger are
derived/selection state: delete `data/proofdb.db` and the ledger, re-merge
(R1), and either start with a fresh ledger or seed it from a committed
snapshot (§3). Nothing in the durable layer is touched.

### 7.2 Consumer recipes (SQL)

All queries below run read-only and are verified against the standing DB
(node counts in comments are its census: 55,703 nodes = 55,628 proven +
75 open). The DB has only two tables, `meta` and `nodes` (plus `shards`
and the `idx_nodes_parent` index); everything a website needs is a `WITH
RECURSIVE` walk away.

**Path resolution — find the row for a UCI line.** There is no path column;
walk the line from the root:

```sql
-- resolves 'e2e4 h7h6 d1h5 e7e5 h5f7'; NULL if any move has no row
WITH RECURSIVE walk(nid, i) AS (
  SELECT 0, 0
  UNION ALL
  SELECT n.id, w.i + 1
  FROM walk w JOIN nodes n ON n.parent_id = w.nid
  WHERE i < 5
    AND n.move_uci = (SELECT split_index)          -- see note below
)
SELECT nid FROM walk WHERE i = 5;
```

SQLite has no `split_index`; in practice resolve the line in application
code (a loop of `SELECT id FROM nodes WHERE parent_id = ? AND move_uci = ?`
per move) — one indexed lookup per ply, no recursion needed. The reverse
— reconstructing a row's full path — is pure SQL:

```sql
-- full UCI path of node :nid, e.g. 55416 → 'e2e4 h7h6 d1h5 e7e5 h5f7'
WITH RECURSIVE up(nid) AS (
  SELECT :nid
  UNION ALL
  SELECT n.parent_id FROM nodes n JOIN up ON n.id = up.nid
)
SELECT group_concat(move_uci, ' ')
FROM (SELECT move_uci FROM nodes WHERE id IN up AND move_uci IS NOT NULL
      ORDER BY ply);
-- → 'e2e4 h7h6 d1h5 e7e5 h5f7'
```

**Children of a node** (facts stored below it; absence ≠ decided — §4):

```sql
SELECT move_uci, outcome, depth_bound, depth_status
FROM nodes WHERE parent_id = :nid ORDER BY id;
-- node 55416 (terminal loss): no rows — a terminal has no children
```

**Replay/line membership.** "Is this line proven, and how?" = resolve the
path (loop above) and read the resolved node:

```sql
SELECT ply, outcome, depth_bound, depth_status, provenance
FROM nodes WHERE id = :nid;
-- → 5 | loss | 0 | exact | p2_e2e4_h7h6   (verified against the standing DB)
```

If the line resolves to a proven node, every prefix is a stored row on the
way (the path *is* the membership proof). A line that dead-ends at a row
with `outcome IS NULL` is undecided at that point; a line that fails to
resolve leaves the stored tree.

**The open frontier** (what harvesting would open next; 75 rows here):

```sql
SELECT id, ply, group_concat(move_uci, ' ') OVER ()  -- or path via the recipe above
FROM nodes WHERE outcome IS NULL ORDER BY ply, id;
-- standing DB: 1 at ply 0 (the root), 7 at ply 1, then reply rows
-- down to ply 31 — the undecided head rows
```

The DB alone cannot say *which legal replies* of an open row are still
unresolved (that needs movegen — the harvester's job). What the DB says:
these rows are undecided, and `provenance` tells which shards touched
their subtrees.

**Shortest proven lines.** Minimal bound first, then reconstruct:

```sql
-- the shallowest proven wins (depth_bound 1 here: mate-in-1 discoveries)
SELECT id FROM nodes WHERE outcome = 'win' AND depth_bound =
  (SELECT MIN(depth_bound) FROM nodes WHERE outcome = 'win');
-- then render each id's path via the recursive recipe above
-- standing DB: MIN(win depth_bound) = 1; 710 win rows sit at depth_bound 5
```

For a *provable-claim* line rather than a shortest one: an OR-node's
proving child is its unique `loss` child; walking OR→proving-child and
AND→any child renders a principal line ending at a
`depth_bound = 0` terminal. 24,833 terminal rows exist in the standing DB.

**Provenance and shard audit.**

```sql
SELECT provenance FROM nodes WHERE id = :nid;         -- shard tags of a row
SELECT tag, file, path, outcome, depth_bound, n_nodes
FROM shards ORDER BY tag;                             -- the manifest as rows
```

**DB census (the three numbers to display as a data version stamp).**

```sql
SELECT (SELECT COUNT(*) FROM nodes),
       (SELECT COUNT(*) FROM nodes WHERE outcome IS NOT NULL),
       (SELECT value FROM meta WHERE key = 'built_from');
-- → 55703 | 55628 | e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd
```

## 8. Soundness contract (operator terms)

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

## 9. Troubleshooting and known limitations

**Stale DB (`built_from` mismatch).** `proofdb_harvest`/`proofdb_flip`
abort with e.g.

```text
proofdb_flip: DB built_from deadbeef != manifest digest e91ad57b44008fee (stale DB or stale manifest — re-merge the standing shard set first)
```

The DB was built from a different manifest than the one on disk (usually a
harvest batch completed since the last merge). Fix: re-run
`proofdb_merge` (recipe R1) with the on-disk manifest. The guard is
length-safe on malformed DB values (a short or empty `built_from` aborts
cleanly, regression-tested).

**Merge aborts with exit 2.** A CONFLICT/DEFECT abort (§6.1) leaves no
partially written DB. Read the abort message: it names the shard/manifest
defect. Never "fix" a proven-outcome conflict by editing the manifest —
that is a soundness bug in the pipeline and belongs in a bug report.

**Orphan shards after a harvest abort.** A batch aborted mid-way (exit 2,
or a crash) leaves already-written shard files unreferenced — the manifest
rewrite is skipped on abort. The retry deterministically overwrites them;
no manual cleanup is needed, but an unreferenced `*.bin` in the shard dir
is a symptom worth noting in the operator log.

**Fresh vs seeded ledger.** A fresh ledger (missing `--ledger` file) is
sound but re-censors every frontier reply at the base budget on first
revisit — lower first-batch yield. Seeding from a committed snapshot (§3)
or a `proofdb_ledger_union` merge (§6.4) recovers the censor history. A
ledger never changes DB facts; getting it wrong costs work, not soundness.

**Digest drift after growth.** The quickstart pins (`0d929f4c…`,
55,703 nodes) describe the *standing* shard set. After a decisive harvest
batch the manifest digest changes and a fresh merge produces a different
DB — that is growth, not drift. Compare DBs per manifest digest
(`built_from`), never across manifests.

**Startpos-rooted only.** Subtree-scoped harvesting (`--root-fen`) is
parked (initiative item 7); every job replays from the standard start
position.

**Merge is a separate step.** `proofdb_harvest --out-db/--dump` only
record the intended targets in the summary; after each batch, run
`proofdb_merge` yourself (recipe R2).

## 10. History

The pipeline was built and hardened by the `proofdb` initiative
(`docs/plans/proofdb/`); the tooling-completeness audit that produced this
manual's first version is plan12 (2026-10-02). plan13 (2026-10-02) extended
it into the operator **and user** manual — data-model primer (§4), shard
provenance (§5), per-tool output grammars with worked invocations (§6),
operator + consumer SQL recipes (§7), troubleshooting (§9) — for the
website handoff package (`measurements/plan13/handoff/`), which packages a
digest-pinned rebuilt DB with a consumer-facing `HANDOFF.md`. Per-plan
measurements and the pinned audit results live under
`docs/plans/proofdb/measurements/`.
