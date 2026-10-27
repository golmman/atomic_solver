# proofdb pipeline — operator and user manual

How to run, consume, and extend the startpos proof-line database pipeline
(sidecar shards → fold → standing DB → harvest loop) as shipped in this
repository. The tools live under `examples/` (`proofdb_merge`,
`proofdb_harvest`, `proofdb_flip`, `proofdb_verify`); the DB schema and its
semantics are the sole external contract and are normative in
[`docs/spec/global_proof_store.md`](spec/global_proof_store.md). This manual
serves two readers:

- the **operator**, who runs the pipeline (merge, harvest, flip, censor-
  state maintenance) — §1–§4, §7, §9, §10;
- the **user/consumer** (e.g. the website project), who reads the finished
  SQLite file and wants to understand its shape and query it — §5, §6, §8
  (plus the normative spec).

## Contents

1. [Pipeline map](#1-pipeline-map)
2. [Artifacts and tools](#2-artifacts-and-tools)
3. [Quickstart](#3-quickstart)
   - [3.1 Initial run](#31-initial-run)
   - [3.2 Production run — the walk batch cycle](#32-production-run--the-walk-batch-cycle)
4. [Censor-state pick-up](#4-censor-state-pick-up)
5. [Data model primer (consumer)](#5-data-model-primer-consumer)
6. [Shard provenance — where a shard comes from](#6-shard-provenance--where-a-shard-comes-from)
7. [CLI reference (operator)](#7-cli-reference-operator)
   - [7.1 `proofdb_merge`](#71-proofdb_merge--shards--sqlite-single-writer)
   - [7.2 `proofdb_harvest`](#72-proofdb_harvest--grow-the-shard-set)
   - [7.3 `proofdb_flip`](#73-proofdb_flip--implied-flip-analysis-read-only)
   - [7.4 `proofdb_verify`](#74-proofdb_verify--whole-db-replay-validation-read-only)
8. [Recipes](#8-recipes)
   - [8.1 Operator recipes](#81-operator-recipes)
   - [8.2 Consumer recipes (SQL)](#82-consumer-recipes-sql)
9. [Soundness contract (operator terms)](#9-soundness-contract-operator-terms)
10. [Troubleshooting and known limitations](#10-troubleshooting-and-known-limitations)
11. [History](#11-history)

## 1. Pipeline map

The nodes are **artifacts** — state that persists between steps. The edges
are **tools** — binaries that read and write that state. Both are catalogued
in §2, with the full tool contracts in §7.

```
  shard pairs ──proofdb_merge (fold)──► proofdb.db ◄──proofdb_verify
      ▲          (bin + sidecar; deletable after the fold)   │
      │                            the DB is the single source of truth
      │                                     ├──proofdb_flip──► flip.json
      └──────── proofdb_harvest ◄───────────┘
                  ▲ │
                  └─┘  censor state lives in the DB (schema v2's
                       censor table; the harvest writes rows, a
                       fold leaves them untouched in place)
```

## 2. Artifacts and tools

### 2.1 Artifacts

All **generated** proofdb state lives under `data/proofdb/` by default
(plan14) — a production run needs no path flags at all: the tools invoked
bare operate wholly inside that gitignored working layer. The committed
shard set at `docs/plans/proofdb/shards/` is a **development/validation
fixture** documenting the implementation (tests and measurement batches
use it via explicit flags); it is never read or written by the defaults,
and production runs ignore it entirely.

| Artifact      | Layer                        | Default path                          | gitignored |
| ------------- | ---------------------------- | ------------------------------------- | ---------- |
| SQLite DB     | **single source of truth** + censor state | `data/proofdb/proofdb.db` | yes        |
| shard pairs   | ephemeral transport (deletable after the fold) | `data/proofdb/shards/*.bin` + `*.meta.json` | yes |
| flip report   | analysis output              | `data/proofdb/flip.json`              | yes        |

Because `data/` is gitignored, the DB lives **outside version control**:
an offsite backup of `data/proofdb/proofdb.db` is the operator's
responsibility (recipe R6's `VACUUM INTO` step; the fold digest,
`built_from`, is the integrity stamp for any backed-up copy). Since the
plan22 pivot the DB is **append-only truth**: it is never rebuilt from
shards, so a deleted or corrupted DB can only be restored from a backup —
a shard dir alone is no longer sufficient once folded shards have been
deleted. The shard pairs and the flip report are regenerable/throwaway.

- **SQLite DB (source of truth + scheduling state).** `proofdb.db`, schema
  v2 per the spec: the `meta`/`shards`/`nodes` surfaces (grown by the
  incremental fold — existing rows and their ids are never rewritten) plus
  two scheduling tables — `censor` (the frontier's censor history: facts of
  absence) and `drawn` (paths whose bounded search proved a draw; the tree
  model has no `draw` outcome, so this prunes the subtree without adding a
  tree row). Writer split: `proofdb_merge` is the sole writer of
  `meta`/`shards`/`nodes`; `proofdb_harvest` writes `censor`/`drawn` rows
  only (one committed transaction per record) and a fold leaves both tables
  untouched in place. The two tools must not run concurrently (operational
  discipline; SQLite's locking is the mechanical backstop).
- **Shard pairs (ephemeral transport).** `<tag>.bin` validated binary
  proof trees (`proof_tree_dump.md` v1 format) plus `<tag>.meta.json`
  sidecars (path, fen, outcome, tag, sha256 — the self-describing pair).
  Written by `proofdb_harvest` (bin first, sidecar second — the sidecar is
  the commit marker), read by `proofdb_merge`, **deletable after their
  fold** (recipe R6); every tool tolerates their absence once folded. The
  shard *directory itself* must exist for every tool to run (a missing
  dir is a hard abort — §10): an empty dir is both the bootstrap state
  (R0) and the steady state of a layer whose folded pairs are deleted
  (R6).
- **Flip report (analysis output).** `proofdb_flip`'s JSON verdict. A
  throwaway report, not part of the pipeline's persisted state.

### 2.2 Tools

- **`proofdb_merge`** — _shard pairs → DB fold_, single writer. One
  transaction per invocation: loads the standing tree from the `nodes`
  rows (replay-validated on load), folds every unmerged shard pair
  (replay-validated again), applies the depth fixpoint, and writes the
  delta; any conflict or defect aborts without writing. Tags already in
  the `shards` table are skipped idempotently after a sha256 cross-check.
  Deterministic: the same standing DB + shard set folds to the same
  content.
- **`proofdb_harvest`** — _DB + scheduling state → shard pairs + censor/drawn rows_.
  Selects jobs with the shallowest-first censor-frontier walk
  (`frontier-walk`, the single harvest policy since plan18) — solves each
  under a deterministic child-eval budget, exports and validates the
  decisive ones as `<tag>.bin` + `<tag>.meta.json` pairs, commits censored
  jobs into the DB's `censor` table (one transaction per censor), and marks
  completed draws in the `drawn` table (one row per proven draw) so the
  walk prunes that subtree. It never merges and never writes tree rows.
- **`proofdb_flip`** — _DB → flip report_, read-only. Iterates implied
  outcomes to fixpoint over the stored tree and re-checks every flip with an
  independent verifier; writes only the report.
- **`proofdb_verify`** — _DB → verdict_, read-only. Replay-validates the
  whole stored tree read back from the DB alone (no shards needed) against
  the spec's invariants — the standing DB's integrity instrument; `--backup`
  writes a `VACUUM INTO` offsite copy (the recipe-R6 backup duty).

Full CLI options, output grammars, and exit codes are in §7; censor-state
pick-up semantics are in §4.

**Production vs development.** _Production_ runs use the bare defaults:
every artifact inside `data/proofdb/`, the committed fixture ignored.
_Development/validation_ runs (tests, measurement batches, fixture work)
pin **explicit** `--shard-dir`/`--db` flags against
`docs/plans/proofdb/shards/` — as every plan measurement did (the merge's
`--manifest` flag is its legacy migration source). The two modes never mix
by accident: defaults never touch the fixture, and explicit flags never
touch `data/proofdb/`.

## 3. Quickstart

This walkthrough bootstraps a **production-shaped** working layer from
nothing: no shards, no DB. It uses only the bare
defaults — every artifact lands inside `data/proofdb/` (gitignored),
nothing under `docs/plans/` is touched, and cleanup is a single
`rm -rf data/proofdb`. Run it from the repo root. §3.1 prepares everything
the pipeline needs — bootstrap merge plus a deliberately tiny smoke
harvest; §3.2 grows the database batch by batch from exactly the state
§3.1 leaves behind. The single harvest policy throughout is the plan16
`frontier-walk` shallowest-first censor-frontier walk (the plan18 policy
consolidation removed the other policies; §7.2).

### 3.1 Initial run

```bash
cargo build --release --examples

BIN=target/release/examples

# 1. Bootstrap fold: an empty shard dir → a root-only DB (created on
#    demand; a bare run needs nothing else).
"$BIN/proofdb_merge"

# 2. Smoke harvest: 2 jobs, tiny budget — both are expected to censor.
"$BIN/proofdb_harvest" --budget-evals 200000 --max-jobs 2
```

Expected bootstrap-fold output — a tree with just the open root:

```
census: folded 0 shards (0 already merged, 0 orphans skipped); shard tree nodes 0;
        overlay new 0; dedupes 0; open-ancestor insertions 0; open nodes upgraded 0;
        merged nodes 1 (proven 0, open 1); skeleton re-validations 0
node arithmetic: this fold 0 shard nodes = 0 overlay-new + 0 deduped (+0 open ancestors);
                 standing nodes 1
db: data/proofdb/proofdb.db (nodes 1), fold digest e3b0c44298fc1c14
```

Expected harvest stderr — the walk census reports empty censor state with the
root open (`root open`; the virtual-root rule, §7.2), and the DB line names
the default path:

```
harvest: db data/proofdb/proofdb.db (1 rows, fold digest e3b0c44298fc1c14) — policy frontier-walk
frontier-walk: records 0 (lineage 0, ancestor 0, or_closed 0, drawn 0, out_of_scope 0, complete 0) — root open; base budget 200000; scope global
```

then two `job: {…}` lines (stdout): the virtual-root **search visit**
(class `C1`, `kind: "search"`) and the **expansion visit** into the root's
first unhandled child `a2a3` (class `C3`, `kind: "expand"`), both
`"outcome": "censored"`, `"pass": 1`, `"work_before": 0`, and

```
harvest: policy frontier-walk stop=max-jobs jobs 2 decisive 0 censored 2 drawn 0 evals 400015 new_shards 0 or_closes 0 wall 0.1s
```

(exit 0; `wall`, the `wall_s` fields, and the small chunk-overshoot
deltas on the `child_evals` values — here 200015 over the 200000 budget —
are host-dependent; the counts are deterministic per host, re-pinned by
report20 on the reference host at 200021 and by report21 on the dev
container at 200015). Censored jobs write no facts, so the shard dir
stays **empty** (no durable file touched) and the DB's `censor` table
(schema v2) now holds two censor rows — the root and `a2a3`, each at the
200k flat budget:

```sql
SELECT path, work_done, passes_failed FROM censor;
-- ''      | 200000 | 1
-- 'a2a3'  | 200015 | 1
```

Re-running the same command continues the sweep instead of repeating the
two visits: both records are now at the budget, so the walk expands into
their first not-yet-handled children (§7.2) — the censor-state pick-up (§4)
carries the sweep forward.

A _decisive_ harvest instead leaves unmerged shard pairs behind, so they
must be folded in before the next pass: run step 1 (`proofdb_merge`)
again after the harvest, or `proofdb_harvest`/`proofdb_flip` refuse on
the unmerged-shard gate (§10). The smoke budget above deliberately
censors, so a follow-up batch runs against the same DB without an
intermediate fold.

The layer is now ready for §3.2: the bootstrap merge produced the root-only
DB, and the smoke harvest verified the bare defaults end-to-end without
writing any fact.

### 3.2 Production run — the walk batch cycle

**Prerequisite — a bootstrapped layer.** §3.2 continues from exactly the
state §3.1 leaves behind: the root-only DB at the default path (on a fresh
machine without even that state, recipe R0, §8.1, creates it). Set
`BIN=target/release/examples` as in §3.1.

**The batch cycle.** The walk is a live selection loop that never retires
— on an undecided layer it always has work (the plan21 exception: a fully
or-closed world legitimately exhausts, see the sibling-skip bullet below) —
so every session needs a stop condition; `--max-total-evals` is the
deterministic choice:

```bash
$BIN/proofdb_harvest --policy frontier-walk --budget-evals 10000000 \
    --max-total-evals 10000000000 --tt-mb 1024
$BIN/proofdb_merge    # fold the batch's shard pairs into the DB (the harvest never merges)
$BIN/proofdb_verify --db data/proofdb/proofdb.db   # replay-validate the grown DB (expect: ok)
$BIN/proofdb_flip     # soundness sanity over the grown DB (expect: flips 0 verified 0)
```

You can gracefully stop the execution via `touch STOP` (see the
`--stop-file` option). Crashes or ctrl-c lose at most the in-flight job's
work and leave at most one orphan bin without a sidecar (§7.2, §10).

Repeat the cycle. After a batch with decisive jobs the shard dir carries
unmerged sidecars — that is growth, not drift (§10) — and the bare fold
before the next session is mandatory (the unmerged-shard gate refuses
both `proofdb_harvest` and `proofdb_flip` until the fold).

**The reference boot.** A 2G-capped boot over a root-only layer (the
plan16 boot-gate protocol, budget 4,000,000) runs `stop=budget jobs 549
decisive 49 censored 500 drawn 0 evals 2000051685 or_closes 0 wall 414.3s` — 49
decisive shards, no flags beyond the policy and the cap (re-pinned by
report21 on the dev container; the plan16 gate's 553 jobs / 53 shards /
6,706 nodes predate both the plan20 chunk-accounting change and the plan21
skip, and the overshoot/wall values are host-dependent). The boot's jobs
are 1 search visit + expansion visits: after the root censor, _expansion
replaces deepening_ — the sweep is breadth-first by `(ply, path)`, each
candidate running to completion before the next at its ply. The boot grew
the layer to **4,030 nodes** (1 root + 3,975 overlay + 54 open-ancestor
insertions, 6 open rows) with `flips 0 verified 0`; a second boot in a
fresh layer reproduced the censor rows, all 49 shards, and the merged DB
byte-identically (the determinism gate, re-verified across the plan21
skip — which does not fire on a fresh layer: every decisive boot job is
an odd-ply refutation win, so `or_closes 0` and the pre-plan21 HEAD build
agrees job-for-job; run pre-plan22, when the durable layer was still
manifest-indexed).

**The walk's budget model.** `--budget-evals` is the flat per-visit
budget `B` (0 → the 4,000,000 base) and there is **no ladder**: a node
censored at `B` is never re-searched deeper by this policy — once its
`work_done` reaches `B`, the walk moves to _expansion_ (searching its
first not-yet-handled child at `B`). Raising `--budget-evals` between
sessions is the deepening mechanism: records whose `work_done` is below
the new `B` are re-searched once at it (the walk's one accepted re-search
waste), and every fresh visit runs at the new depth. A censor that spends
zero evals (an on-path repetition draw) is complete-marked instead of
re-queued — the walk cannot livelock on it.

**What growth looks like.** Expect censoring to dominate; a batch is a
multi-hour commitment (at ~200k child-evals/s, a 10G batch is roughly
half a day), and yield is single-digit shards. Read progress from the
censor-state growth and the session-start census (which censor paths are
already priced, which are fresh), not from one batch's shard count.

**Cold start.** The walk boot itself is the cold start: recipe R0
(bare fold over the empty default shard dir) plus a capped walk session
grows a layer from nothing — no artifact-copying, no fixture dependency. The committed
shard set at `docs/plans/proofdb/shards/` keeps a development/validation
role only (R1, tests).

The knobs that matter, most-used first:

- **`--budget-evals`** — the flat per-visit child-eval budget (default
  4,000,000, so the flag is usually omitted at small budgets and set
  explicitly for deep sweeps). It is the depth of every visit, not a
  screening budget: the sweep slows proportionally to the budget, and
  raising it between sessions is how censored nodes get re-priced (§
  above).
- **`--max-total-evals`** — deterministic session cap across all jobs
  (child-evals are the currency the budget spends). Prefer this over
  wall time for reproducible batch sizes.
- **`--max-runtime` / `--max-jobs` / `--stop-file`** — operational stop
  conditions, checked between jobs. `touch STOP` (default path) ends an
  unattended batch gracefully after the in-flight job — the intended way
  to stop a run before its eval budget is spent.
- **`--tt-mb`** — per-job transposition table (default 128 MB; since
  lean plan15 size labels are exact upper bounds — the allocation
  never exceeds the label — so 128 MB now means ≤ 112 MiB). The label
  is a *reservation* bound: table pages are populated on first use, so
  a job's TT contribution to RSS is bounded by the label rather than
  equal to it. The canonical cycle uses 1024. Every job runs with a
  fresh TT, so this bounds peak RAM per job; TT size measurably affects
  large budgets (a multi-billion-eval visit can censor at 128 MB that
  survives at 1 GB), so give deep sweeps headroom.
Operational notes:

- Jobs are deterministic: fresh TT per job, fixed child-eval budgets, no
  wall clock in any decision. Given the same DB and censor state, the
  same command reproduces the same job sequence; the summary's `wall` is
  the only host-dependent number (§7.2).
- Every censor commits one `censor`-table transaction (plan20), and every
  decisive job leaves a durable bin + sidecar pair on disk (the sidecar
  written last, atomically) — so an unexpected crash or ctrl-c loses at
  most the in-flight job's work: at most one bin without its sidecar
  (§7.2, §10). The DB `data/proofdb/proofdb.db` is unversioned and
  append-only — keep an offsite backup current (§2.1, recipe R6).
## 4. Scheduling-state pick-up

The frontier's **scheduling state** is the DB's `censor` table (one row per
known-open frontier path with its censor history: `work_done`,
`passes_failed`) plus the `drawn` table (one row per path whose bounded
search proved a draw). Since plan20 it lives in the DB — there is no
separate `work.json` anymore (the pre-plan20 `--ledger` flag is retired; the
JSON sidecar and its `proofdb_ledger_union` merge tool with it).

**Pick-up semantics.** The scheduling state is part of the DB, so pick-up
needs no setup: a merged DB carries its censor history and proven-draw
marks, and a harvest session continues exactly where the last one stopped.
A DB without a `censor` table (a schema-v1 file) aborts cleanly with an
upgrade hint — the fold cannot upgrade it in place; rebuild the layer from
a pre-plan20 backup or restart fresh before harvesting. Two ways to start
over:

1. **Fresh** (sound by construction): wipe the censor rows
   (`DELETE FROM censor;`; delete `drawn` too for a full re-exploration) —
   every frontier path is re-censored at the budget before the walk
   resumes; lower yield on the first revisit, no other effect.
2. **Import a legacy ledger** (`proofdb_merge --import-censor <work.json>`,
   §7.1): merges a pre-plan20 JSON ledger into the table under the
   per-field max rule (`work_done`/`passes_failed` each keep the max) —
   idempotent, and it never decreases a stored value, so importing stale
   snapshots after newer work is harmless. Legacy snapshots still referenced
   by committed measurements (e.g. the plan10 snapshot with 8,446 records,
   `docs/plans/proofdb/measurements/plan10/ledger_union.json`) migrate this
   way in one command.

The scheduling state is deliberately **not** proof data (spec §2.1): it is
state inside the DB, its loss costs re-searched work, never soundness.

## 5. Data model primer (consumer)

This section is consumer-level prose. The normative contract is the spec
(`docs/spec/global_proof_store.md`) — schema §2, field semantics §3,
invariants §4. Everything below can be verified by SQL against the DB alone.

**Node identity is the move path.** Every row of `nodes` is a position
reached by a specific move sequence (UCI, space-separated along the
`parent_id` chain) from `meta.root_fen` — the standard start position. The
same position reached by two different routes is **two different rows**:
halfmove-clock-dependent repetition verdicts are part of what a path fixes,
and facts are never transferred across paths (spec invariant 6). Row `id`s
are stable surrogate keys (spec invariant 7): never renumbered, appended by
each fold; a fresh fold assigns them in lexicographic path order, so the
root is `id = 0` — but identity is the path, never the id.

**Partial AND/OR tree.** `outcome` is from the _side-to-move_ perspective:

| `outcome` | role     | what the tree stores below it                                                                                              |
| --------- | -------- | -------------------------------------------------------------------------------------------------------------------------- |
| `'win'`   | OR-node  | at least one proven-loss child — the refutation of the opponent's best try; additional proven-loss children are annotations (merged alternatives), and non-proven replies are not rows |
| `'loss'`  | AND-node | every legal, non-terminal reply as a child with `outcome = 'win'` (terminal replies are absent — terminality is derivable) |
| `NULL`    | open     | undecided; may still have stored children (resolved replies, or an open ancestor chain over a grafted subtree)             |

Two consequences a consumer must not get wrong:

- **The tree is partial, and absence is not evidence.** A missing child row
  means "no proven fact recorded for that reply (yet)". It does _not_ mean
  the move is illegal, decided, or refuted. The root, for example, has 7
  stored children (its two knight moves and the five most informative pawn
  pushes, all still open); the other ~34 legal replies simply have no rows.
- **Open is open.** `outcome IS NULL` covers both "searched and no decisive
  result within some bounded budget" and "not yet searched" — the DB does
  not distinguish the two (spec invariant 5). A non-terminal draw is also
  represented as `NULL`; no `draw` outcome exists in the tree schema. A
  proven draw lives only in the `drawn` scheduling table (§4), which the
  walk prunes.

**Depth vocabulary: bound by default, exactness an annotation.** A proven
node's `depth_bound` proves _the decisive outcome within ≤ depth_bound
plies from this node_. `depth_status = 'bound'` is the default (the true
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

**The `meta` table.** `schema_version=2`, `root_fen`, `node_count`
(rows in `nodes`), `built_from` (the fold digest — the SHA-256 over the
sorted `tag sha256` lines of the `shards` table; spec §2.2), `generator`
(free-form producer identification). Consumers should surface `built_from`
as a version stamp of the data they display.

## 6. Shard provenance — where a shard comes from

A **shard** is one validated binary proof tree (`proof_tree_dump.md` v1
format) rooted at a job position, transported as a self-describing
`<tag>.bin` + `<tag>.meta.json` sidecar pair. Shards are ephemeral
transport; the DB is the source of truth — a folded shard pair is
deletable (recipe R6). This section explains how a shard comes to exist
and how to inspect one — useful to an operator auditing the transport
layer, and to a consumer who wants to know what `provenance` tags refer
to.

**Production path (all inside `proofdb_harvest`):**

1. The walk selects a frontier path from the censor state + merged DB
   (§7.2).
2. The job replays that path from the startpos, then runs a bounded
   solver session on the reached position (fresh TT per job, deterministic
   child-eval budget).
3. **Decisive result** → the solver's proof-tree reconstruction is
   exported as `<tag>.bin`, replay-validated, and written to the shard dir
   with its `<tag>.meta.json` sidecar (bin first, sidecar second — the
   commit marker); the census `job:` line carries the `tag` and
   `shard_nodes`. **Proven draw** (`Complete` Draw) → no shard pair, no tree
   row, a `drawn` scheduling row (the walk prunes the subtree).
   **Censored result** (any other draw, budget exhausted) → no shard pair,
   a `censor` row only.
4. After the batch, run `proofdb_merge` (§8.1, recipe R2): every shard is
   replay-validated _again_ at fold time, grafted at its sidecar path,
   and only then becomes DB rows (recorded in the `shards` table).

**Sidecar anatomy** (one standing-set sidecar, verbatim):

```json
{
  "fen": "1nbqkbnr/1ppppppp/8/8/8/2PPP3/5PPP/RNBQKBNR w KQk - 0 6",
  "file": "h_013861d97dce5d48.bin",
  "moves": [
    "a2a3",
    "a7a6",
    "b2b3",
    "a6a5",
    "c2c3",
    "a5a4",
    "d2d3",
    "a4b3",
    "e2e3",
    "a8a3"
  ],
  "outcome": "win",
  "sha256": "af09ff3dd6f98294c91918af1e235b96c11003f198d00660f5d024ecbba61bd4",
  "tag": "h_013861d97dce5d48"
}
```

`moves` is the UCI path from the startpos to the shard root; `fen` is the
replayed shard-root position (the fold cross-checks it against the
replay); `outcome` is what the shard proves; `sha256` is the bin digest —
the corruption cross-check and the fold's idempotence key; a bin/sidecar
mismatch is refused. (The committed fixture set's sidecar files were
written by that set's one-time migration fold, which backfilled them from
its legacy manifest; the harvest writes fresh sidecars directly.) Tag
conventions:
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

## 7. CLI reference (operator)

Every tool: `1` = usage error, `2` = defect abort where applicable, `0` =
success. Options are shown with their defaults; "required" options have no
default.

### 7.1 `proofdb_merge` — the incremental fold (single writer)

**Purpose.** Fold unmerged shard pairs into the standing DB — the
single writer of the tree surfaces (`meta`/`shards`/`nodes`). One
transaction per invocation: the standing tree is loaded from the `nodes`
rows by a replay-validated DFS (a structurally broken DB aborts before
any write), every unmerged shard is replay-validated and grafted, the
depth fixpoint runs, per-graft skeletons are re-validated, and the delta
is committed — new node rows with appended ids (never renumbered), UPDATEs
for refined rows, one `shards` row per folded tag, `meta` refreshed
(`node_count`, `built_from` = the fold digest, `generator`). Tags already
in the `shards` table are skipped after cross-checking their stored
`sha256` against the bin on disk. The `censor` table is untouched in
place. Any conflict or defect rolls the whole invocation back — conflicts
are never patched. All paths default into `data/proofdb/` (plan14) —
invoked bare it is a production fold; the committed fixture is consumed
only via explicit flags (recipe R1).

**Prerequisites.** A shard dir of self-describing pairs (§5.1 of the
spec; a bin without a sidecar is an orphan — skipped with a census note;
a sidecar without a bin refuses). A missing `--db` file is created fresh
(a fresh fold assigns ids globally in lexicographic path order); an
existing DB must be schema v2 (a v1 file aborts with the move-aside hint).
`--manifest` is a **legacy** metadata source: read only when the file
exists, used to backfill sidecars for unmerged tags that lack one; with
no manifest on disk a fold is pure sidecar mode. Reading never creates
anything; the DB write creates `data/proofdb/` as needed.

    usage: proofdb_merge [--manifest <index.json>] [--shard-dir <dir>]
                         [--db <out.db>] [--dump <nodes.txt>]
                         [--sample-lines <n>] [--import-censor <work.json>]

| Option            | Default                             | Meaning                                            |
| ----------------- | ----------------------------------- | -------------------------------------------------- |
| `--manifest`      | `data/proofdb/shards/manifest.json` | legacy manifest (migration/metadata source only; read when present, backfills sidecars — sidecar metadata wins) |
| `--shard-dir`     | `data/proofdb/shards`               | directory holding the `<tag>.bin` + `<tag>.meta.json` pairs |
| `--db`            | `data/proofdb/proofdb.db`           | the standing DB (created fresh if missing; never rebuilt) |
| `--dump`          | off                                 | canonical text dump of the folded tree             |
| `--sample-lines`  | `0`                                 | print `n` random root-to-leaf walks as spot-checks |
| `--import-censor` | off                                 | merge a legacy pre-plan20 JSON work ledger into the `censor` table under the per-field max rule (idempotent, never decreases a stored value; in place inside the fold transaction, §4) |

**Exit codes.** `0` folded and committed; `1` usage error; `2`
CONFLICT/DEFECT abort (a pre-v2 DB file, unreadable/unparseable shard,
replay validation defects, sidecar cross-check mismatch, hash-fidelity
failure, standing-sha mismatch on a skipped tag, proven-outcome conflict,
depth-fixpoint error, skeleton re-validation failure, DB write failure).
An abort writes nothing — the transaction rolls back.

**Output grammar (stdout, three lines + optional lines).**

```text
census: folded 262 shards (0 already merged, 0 orphans skipped); shard tree nodes 56226;
        overlay new 55360; dedupes 866; open-ancestor insertions 342; open nodes upgraded 268;
        merged nodes 55703 (proven 55628, open 75); skeleton re-validations 262
node arithmetic: this fold 56226 shard nodes = 55360 overlay-new + 866 deduped
                 (+342 open ancestors); standing nodes 55703
db: data/proofdb/proofdb.db (nodes 55703), fold digest 14fd07530996eb9e
```

- `census:` — this fold's coverage (folded/already-merged/orphan-skipped),
  raw shard-tree nodes, how many became new overlay rows, how many were
  deduped against already stored facts, how many open ancestor rows were
  inserted to graft, how many open rows a graft upgraded to proven, and
  the final merged counts (proven = win+loss, open = `outcome IS NULL`),
  plus the per-graft skeleton re-validation count (one per folded shard,
  262/262 must be clean).
- `node arithmetic:` — the bookkeeping identity restated; if the two sides
  of either equation disagree, the fold is defective.
- `db:` — the standing DB, its node count, and the 16-hex prefix of the
  fold digest (`meta.built_from`; the full digest is in the DB).
- `backfill: N sidecars written from manifest metadata` — printed only
  when `--manifest`'s legacy entries were folded into backfilled sidecars
  (recipe R1's migration step).
- `censor: N imported records (max rule, in place)` — printed only with
  `--import-censor`.
- with `--sample-lines n` (printed _before_ the census): `n` lines like
  `sample 1: e2e4 h7h6 d1h5 e7e5 h5f7 (5 plies, end: loss bound 0 (exact))`
  — random root-to-leaf walks as spot-checks.

**Worked invocation.** Recipe R1 (standing fixture shard set with its
legacy manifest; the census above is verbatim); the production bootstrap
fold is in §3.1.

### 7.2 `proofdb_harvest` — grow the shard set

**Purpose.** Select frontier jobs, solve them under deterministic
child-eval budgets, and export validated shard pairs (bin + sidecar) for
the decisive ones. Censored jobs (no decisive result, budget exhausted)
record a fact of absence as a `censor`-table row only.

**Prerequisites.** A merged schema-v2 DB, and a clean unmerged-shard gate:
every sidecar in the shard dir must already have a `shards`-table row (a
sidecar without a row → exit-1 refusal naming the tag — run
`proofdb_merge` first; a sidecar without a bin → refusal as a
transport-layer defect; a bin without a sidecar is the interrupted job's
orphan and is ignored). The session's collision sets are seeded from the
DB's `shards` rows. The DB is opened read-write for the censor commits
(plan20 D3) — the only DB writes this tool ever makes; the default shard
dir `data/proofdb/shards/` is created on the first shard write.

    usage: proofdb_harvest [--db <proofdb.db>]
                           [--shard-dir <dir>] [--policy frontier-walk]
                           [--budget-evals <n>] [--max-total-evals <n>]
                           [--tt-mb <mb>]
                           [--fill-attempt-budget <n>] [--fill-total-budget <n>]
                           [--max-jobs <n>] [--max-runtime <s>]
                           [--stop-file <path>] [--moves '<uci path>']
                           [--out-db <grown.db>] [--dump <nodes.txt>]

| Option               | Default                             | Meaning                                                                             |
| -------------------- | ----------------------------------- | ----------------------------------------------------------------------------------- |
| `--db`               | `data/proofdb/proofdb.db`           | the merged DB to work against (read-only on the tree surfaces; censor-row writes only, plan20 D3) |
| `--shard-dir`        | `data/proofdb/shards`               | where new shard pairs are written (created on first write)                          |
| `--policy`           | `frontier-walk`                     | kept for command-line compatibility; only `frontier-walk` is accepted (plan18)      |
| `--budget-evals`     | `0` → base `4,000,000`              | the flat per-visit child-eval budget `B` (no ladder)                                |
| `--max-total-evals`  | `0` (unlimited)                     | session cap across jobs                                                             |
| `--tt-mb`            | `128`                               | per-session transposition table (MB) — also the eviction lever for a repeated export abort (§10) |
| `--fill-attempt-budget` | `2000000`                        | per-attempt child-eval budget of one export fill search (§10, recovery ladder)      |
| `--fill-total-budget`| `100000000`                         | child-eval budget summed over all fills of one export; `0` = unbounded              |
| `--max-jobs`         | `0` (unlimited)                     | stop after n jobs                                                                   |
| `--max-runtime`      | `0` (unlimited)                     | stop after n seconds                                                                |
| `--stop-file`        | `STOP`                              | batch stops (gracefully, after the in-flight job) when this file appears            |
| `--moves`            | off (global session)                | scoped harvest: whitespace-separated UCI moves naming a startpos path — the session runs with its world rooted at that node (selection only: same censor state, DB, shard pairs); the path is replay-validated before any state is loaded (nonsense or prefix-illegal → usage exit 1); `--moves ''` is rejected (the flag's absence already means global) |
| `--out-db` / `--dump`| off                                 | recorded in the summary only — **the caller runs `proofdb_merge` separately** (by design; this CLI never merges) |

**Selection policy — `frontier-walk` (plan16; the only policy since the
plan18 consolidation).** A live selection loop over the DB's censor state
plus the merged DB's rows; the frontier mutates between iterations, so there
is no precomputed job sequence. The mechanism, in operator terms:

- **Candidates** are the censor records, minus records whose path (or a
  proper prefix, the empty prefix included) is a *proven* DB row —
  lineage/ancestor drops, the pick-up gate — records at-or-under an
  **or-closed** white-to-move parent (the plan21 OR-node sibling skip, see
  below), and records decided or complete-marked earlier in the session.
  Selection is the shallowest candidate first, `(ply, path)`-lexicographic.
- **Search visit** (candidate with `work_done < B`): replay the path,
  search at the flat budget `B` (retained TT, so a revisit is never a
  byte-identical repeat); decisive → shard pipeline; `Complete` draw →
  commit a `drawn` row and prune the subtree (no shard, no tree row); any
  other draw → bump the record's `work_done`/`passes_failed` and commit the
  censor row (one transaction per censor, plan20 D3).
- **Expansion visit** (candidate with `work_done ≥ B`): enumerate the
  legal children in lex UCI order and search the first *unhandled* one at
  `B`. *Handled* = a stored DB row (any outcome), a standing or in-session
  shard path, a proven-draw mark, or a censor record already at `≥ B` — so
  the sweep walks *down* the tree breadth-first instead of deepening a
  censored node.
- **Complete rule**: a candidate with `work_done ≥ B` and no unhandled
  child is complete-marked (a pick-only iteration, no evals); each
  candidate completes at most once, so the loop cannot stall. A censor
  that spends zero child-evals is also complete-marked; a zero-eval
  `Complete` draw instead takes the proven-draw branch and is pruned as a
  draw.
- **Root rule**: when the candidate set is empty, the scope root (see the
  next bullet) becomes a virtual candidate (this is how a root-only layer
  bootstraps, §3.2) — unless the scope root is proven-stored, or-closed, or
  complete-blocked, in which case the session stops `exhausted`.
- **OR-node sibling skip (plan21; always on, no flag).** At a
  **white-to-move** node (even absolute ply — the startpos is white, strict
  alternation), one child proven `Loss` (the defender to move loses ⇒ the
  white move into it wins) establishes the node: the walk prunes all
  sibling work there. Statically, every censor record at-or-under such a
  parent is a non-candidate (`or_closed` in the census); dynamically, a
  decisive `loss` job at an odd-ply child or-closes its parent in-session
  (`or_closes N` in the summary) and a decided job's whole subtree leaves
  candidacy — the in-session form of the cross-session lineage gate.
  Standard AND/OR semantics: at an OR node one proving child suffices; a
  child proven `Win` is a *refutation* of the parent's move and never
  triggers, and black-to-move nodes behave exactly as before (their
  refutation corridors are wanted in full). The drops are
  candidacy-only — censor rows are never deleted — so the next policy
  change or scope re-picks the territory up. An or-closed scope root ends
  the session `exhausted` (a scoped session prints an explanatory stderr
  note, like a proven-stored one). The white-wins assumption motivates the
  corridor narrowing but is not required for soundness: if a white-to-move
  node is in fact a loss, no child ever proves `Loss`, nothing triggers,
  and all its children are explored exactly as before.
- **Scoped harvest (`--moves`)**: `--moves 'a2a4 b7b5 b1a3'` re-anchors
  the session at the node reached by replaying the moves from the
  startpos — the scope node *is* the walk's root (a re-anchor, not a
  filter): the virtual-root bootstrap, the lineage/ancestor drops, the
  ordering, and the census all evaluate scope-relative values, so no
  budget is spent above the scope and no in-scope record is silently
  excluded behind settled territory. Out-of-scope censor records are
  never candidates, never visited, and never written — but stay in the
  shared censor state untouched (censors only bump visited nodes), so later
  global sessions pick them up unchanged; they are counted in the census
  as `out_of_scope`. Proven DB rows *above* the scope root are ignored
  (the operator chose the scope deliberately — the same holds for
  or-closedness above the scope root, plan21); a proven-stored or
  or-closed scope root (a proven DB row or a standing/in-session shard
  path at the scope key, or a white-to-move scope root with a proven
  `Loss` child row)
  ends the session `exhausted` with an explanatory stderr note — no work
  is done beneath proven or or-closed nodes. Everything else (budget, caps, stop file, censor
  state, shard pairs) keeps its tool-wide semantics, and the searches and
  their results are identical wherever the walk starts (every job replays
  the full startpos path).
- **No ladder**: a node censored at `B` is never re-searched deeper by
  this policy. Raising `--budget-evals` between sessions is the
  deepening mechanism (underworked records are re-searched once at the
  new `B`, §3.2). The walk never retires: on an undecided layer it always
  has work, so a session needs a cap (`--max-total-evals`, `--max-jobs`,
  …) to stop — the only exception is the plan21 skip's endgame: a fully
  or-closed scope (every white-to-move node covered by a proven winning
  child) legitimately exhausts.

**Exit codes.** `0` batch completed; `1` usage error (including the
retired `--manifest` flag, which refuses with a pointer to
`proofdb_merge`); `2` ABORT on any defect. The produce pipeline writes
the `<tag>.bin` first, then its `<tag>.meta.json` sidecar (tmp + rename) —
the sidecar is the commit marker, so after every finished job the
transport layer is consistent: every written bin has its sidecar, and an
unexpected crash or ctrl-c loses at most the in-flight job's work — at
most one orphan bin without a sidecar remains (deterministically
overwritten by the retry); a sidecar-write failure aborts the session
with that bin as the one orphan. There is no signal handling: the stop
conditions below stay the only graceful exits; this contract only narrows
what an unclean exit costs. Stop
reasons in the summary: `budget` (`--max-total-evals` cap reached),
`max-jobs`, `max-runtime`, `stop-file`, `exhausted` (no candidate can
yield a visit). All stop conditions are checked between jobs only — a
started job always runs to its granted budget.

**Output grammar.**

_stderr, session start_ (informational; the numbers are the censor-frontier
state):

```text
harvest: db data/proofdb/proofdb.db (4314 rows, fold digest 14fd07530996eb9e) — policy frontier-walk
frontier-walk: records 368 (lineage 0, ancestor 0, or_closed 0, drawn 0, out_of_scope 0, complete 0) — root open; base budget 4000000; scope global
```

Since plan20 the `harvest:` echo no longer names a ledger path, and since
plan22 it stamps the DB with its **fold digest** (`meta.built_from`, §7.1)
instead of a manifest digest. The census line reports: total
censor records, the lineage/ancestor drops
(records on or behind a proven DB row — not candidates), the plan21
`or_closed` drops (records at-or-under an or-closed white-to-move parent —
not candidates; the shallowest decisive prefix wins the attribution, so a
record behind both a proven row and an or-closed parent counts whichever
prefix is shallower), the
out-of-scope
records (outside the session scope — never candidates, preserved for later
global sessions; `0` for a global session), the in-session complete marks
(0 at session start), the root state of the session's scope root (`root
open` / `root proven` / `root fresh`), the flat budget, and the scope (the
`--moves` path, or `global`; the reference boot on a root-only DB prints
`records 0 … — root open; base budget 4000000; scope global`). When
scoped, the `harvest: db` echo above carries `; scope <path>` as well.

The example above is the regenerable 52-shard dev layer. On layers grown
past the skip the `or_closed` count reaches the thousands: the committed
fixture carries 23,885 or-closed parents covering 90% of its nodes, and
the standing layer 6,306 covering 29% — but their *current* censor
frontiers also sit entirely outside the or-closed subtrees, so today's
census lines still read `or_closed 0` on both (report21 §6).

_stdout, per job_ — one `job: {…}` JSON line per visit. Field table (as
emitted by `examples/proofdb/session/record.rs`; `pass`/`work_before`/
`kind` are the walk census and are omitted on a non-census record):

| field          | meaning                                                                                                                             |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `path`         | UCI path of the job position (space-separated)                                                                                       |
| `policy`       | the `--policy` used (`frontier-walk`)                                                                                                |
| `class`        | frontier class of the job (`L` on a censor record, `C3` on a fresh child, `C1` on the virtual root)                                  |
| `tier`         | job tier (the walk labels all jobs `frontier-walk`)                                                                                  |
| `budget`       | child-eval budget granted for this visit                                                                                             |
| `child_evals`  | child-evals actually spent                                                                                                           |
| `wall_s`       | job wall time (seconds)                                                                                                              |
| `outcome`      | `"win"` / `"loss"` (decisive, shard exported), `"draw"` (proven draw, `drawn` row, no shard) or `"censored"` (fact of absence)     |
| `exit_reason`  | solver exit: `Complete` (decisive or proven draw) or `BudgetExhausted` (censored)                                                   |
| `tag`          | shard tag if decisive (the sidecar tag; matches `provenance` in the DB)                                                              |
| `shard_nodes`  | exported shard's node count if decisive                                                                                              |
| `pass`         | visit rung: `passes_failed + 1` (the walk census)                                                                                    |
| `work_before`  | censor `work_done` recorded before this visit (the walk census)                                                                      |
| `kind`         | visit kind: `search` (a picked underworked candidate) or `expand` (the first unhandled child of a worked candidate)                  |

**Censored** job records (quickstart §3.1, the smoke's two visits;
`wall_s` varies by host):

```text
job: {"budget":200000,"child_evals":200000,"class":"C1","exit_reason":"BudgetExhausted","kind":"search","outcome":"censored","pass":1,"path":"","policy":"frontier-walk","shard_nodes":null,"tag":null,"tier":"frontier-walk","wall_s":0.02,"work_before":0}
job: {"budget":200000,"child_evals":200015,"class":"C3","exit_reason":"BudgetExhausted","kind":"expand","outcome":"censored","pass":1,"path":"a2a3","policy":"frontier-walk","shard_nodes":null,"tag":null,"tier":"frontier-walk","wall_s":0.02,"work_before":0}
```

A **decisive** job record (the reference boot's first decisive expansion
visit, shown in the current grammar; `wall_s` varies by host):

```text
job: {"budget":4000000,"child_evals":11554,"class":"C3","exit_reason":"Complete","kind":"expand","outcome":"win","pass":1,"path":"c2c3 g7g5","policy":"frontier-walk","shard_nodes":334,"tag":"h_2fd82f9a8230a668","tier":"frontier-walk","wall_s":0.0017,"work_before":0}
harvest: policy frontier-walk stop=budget jobs 549 decisive 49 censored 500 drawn 0 evals 2000051685 new_shards 49 or_closes 0 wall 414.3s
```

_stdout, after the last job_ — the summary line; `out-db:`/`dump:` echo
lines follow when those options were given:

```text
harvest: policy frontier-walk stop=max-jobs jobs 2 decisive 0 censored 2 drawn 0 evals 400015 new_shards 0 or_closes 0 wall 0.1s
```

- `harvest:` — the `stop=` reason, then `jobs N decisive D censored C
  drawn R evals E new_shards S or_closes O wall Ws`. `decisive` counts jobs
  that exported a shard pair; `censored` counts facts of absence; `drawn`
  counts proven draws (`Complete` Draw jobs, no shard/tree row, a `drawn`
  scheduling row); `or_closes` counts the plan21 dynamic or-closes
  (in-session closures of white-to-move parents on a decisive `loss` child;
  the static census drops are the `or_closed` census field, not this).

Under `frontier-walk`, `stop=exhausted` means no candidate can yield a
visit (every candidate complete and the scope root proven-stored, or-closed,
or complete-blocked — a scoped session with a proven-stored or or-closed
scope root exhausts immediately, with the explanatory stderr note); on any
undecided layer the walk always has work, so capped stops
(`budget`/`max-jobs`) are the normal way a session ends.

_stderr, during jobs_ — `[bounded_search] chunk done: …` progress lines
(one per budget chunk); informational, safe to ignore in scripts.

**Worked invocation.** Initial run §3.1 (the smoke shape: two censored
jobs at 200k evals, no durable file touched, exit 0; production budgets in
§3.2). The censor-state pick-up the smoke run seeds — the sweep continuing
from the recorded censor history — is described in §4.

### 7.3 `proofdb_flip` — implied-flip analysis (read-only)

**Purpose.** Sanity check over a grown DB: for every open row whose legal
replies are all resolved (stored or implied), compute the implied outcome +
bound, iterated to fixpoint; every reported flip is then recomputed by an
independent recursive verifier. Reads the DB; writes only `--out`
(default `data/proofdb/flip.json`, created on demand).

**Prerequisites.** A merged DB and a clean unmerged-shard gate (§7.2): a
sidecar without a `shards`-table row → exit-1 refusal naming the tag —
run `proofdb_merge` first; a sidecar without a bin refuses as a
transport-layer defect; an orphan bin without a sidecar is ignored.

    usage: proofdb_flip [--db <grown.db>] [--shard-dir <dir>]
                        [--out <flip_analysis.json>]

| Option       | Default                             | Meaning                       |
| ------------ | ----------------------------------- | ----------------------------- |
| `--db`       | `data/proofdb/proofdb.db`           | the grown DB (read-only)      |
| `--shard-dir`| `data/proofdb/shards`               | shard dir for the unmerged-shard gate |
| `--out`      | `data/proofdb/flip.json`            | report JSON (overwritten)     |

**Exit codes.** `0` normally (also with 0 flips), `1` on usage error, DB
load failure (including the unmerged-shard gate), or any unverified
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

**Worked invocation.** Recipe R1 then R3 (0 flips over the standing set,
exit 0); in a production cycle the flip check is recipe R2's step 3.

### 7.4 `proofdb_verify` — whole-DB replay validation (read-only)

**Purpose.** The standing DB's integrity instrument: replay-validate the
whole stored tree read back from the DB alone (no shards needed) against
the spec's invariants — path replay legality (the stored move parses and
replays in the parent position, ply and move-code agreement), the AND/OR
structure (proven AND: the full legal-reply child set, every child `win`;
proven OR: ≥ 1 proven-loss child, bound = min loss-child bound + 1;
terminal claims statically terminal with no children), depth
monotonicity on the retreat, non-empty provenance, full reachability, and
the meta census (`schema_version`, `root_fen`, `node_count`). Recorded
limitation (accepted): a non-terminal `'exact'` minimality claim is
trusted as a search fact — not re-derivable from the DB alone.

    usage: proofdb_verify [--db <proofdb.db>] [--max-report <n>]
                          [--backup <offsite.db>]

| Option        | Default                   | Meaning                                        |
| ------------- | ------------------------- | ---------------------------------------------- |
| `--db`        | `data/proofdb/proofdb.db` | the standing DB (read-only)                    |
| `--max-report`| `20`                      | cap on printed defect lines (all are counted)  |
| `--backup`    | off                       | after a green verdict, write a `VACUUM INTO` offsite copy at the path (the recipe-R6 backup duty) |

**Exit codes.** `0` `verify: ok` (the backup, when requested, is written
only on a green verdict); `1` usage error or a DB-level failure (unreadable
file, failed meta gate); `2` `verify: FAILED n` — defects found.

**Output grammar.**

```text
verify: ok — nodes 336 (proven 334, open 2)
backup: offsite.db written
```

or, on defects (exit 2):

```text
verify: FAILED 1 (nodes 4)
defect: node 3 (g2g4): proven win without a proven-loss child
```

**Worked invocation.** Recipe R6's step 2 (`verify`) and step 3
(`--backup`); the fixture-fold DB verifies green (§7.1's census).

## 8. Recipes

### 8.1 Operator recipes

**R0 — first production start (bootstrap).** A bare fold over an empty
(default) shard dir creates the root-only DB (bare tools; everything lands
inside `data/proofdb/`):

```bash
mkdir -p data/proofdb/shards
proofdb_merge   # root-only DB at data/proofdb/proofdb.db
```

**Batch 1 — the walk boot.** The walk bootstraps the layer itself: on a
root-only DB the candidate set is empty, so the root becomes a virtual
candidate (§7.2's root rule) and the sweep expands breadth-first from
there — the layer bootstraps by running the pipeline, not by copying
artifacts. Unlike a bootstrapping enumeration the walk never retires, so
cap the session (`--max-total-evals` is the deterministic choice):

```bash
proofdb_harvest --policy frontier-walk --max-total-evals 2000000000
proofdb_merge
proofdb_flip
```

On the dev container a 2G-capped boot at the 4M base budget grew the
layer from 1 to 4,030 nodes with 49 shards in ~7 minutes (§3.2). From
batch 2 on, grow with R2. The committed fixture shard set keeps a
development/validation role only (R1, tests); the plan10 legacy-ledger
snapshot remains an optional R4 import for resuming censor history, never
part of the boot path.

**R1 — fold the fixture shard set (development, explicit flags; the
standing-layer migration).** Fold the committed fixture shard set into a
fresh DB — its legacy manifest is backfilled into sidecars — then verify
and re-fold idempotently:

```bash
cp -r docs/plans/proofdb/shards /tmp/fixture-shards
proofdb_merge --manifest /tmp/fixture-shards/manifest.json \
    --shard-dir /tmp/fixture-shards --db /tmp/fixture.db --dump nodes.txt
proofdb_verify --db /tmp/fixture.db        # expect: verify: ok — nodes 55703 …
proofdb_merge --shard-dir /tmp/fixture-shards --db /tmp/fixture.db
# expect: folded 0 shards (262 already merged …)
```

A fresh fold is id- and value-identical to the retired full rebuild (the
deterministic-fold regression is
`tests/proofdb_fold.rs::r1_fresh_fold_matches_the_rebuild_and_re_folds_idempotently`).
For the standing fixture shard set the census is the one in §7.1 and the
fold digest is
`14fd07530996eb9eddaa2b5d3e112d51d57339a892f7f8d0bb166205180a04e2` (the
pre-plan22 manifest-digest stamp was `406752cc…`; the pre-plan20 v1
digest was `0d929f4c…`). The production standing layer's own migration to
the fold world is complete (in-place fold, verify, backup, transport
deletion — 2026-10-20; §11): today's operator runs start from R0/R2, and
no standing migration recipe remains.

**R2 — grow the DB by one batch, then fold (production, bare defaults).**

```bash
# 1. Harvest (a capped walk batch; stop conditions as needed).
proofdb_harvest --policy frontier-walk --budget-evals 4000000 \
    --max-total-evals 10000000000
# 2. Fold the grown set (the harvest never merges).
proofdb_merge
# 3. Replay-validate the grown DB.
proofdb_verify
# 4. Flip sanity over the grown DB.
proofdb_flip
```

Note: after a _decisive_ batch the shard dir carries unmerged sidecars —
the fold's delta extends the DB (appended ids, refined rows, a new fold
digest in `built_from`). Two different fold sequences over the same facts
may differ in row ids only (spec invariant 7).

**R3 — check for implied flips.** Recipe R2 step 4 (`proofdb_flip`, bare
defaults); expected on a sound tree: `flips 0 verified 0`. Any flip is a
soundness signal — stop and investigate (§9), never re-merge to make it
disappear.

**R4 — import a legacy work ledger (one-shot migration).** Pre-plan20
layers (or committed snapshots) migrate their censor history into the DB:

```bash
# Migrate the standing pre-plan20 sidecar (or any legacy JSON ledger).
proofdb_merge --import-censor data/proofdb/work.json
# The committed plan10 snapshot (8,446 records) into a fresh layer:
proofdb_merge --import-censor \
    docs/plans/proofdb/measurements/plan10/ledger_union.json
```

The import merges under the per-field max rule (`work_done`/`passes_failed`
each keep the max of stored vs imported) — idempotent (a repeated import
changes nothing), and it never decreases a stored value, so importing a
stale snapshot after newer censor work is harmless. Afterwards the JSON
file can be deleted; the DB's `censor` table is the single source of truth.
(When several per-worker ledgers exist, import them one after another —
the max rule makes the order irrelevant. The former
`proofdb_ledger_union` tool is retired; parallel-harvester censor merging
is plain SQL against the table.)

**R5 — start a fresh layer (retire the censor state).** The censor table
is selection state, but the tree surfaces are the truth: **never delete
the DB to "reset" it** — that discards proven facts (R6's backup is the
only recovery). To restart the *scheduling* only, wipe the censor rows
(`DELETE FROM censor;`, §4) or re-import a legacy snapshot (R4); the tree
survives untouched.

Operators with a pre-plan14 working layer (flat paths) migrate it first:

```bash
mkdir -p data/proofdb && mv data/proofdb.db data/proofdb/proofdb.db
```

Operators with a pre-plan20 layer (a `work.json` sidecar next to the DB)
finish the migration with R4's import, then delete the sidecar. Operators
with a pre-plan22 layer (manifest-indexed shards) migrate with R1's
production-path variant: fold, verify, then delete the folded shard pairs
and the manifest (R6).

**R6 — shard deletion (operator recipe only; irreversible).** Folded
shard pairs are deletable transport (§2.1). Deletion is manual —
deliberately not one flag away — and must never precede the backup
(append-only truth: a buggy fold can no longer be excised by deleting a
shard and rebuilding):

```bash
# 1. Fold everything collected so far.
proofdb_merge
# 2. Replay-validate the whole DB.
proofdb_verify --db data/proofdb/proofdb.db          # expect: verify: ok
# 3. Write the offsite backup (the only recovery path afterwards).
proofdb_verify --db data/proofdb/proofdb.db \
    --backup /offsite/proofdb-$(date +%F).db
# 4. Now (and only now) delete the folded pairs + any legacy manifest.
rm data/proofdb/shards/*.bin data/proofdb/shards/*.meta.json \
   data/proofdb/shards/manifest.json
```

After step 4 every gate tolerates the empty shard dir: a fold skips tags
with `shards` rows, and the harvest/flip gates key on unmerged sidecars
(absent here). One hard rule survives deletion: the **directory itself
must exist** — a missing shard dir aborts every tool (`cannot read shard
dir`: merge exit 2, harvest/flip exit 1). If the dir was removed
wholesale (e.g. `rm -rf`, the migration path), recreate it first:

```bash
mkdir -p data/proofdb/shards
```

Operators with a pre-plan14 working layer (flat paths) migrate it first:

```bash
mkdir -p data/proofdb && mv data/proofdb.db data/proofdb/proofdb.db && \
    mv data/proofdb_work.json data/proofdb/work.json
```

Operators with a pre-plan20 layer (a `work.json` sidecar next to the DB)
finish the migration with R4's import, then delete the sidecar.

### 8.2 Consumer recipes (SQL)

All queries below run read-only and are verified against the standing DB
(node counts in comments are its census: 55,703 nodes = 55,628 proven +
75 open). Everything a website needs is a `WITH RECURSIVE` walk away over
`meta`/`nodes` (plus `shards` and the `idx_nodes_parent` index); schema
v2's additive `censor` and `drawn` tables (spec §2.1) are harvest
scheduling state and can be ignored entirely.

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

**Children of a node** (facts stored below it; absence ≠ decided — §5):

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
way (the path _is_ the membership proof). A line that dead-ends at a row
with `outcome IS NULL` is undecided at that point; a line that fails to
resolve leaves the stored tree.

**The open frontier** (what harvesting would open next; 75 rows here):

```sql
SELECT id, ply, group_concat(move_uci, ' ') OVER ()  -- or path via the recipe above
FROM nodes WHERE outcome IS NULL ORDER BY ply, id;
-- standing DB: 1 at ply 0 (the root), 7 at ply 1, then reply rows
-- down to ply 31 — the undecided head rows
```

The DB alone cannot say _which legal replies_ of an open row are still
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

For a _provable-claim_ line rather than a shortest one: an OR-node's
proving child is its unique `loss` child; walking OR→proving-child and
AND→any child renders a principal line ending at a
`depth_bound = 0` terminal. 24,833 terminal rows exist in the standing DB.

**Provenance and shard audit.**

```sql
SELECT provenance FROM nodes WHERE id = :nid;         -- shard tags of a row
SELECT tag, file, path, outcome, depth_bound, n_nodes
FROM shards ORDER BY tag;                             -- the provenance set as rows
```

**DB census (the three numbers to display as a data version stamp).**

```sql
SELECT (SELECT COUNT(*) FROM nodes),
       (SELECT COUNT(*) FROM nodes WHERE outcome IS NOT NULL),
       (SELECT value FROM meta WHERE key = 'built_from');
-- → 55703 | 55628 | e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd
```

## 9. Soundness contract (operator terms)

- Only validator-clean shards enter a fold (sidecar-produced shards are
  clean by construction; legacy manifest entries need `validate: "ok"`),
  and each is replay-validated again at fold time and once more as a
  re-assembled skeleton after the depth fixpoint.
- Proven-outcome conflicts between shards abort the fold (exit 2). They
  are soundness bugs, never patched over.
- The DB is the single source of truth, grown by the incremental fold —
  never rebuilt from shards. Its integrity instrument is
  `proofdb_verify`: the whole tree read back from the DB alone is
  replay-validated against the spec's invariants (recipe R2 step 3, R6
  step 2). The deterministic fold is a standing regression test
  (`tests/proofdb_fold.rs` R1/R2). Keep an offsite backup current (R6
  step 3) — after folded shards are deleted it is the only recovery.
- `proofdb_harvest`/`proofdb_flip` refuse a stale layer via the
  unmerged-shard gate: any sidecar whose tag has no `shards`-table row
  means shards were collected since the last fold — run `proofdb_merge`
  first (§10).
- Censored results are facts of absence only: no shard, no tree row, a
  `censor` row. Proven draws are still not representable as *tree facts*
  (schema v2 has no `draw` outcome), but they are no longer censored: a
  `drawn` scheduling row records the path and the walk prunes its subtree.
- Writer split (plan20): `proofdb_merge` writes `meta`/`shards`/`nodes`
  only; `proofdb_harvest` writes `censor`/`drawn` rows only (one committed
  transaction per record). Running both concurrently is an operator
  error — never fold while a harvest session is running.

## 10. Troubleshooting and known limitations

**Missing shard dir.** A shard *directory* that does not exist (as
opposed to an existing empty one) aborts every tool at startup:
`cannot read shard dir …` — merge exit 2, harvest/flip exit 1. Fix:
`mkdir -p data/proofdb/shards`. An empty dir is always valid (the
bootstrap state, R0; the steady state after R6's deletion).

**Unmerged shards (the gate).** `proofdb_harvest`/`proofdb_flip` refuse
with exit 1 and name the tag:

```text
proofdb_flip: unmerged shard h_2fd82f9a8230a668 — run proofdb_merge first
```

The shard dir carries sidecars collected since the last fold (usually a
harvest batch completed since then). Fix: run the bare `proofdb_merge`
(recipe R2 step 2; the fold commits the collected pairs into the standing
DB). The gate also refuses a **sidecar without its bin** (a
transport-layer defect — a sidecar is only written after its bin);
a **bin without a sidecar** is the interrupted job's orphan and is
ignored by every gate (the retry overwrites it determinically).

**Schema-v1 DB.** A fold over a pre-plan20 DB file aborts with an
upgrade hint (the fold never rewrites an existing file): move the old
file aside, re-run the fold over the shard set (a fresh fold recreates
schema v2), and migrate the old censor rows with `--import-censor` if
wanted (R4).

**Fold aborts with exit 2.** A CONFLICT/DEFECT abort (§7.1) leaves no
partially written state — the transaction rolls back. Read the abort
message: it names the shard/standing-DB defect. Never "fix" a
proven-outcome conflict by editing a sidecar — that is a soundness bug in
the pipeline and belongs in a bug report.

**Orphan bins after a harvest abort.** The sidecar is the commit marker
(written after its bin, atomically), so a mid-session crash or ctrl-c
(exit 2, or killed) leaves a consistent transport layer: everything
collected up to the last finished job is a complete pair and a subsequent
bare `proofdb_merge` folds it in. At most one orphan remains — the bin of
the job that was in flight (interrupted between the bin write and its
sidecar write, or whose sidecar write failed). The retry deterministically
overwrites it; no manual cleanup is needed, but a bin without a sidecar
in the shard dir is a symptom worth noting in the operator log. A fold
after any batch with decisive jobs is still required before the next
harvest session (the unmerged-shard gate above).

**A rejected export aborts the session (`ABORT: job … export rejected: …`).**
A decisive job whose proof subtree cannot be rebuilt from the retained
session TT kills the session with exit 2 (`rejected, never patched` — a
shard is only written after reconstruction *and* replay validation agree,
§9). The rejection is rare, but it is fatal by design; this section is the
whole operator story. The message names the failing node, its **abort
class**, and the walk census of the failed export:

```text
proofdb_harvest: ABORT: job g1f3 d7d6 b1c3 c7c6 f3g5 f7f6 g2g4 f6g5 f2f4:
  export rejected: BudgetExhausted: fill at root.e7e5.a2a3 censored at bound 46
  (attempt budget 2000000 evals, parent depth 46) (holes: hit=1188 … filled=0
  unfillable=0 anomalies=0, fill_evals=2011003)
```

The walk fills the holes of a proof path — nodes the original search proved
but whose TT record did not survive into the export (`--tt-mb` retention,
cross-job eviction, a clock-scan Draw miss, …). A hole below a deep parent
is bounded by that parent's proven depth (a child's proven depth is always
smaller), so a deep parent can no longer be rejected by the fixed depth
cap: only a *parentless* root hole is depth-capped, and a root hole is an
earlier failure anyway (`snapshot does not prove a decisive root`).

_The two abort classes._

- **Budget class** — `BudgetExhausted: fill at <path> censored at bound B
  (attempt budget A evals, parent depth P)` or `BudgetExhausted: fill total
  budget exhausted`. The local solve was **cut** by its child-eval budget,
  not by depth: nothing is known about the hole's true difficulty. Retrying
  with more budget is meaningful. (`attempt budget` is per fill attempt;
  the `fill_evals=` census figure is the whole export's spend.)
- **Depth/contradiction class** — `unfillable hole at <path>: no decisive
  line within bound B (cap C, parent depth P)` or one of the walk's
  `anomaly:` / `repetition-context` / `snapshot does not prove a decisive
  root` messages / `replay validation found N defects`. The bounded local
  solve ran to *completion* and produced no decisive result where the
  parent's proof requires one — a contradiction, or a structural defect in
  the reconstruction. More budget cannot fix it; a larger `--tt-mb` (fewer
  evicted records, so fewer/simpler holes) or a re-scoped session can.

_The recovery ladder_ (each rung is a manual operator action; the harvest
never retries, never auto-quarantines, and writes no recovery state — it
aborts deterministically, plan25 D7):

1. **Fold, then retry.** Every finished job's pair is on disk and the
   session's censors are committed, so `proofdb_merge` first (the batch
   made progress, and the unmerged-shard gate would refuse the retry
   anyway), then re-run the batch unchanged. A retry is *not* a
   byte-identical repeat: the retained TT differs, so a different job
   order/snapshot can fill the hole cheaply. **A decisive-but-unexported
   job leaves no record**, so the same path is re-selected next session —
   that is why this abort must be diagnosed and recovered, not ignored.
2. **Raise `--tt-mb`.** The typical cause is TT eviction inside the session
   (the hole's record was proven under an earlier job and evicted). A bigger
   retained table keeps more of the proven interior, so more holes hit
   exactly and fewer need filling at all.
3. **Escalate `--fill-attempt-budget` only on a budget-class abort**, and
   only after rung 2. Measured caveat: raising the attempt budget can cost
   *up to ~9×* more child-evals for the same holes (restart-chunk pacing:
   a 20 M attempt budget spent 60 M evals on holes a 2 M budget resolved
   with 6.7 M), so it is the escalation of last resort, not a free knob.
   `--fill-total-budget` (default 100 M) remains the guard that keeps an
   export from costing more than the search it exports.
4. **Quarantine the path (censor-row)** — a manual, soundness-neutral
   scheduling edit that makes the walk skip the path for one session:

   ```sql
   INSERT OR REPLACE INTO censor(path, work_done, passes_failed)
   VALUES ('g1f3 d7d6 b1c3 c7c6 f3g5 f7f6 g2g4 f6g5 f2f4', 100000000, 0);
   ```

   `work_done` = the session budget you are about to run (`B`), so the
   record counts as worked and its subtree is expanded instead of
   re-searched; `passes_failed = 0`. `censor`/`drawn` rows are
   replay-independent scheduling hints (spec §2.1) — deleting or writing
   them costs re-searched work, never soundness. This is the one
   sanctioned operator-side write to scheduling state (the tool itself
   remains its only automatic writer, and `nodes`/`shards`/`meta` stay
   `proofdb_merge`-only; never touch those). Remove the row
   (`DELETE FROM censor WHERE path = …`) once the real cause is fixed.
5. **Re-scope with `--moves '<uci path>'`** — run the rest of the territory
   under a different anchor (§7.2 scoped harvest), leaving the offending
   path out of the session's world.

_The three-strike quarantine rule_ (pre-registered; this is the decision
rule, not a suggestion):

1. **Strike 1 — a path aborts.** Fold, then retry (rung 1); if it is a
   budget-class abort, rung 3 applies after rung 2.
2. **Strike 2 — the same path aborts a second time.** Quarantine it
   (rung 4) and retry the session with a larger `--tt-mb` (rung 2). Do not
   raise the attempt budget a second time for the same path.
3. **Strike 3 — a third distinct path aborts the same way.** **Stop the
   campaign.** Do not retry further and do not quarantine a third path:
   write a report (the three paths, their classes, messages, and censuses,
   plus the session configuration) and treat it as a design finding. The
   skip-and-censor successor is parked as backlog item 23 and is drafted
   only when this rule fires.

A quarantined path stays quarantined until its cause is understood; the
quarantine row is a scheduling hint, so deleting it later costs only
re-searched work.

Never edit a shard, a sidecar, or the DB to "fix" an export abort — a
rejected export writes nothing, and a proven fact is append-only truth
(§9). Never raise `--fill-attempt-budget` to hide a depth/contradiction
abort: it will cost more and fail the same way.

**Fresh vs carried censor state.** Empty censor rows (`DELETE FROM
censor;`, §4) are sound but re-censor every frontier path at the base
budget on first revisit — lower first-batch yield. Importing a legacy
snapshot (`proofdb_merge --import-censor`, R4) recovers the censor
history. The censor state never changes DB facts; getting it wrong costs
work, not soundness. A schema-v1 DB (no `censor` table) aborts harvest
with an upgrade hint — re-run the bare `proofdb_merge` first.

**Fold-digest drift after growth.** Recipe R1's pins (fold digest
`14fd0753…`, 55,703 nodes) describe the _standing fixture_ shard set.
After a decisive harvest batch the fold digest changes — that is growth,
not drift. The fold digest is a provenance-set stamp (spec §2.2), not a
rebuildability stamp: the DB is never rebuilt, so there is no "rebuild
and compare" anymore — the replacement checks are the deterministic-fold
regression tests (same standing DB + same shard set ⇒ identical content,
`tests/proofdb_fold.rs` R1/R2) and `proofdb_verify`.

**A lost or corrupted DB after shard deletion.** With folded shards
deleted, the DB is the only copy of its tree (append-only truth): restore
from the offsite backup (recipe R6 step 3 is the mitigation's whole
point) and re-fold whatever shards remain. Without a backup the facts are
gone — this is why R6's ordering (fold → verify → backup → delete) is
manual and mandatory.

**Startpos-rooted only.** Subtree-scoped harvesting (`--root-fen`) is
parked (initiative item 7); every job replays from the standard start
position.

**Fold is a separate step.** `proofdb_harvest --out-db/--dump` only
record the intended targets in the summary; after each batch, run
`proofdb_merge` yourself (recipe R2).

**Stale restored backup.** A DB restored from an old backup is invisible
to every gate if its shard dir was already emptied (no unmerged sidecars
to compare against) — the gates check the dir↔`shards`-table delta, and
an old DB + empty dir looks consistent. The mitigation is the backup
duty itself: keep the offsite copy current (R6), and stamp checks against
the fold digest when in doubt (§8.2's census query).

## 11. History

plan25 (2026-10-27) made deep-hole exports recoverable without changing the
"a rejected export aborts the session" rule: a hole's fill bound is seeded
from its parent's proven depth and now dominates the fixed depth cap (the
cap governs parentless root holes only), the doubling search clamps at the
cap instead of overshooting it, and a budget-censored fill attempt is
reported as a budget abort instead of masquerading as depth exhaustion. The
harvest gained the export fill budget knobs `--fill-attempt-budget` (default
2,000,000) and `--fill-total-budget` (default 100,000,000) — the values it
previously hardcoded — and a rejected export's abort line carries the walk
census (`holes: …`, `fill_evals=N`). §10 documents the two abort classes,
the recovery ladder, and the three-strike quarantine rule. The incident it
fixes: a production session aborted on a mate-in-1 hole under a depth-46
parent with `fill depth cap 32 exhausted` and **zero** fill evals.

plan23 (2026-10-20) completed the standing layer's migration to the fold
world and retired the last transitional wording: the parked pre-plan22
transport (`shards-bak/`, 405 manifest-era bins + legacy manifest) was
folded in place over the standing DB — a no-op fold that re-stamped
`built_from` to the fold digest `47b25861…` with `nodes`/`censor`/`shards`
rows byte-equal before/after (43,092 nodes = 43,028 proven + 64 open) —
then verify + `VACUUM INTO` offsite backup, transport archived
to `data/proofdb-backups/` and deleted, the empty production shard dir
recreated (a missing dir aborts every tool — §10), and bare post-checks
green. The external handoff package (`measurements/plan13/handoff/`) was
re-cut against the migrated DB under the v2 spec (superseding the
pre-migration 10-16 re-cut); the spec's §2 subsection numbering, the
`shards` table's permanent-provenance-record statement, and the §6
sidecar-anatomy provenance phrasing were made coherent with the v2
contract. The DB is now the standing layer in its final fold-world shape:
no manifest, no backfill, no migration recipe left (R1 keeps the fixture
role only).

plan22 (2026-10-16, the durable-layer pivot's step 2) made the **DB the
single source of truth**: `proofdb_merge` became an incremental fold
(one transaction per invocation; existing row ids preserved forever,
new ones appended; a fresh fold assigns global lexicographic-path order,
id-identical to the retired rebuild), shards became self-describing
`<tag>.bin` + `<tag>.meta.json` sidecar pairs (the sidecar is the commit
marker — a bin without one is the interrupted job's orphan) and are
deletable after their fold, the manifest was retired to a legacy
migration source (`--manifest` backfills sidecars; `proofdb_harvest
--manifest` refuses), the byte-identical-rebuild contract was replaced by
`proofdb_verify` (whole-DB replay validation from the DB alone) plus the
`VACUUM INTO` offsite-backup duty (recipe R6's fold → verify → backup →
delete), and the `built_from` manifest-digest staleness gate was replaced
by the unmerged-shard detection (§10). `built_from` is now the fold
digest over the `shards` table (spec §2.2). Fixture fold digest:
`14fd0753…` (55,703 nodes; the pre-plan22 manifest stamp was `406752cc…`).

plan20 (2026-10-15, the durable-layer pivot's step 1) moved the censor
state into the DB: schema v2 adds the additive `censor` table (spec §2.1),
the harvest commits one transaction per censor and writes nothing else in
the DB, and the merger preserves `censor` rows across its full rebuild
(`--import-censor` migrates a legacy JSON ledger under the per-field max
rule). The `work.json` sidecar and the `proofdb_ledger_union` tool are
retired; the `--ledger` flag aborts with a migration hint. The fixture
rebuild digest was re-pinned (`406752cc…`; pre-plan20 `0d929f4c…`).

plan18 (2026-10-14, owner request) consolidated the harvest policies:
`frontier-walk` is now the only one. The `breadth-pns`, `and-close`,
`descend`, and legacy-gradient policies (plans 2–15) were removed — every
new harvest feature had to work and be tested against all of them, an
unreasonable maintenance cost for surface the owner does not use. The
walk's boot role (§3.2, R0 batch 1) absorbs the cold start; the flat
budget replaces the ladder as the deepening mechanism. The job-JSON
grammar dropped `parent_bound` (C2-only) and `number` (PNS-only); the
`--pns-config`, `--and-close-*`, `--descend-plies`, and `--heavy-*`
flags are gone. History below describes the removed policies as they
were; §-references point at the pre-plan18 layout.

plan17 (2026-10-13) made the harvest session crash-consistent: the
manifest is atomically rewritten (tmp + rename, the ledger's pattern)
after every decisive job — no signal handling, no format change — so an
unexpected crash or ctrl-c loses at most the in-flight job's work and the
next bare `proofdb_merge` folds in everything collected up to the last
finished job (§3.2 operational notes, §7.2 exit codes, §10 orphans).

plan16 (2026-10-13) added the `frontier-walk` harvest policy — the
shallowest-first ledger walk with a flat budget, an alternative grow sweep
beside the then-dominant `and-close` cycle, composing through the shared
ledger; the plan16 boot gate doubled as the walk-only walkthrough (now
§3.2).

The pipeline was built and hardened by the `proofdb` initiative
(`docs/plans/proofdb/`); the tooling-completeness audit that produced this
manual's first version is plan12 (2026-10-02). plan13 (2026-10-02) extended
it into the operator **and user** manual — data-model primer (§5), shard
provenance (§6), per-tool output grammars with worked invocations (§7),
operator + consumer SQL recipes (§8), troubleshooting (§10) — for the
website handoff package (`measurements/plan13/handoff/`), which packages a
digest-pinned rebuilt DB with a consumer-facing `HANDOFF.md`. Per-plan
measurements and the pinned audit results live under
`docs/plans/proofdb/measurements/`.

plan14 (2026-10-02, owner request) centralized all generated proofdb state
under `data/proofdb/` by default: every tool's `--db`, `--manifest`,
`--shard-dir`, `--ledger`, and `--out` now default there (previously
`proofdb.db` in the cwd, required manifest/shard-dir flags, and the flat
`data/proofdb_work.json`), making a production run flag-free. The
committed shard set at `docs/plans/proofdb/shards/` was reclassified as a
development/validation fixture (explicit flags only); backup of the
unversioned production durable layer became the operator's responsibility
(§2.1). Migration one-liner in §4/R5.

plan15 (2026-10-08) made the pipeline self-bootstrapping: the `descend`
policy (`--policy descend`, `--descend-plies`, default 2, range 1..=3)
enumerates all legal startpos-rooted candidate paths of length 1..=K,
skips those covered by stored territory, and bounded-solves the rest
under the standard ladder — the production boot is now R0 + a bare
`descend` batch instead of artifact-copying the committed fixture (§3.2,
R0; the former §3.2 step 0 / §8.1 seeding recipe is retired). The
reference boot grew a fresh layer to 4,314 nodes (52 shards) with
`flips 0 verified 0`; determinism gate: a second boot reproduced manifest
digest and ledger bytes identically.
