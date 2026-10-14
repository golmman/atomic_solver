# proofdb pipeline — operator and user manual

How to run, consume, and extend the startpos proof-line database pipeline
(shards → merger → DB → harvest loop) as shipped in this repository. The four
tools live under `examples/` (`proofdb_merge`, `proofdb_harvest`,
`proofdb_flip`, `proofdb_ledger_union`); the DB schema and its semantics are
the sole external contract and are normative in
[`docs/spec/global_proof_store.md`](spec/global_proof_store.md). This manual
serves two readers:

- the **operator**, who runs the pipeline (merge, harvest, flip, ledger
  maintenance) — §1–§4, §7, §9, §10;
- the **user/consumer** (e.g. the website project), who reads the finished
  SQLite file and wants to understand its shape and query it — §5, §6, §8
  (plus the normative spec).

## Contents

1. [Pipeline map](#1-pipeline-map)
2. [Artifacts and tools](#2-artifacts-and-tools)
3. [Quickstart](#3-quickstart)
   - [3.1 Initial run](#31-initial-run)
   - [3.2 Production run — the walk batch cycle](#32-production-run--the-walk-batch-cycle)
4. [Ledger pick-up](#4-ledger-pick-up)
5. [Data model primer (consumer)](#5-data-model-primer-consumer)
6. [Shard provenance — where a shard comes from](#6-shard-provenance--where-a-shard-comes-from)
7. [CLI reference (operator)](#7-cli-reference-operator)
   - [7.1 `proofdb_merge`](#71-proofdb_merge--shards--sqlite-single-writer)
   - [7.2 `proofdb_harvest`](#72-proofdb_harvest--grow-the-shard-set)
   - [7.3 `proofdb_flip`](#73-proofdb_flip--implied-flip-analysis-read-only)
   - [7.4 `proofdb_ledger_union`](#74-proofdb_ledger_union--n-way-ledger-merge)
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
  shards + manifest ──proofdb_merge──► proofdb.db
          ▲                                │
          │                                ├──proofdb_flip──► flips.json
          └──── proofdb_harvest ◄──────────┘
                       ▲ │
                       │ ▼
                  work ledger ──proofdb_ledger_union──► work ledger
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

| Artifact    | Layer           | Default path                                  | gitignored |
| ----------- | --------------- | --------------------------------------------- | ---------- |
| shard set   | durable truth   | `data/proofdb/shards/*.bin` + `manifest.json` | yes        |
| SQLite DB   | derived view    | `data/proofdb/proofdb.db`                     | yes        |
| work ledger | selection state | `data/proofdb/work.json`                      | yes        |
| flip report | analysis output | `data/proofdb/flip.json`                      | yes        |

Because `data/` is gitignored, the durable layer now lives **outside
version control**: backing up `data/proofdb/shards/` is the operator's
responsibility (the manifest digest, `built_from`, is the integrity stamp
for any backed-up copy). Nothing else in this layout is irreplaceable:
the DB is derived, the ledger is scheduling state, the flip report is a
throwaway.

- **Shard set (durable truth).** `*.bin` validated binary proof trees
  (`proof_tree_dump.md` v1 format) plus `manifest.json`, one entry per shard
  (path, fen, outcome, tag, `validate`). Append-only: only
  `proofdb_harvest` adds to it, and `proofdb_merge` only reads it.
- **SQLite DB (derived view).** `proofdb.db`, schema v1 per the spec.
  Rebuildable deterministically and byte-identically from the shard set;
  `proofdb_merge` is its sole writer.
- **Work ledger (selection state).** Per frontier path: the censor history
  (`passes_failed`, `work_done`). Scheduling state only — it never enters
  the DB or the durable layer.
- **Flip report (analysis output).** `proofdb_flip`'s JSON verdict. A
  throwaway report, not part of the pipeline's persisted state.

### 2.2 Tools

- **`proofdb_merge`** — _shards + manifest → DB_, single writer. Replay-
  validates every shard, grafts it at its manifest path, applies the depth
  fixpoint, and writes the DB; any conflict or defect aborts without
  writing. Deterministic for a given shard set.
- **`proofdb_harvest`** — _DB + ledger → shards + manifest + ledger_.
  Selects jobs with the shallowest-first ledger walk (`frontier-walk`,
  the single harvest policy since plan18) — solves each under a
  deterministic child-eval budget, exports and validates the decisive
  ones as shards, and records censored jobs in the ledger. It never
  merges.
- **`proofdb_flip`** — _DB → flip report_, read-only. Iterates implied
  outcomes to fixpoint over the stored tree and re-checks every flip with an
  independent verifier; writes only the report.
- **`proofdb_ledger_union`** — _N ledgers → one ledger_. Deterministic
  max-rule merge (highest `passes_failed`, ties broken by `work_done`).

Full CLI options, output grammars, and exit codes are in §7; ledger pick-up
semantics are in §4.

**Production vs development.** _Production_ runs use the bare defaults:
every artifact inside `data/proofdb/`, the committed fixture ignored.
_Development/validation_ runs (tests, measurement batches, fixture work)
pin **explicit** `--manifest`/`--shard-dir`/`--db` flags against
`docs/plans/proofdb/shards/` — as every plan measurement did. The two
modes never mix by accident: defaults never touch the fixture, and
explicit flags never touch `data/proofdb/`.

## 3. Quickstart

This walkthrough bootstraps a **production-shaped** working layer from
nothing: no shards, no manifest, no DB, no ledger. It uses only the bare
defaults — every artifact lands inside `data/proofdb/` (gitignored),
nothing under `docs/plans/` is touched, and cleanup is a single
`rm -rf data/proofdb`. Run it from the repo root. §3.1 prepares everything
the pipeline needs — bootstrap merge plus a deliberately tiny smoke
harvest; §3.2 grows the database batch by batch from exactly the state
§3.1 leaves behind. The single harvest policy throughout is the plan16
`frontier-walk` shallowest-first ledger walk (the plan18 policy
consolidation removed the other policies; §7.2).

### 3.1 Initial run

```bash
cargo build --release --examples

# The empty manifest is the bootstrap seed (these exact bytes; the
# built_from digest is taken over them).
mkdir -p data/proofdb/shards
printf '{"entries":[]}' > data/proofdb/shards/manifest.json

BIN=target/release/examples

# 1. Bootstrap merge: no shards → a root-only DB.
"$BIN/proofdb_merge"

# 2. Smoke harvest: 2 jobs, tiny budget — both are expected to censor.
"$BIN/proofdb_harvest" --budget-evals 200000 --max-jobs 2
```

Expected bootstrap-merge output — a tree with just the open root:

```
census: shards merged 0/0; shard tree nodes 0; overlay new 0; dedupes 0;
        open-ancestor insertions 0; open nodes upgraded 0; merged nodes 1
        (proven 0, open 1); skeleton re-validations 0
node arithmetic: 1 = 1 root + 0 overlay + 0 ancestors; shard nodes 0 = 0 new + 0 deduped
db: data/proofdb/proofdb.db (nodes 1), manifest built_from d801aa1fb7ddcc33
```

Expected harvest stderr — the walk census reports an empty ledger with the
root open (`root open`; the virtual-root rule, §7.2), and the DB line names
the default path:

```
harvest: db data/proofdb/proofdb.db (1 rows, built_from d801aa1fb7ddcc33) — policy frontier-walk over ledger data/proofdb/work.json
frontier-walk: records 0 (lineage 0, ancestor 0, out_of_scope 0, complete 0) — root open; base budget 200000; scope global
```

then two `job: {…}` lines (stdout): the virtual-root **search visit**
(class `C1`, `kind: "search"`) and the **expansion visit** into the root's
first unhandled child `a2a3` (class `C3`, `kind: "expand"`), both
`"outcome": "censored"`, `"pass": 1`, `"work_before": 0`, and

```
harvest: policy frontier-walk stop=max-jobs jobs 2 decisive 0 censored 2 evals 400015 new_shards 0 wall 0.1s
```

(exit 0; `wall` and the `wall_s` fields are host-dependent, the counts are
deterministic). Censored jobs write no facts, so
`data/proofdb/shards/manifest.json` is **unchanged**
(`built_from d801aa1fb7ddcc33…`) and `data/proofdb/work.json` now holds
two censor records — the root and `a2a3`, each at the 200k flat budget.

Re-running the same command continues the sweep instead of repeating the
two visits: both records are now at the budget, so the walk expands into
their first not-yet-handled children (§7.2) — the ledger pick-up (§4)
carries the sweep forward.

A _decisive_ harvest instead changes the manifest digest, so its new
shards must be folded in before the next pass: run step 1
(`proofdb_merge`) again after the harvest, or `proofdb_harvest` aborts on
the stale `built_from` (§10). The smoke budget above deliberately
censors, so a follow-up batch runs against the same DB without an
intermediate merge.

The layer is now ready for §3.2: the bootstrap merge produced the root-only
DB, and the smoke harvest verified the bare defaults end-to-end without
writing any fact.

### 3.2 Production run — the walk batch cycle

**Prerequisite — a bootstrapped layer.** §3.2 continues from exactly the
state §3.1 leaves behind: the empty manifest + root-only DB at the default
paths (on a fresh machine without even that state, recipe R0, §8.1,
creates it). **Never re-create the empty manifest on an existing layer**
— overwriting `manifest.json` discards the durable shard index (§2.1).
Set `BIN=target/release/examples` as in §3.1.

**The batch cycle.** The walk is a live selection loop that never retires
— on an undecided layer it always has work — so every session needs a
stop condition; `--max-total-evals` is the deterministic choice:

```bash
$BIN/proofdb_harvest --policy frontier-walk --budget-evals 10000000 \
    --max-total-evals 10000000000 --tt-mb 1024
$BIN/proofdb_merge    # fold the batch's shards into the DB (the harvest never merges)
$BIN/proofdb_flip     # soundness sanity over the grown DB (expect: flips 0 verified 0)
```

You can gracefully stop the execution via `touch STOP` (see the
`--stop-file` option). Crashes or ctrl-c lose at most the in-flight job's
work and leave at most one unreferenced orphan shard (plan17; §7.2, §10).

Repeat the cycle. After a batch with decisive jobs the manifest digest
changes — that is growth, not drift (§10) — and the bare merge before the
next session is mandatory (the stale-`built_from` guard).

**The reference boot.** On the reference host, a 2G-capped boot over a
root-only layer (plan16's boot gate, budget 4,000,000) ran
`stop=budget jobs 553 decisive 53 censored 500 evals 2001311002
wall 216.2s` — 53 decisive shards, no flags beyond the policy and the
cap. The boot's 553 jobs are 1 search visit + 552 expansion visits: after
the root censor, _expansion replaces deepening_ — the sweep is
breadth-first by `(ply, path)`, each candidate running to completion
before the next at its ply. The boot grew the layer to **6,706 nodes**
(1 root + 6,647 overlay + 58 open-ancestor insertions, 6 open rows) with
`flips 0 verified 0`; a second boot in a fresh layer reproduced the
manifest digest, the ledger bytes, all 53 shards, and the merged DB
byte-identically (the determinism gate).

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
ledger growth and the session-start census (which ledger paths are
already priced, which are fresh), not from one batch's shard count.

**Cold start.** The walk boot itself is the cold start: recipe R0
(empty manifest + bare merge) plus a capped walk session grows a layer
from nothing — no artifact-copying, no fixture dependency. The committed
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
  never exceeds the label — so 128 MB now means ≤ 112 MiB). The
  canonical cycle uses 1024. Every job runs with a fresh TT, so this
  bounds peak RAM per job; TT size measurably affects large budgets (a
  multi-billion-eval visit can censor at 128 MB that survives at 1 GB),
  so give deep sweeps headroom.
- **`--ledger`** — pick-up state (§4). Seed it from a committed snapshot
  to avoid re-censoring known replies; a missing ledger is a sound fresh
  start at lower first-batch yield.

Operational notes:

- Jobs are deterministic: fresh TT per job, fixed child-eval budgets, no
  wall clock in any decision. Given the same DB and ledger state, the
  same command reproduces the same job sequence; the summary's `wall` is
  the only host-dependent number (§7.2).
- Censor records are saved to the ledger atomically per censor, and the
  manifest is atomically rewritten after every decisive job (plan17: tmp
  + rename) — after every finished job the durable layer (shards +
  manifest) is consistent, so an unexpected crash or ctrl-c loses at
  most the in-flight job's work (§7.2, §10). The durable layer
  `data/proofdb/shards/` is unversioned — back it up (§2.1).
## 4. Ledger pick-up

**Ledger pick-up semantics:** a _missing_ ledger file loads as an empty
(fresh) ledger — a new working layer starts a harvest from scratch without
any setup. The default ledger path is `data/proofdb/work.json` (plan14;
previously the flat `data/proofdb_work.json`). Two ways to come back to
the standing state:

1. **Fresh** (sound by construction): every frontier path is re-censored
   at the budget before the walk resumes; lower yield on the first
   revisit, no other effect. This is what the initial run (§3.1) does.
2. **Seed from the committed snapshot**: copy
   `docs/plans/proofdb/measurements/plan10/ledger_union.json` (8,446
   records) to `data/proofdb/work.json`, or pass it via `--ledger`. Use
   `proofdb_ledger_union` to merge several ledger inputs (e.g. per-worker
   ledgers, or a snapshot plus newer work) into one.

Operators with a pre-plan14 working layer migrate it in one line:

```bash
mkdir -p data/proofdb && mv data/proofdb.db data/proofdb/proofdb.db && \
    mv data/proofdb_work.json data/proofdb/work.json
```

Renaming does not affect the `built_from` staleness guard (digest-based,
not path-based).

The ledger is deliberately **not** part of the durable layer: it is
scheduling state, and its durable residue is the committed snapshots under
`docs/plans/proofdb/measurements/`.

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
are assigned in lexicographic path order (spec invariant 7), so the root is
`id = 0` and a prefix path always sorts before its extensions.

**Partial AND/OR tree.** `outcome` is from the _side-to-move_ perspective:

| `outcome` | role     | what the tree stores below it                                                                                              |
| --------- | -------- | -------------------------------------------------------------------------------------------------------------------------- |
| `'win'`   | OR-node  | exactly one proving child (a `'loss'` child) — the refutation of the opponent's best try; the other replies are _not_ rows |
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
  represented as `NULL`; no `draw` outcome exists in v1.

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

**The `meta` table.** `schema_version=1`, `root_fen`, `node_count`
(rows in `nodes`), `built_from` (the SHA-256 of the manifest the DB was
built from — identifies which shard set the DB reflects), `generator`
(free-form producer identification). Consumers should surface `built_from`
as a version stamp of the data they display.

## 6. Shard provenance — where a shard comes from

A **shard** is one validated binary proof tree (`proof_tree_dump.md` v1
format) rooted at a job position, plus its manifest entry. Shards are the
durable truth; the DB is derived from them. This section explains how a
shard comes to exist and how to inspect one — useful to an operator auditing
the durable layer, and to a consumer who wants to know what `provenance`
tags refer to.

**Production path (all inside `proofdb_harvest`):**

1. The walk selects a frontier path from the work ledger + merged DB
   (§7.2).
2. The job replays that path from the startpos, then runs a bounded
   solver session on the reached position (fresh TT per job, deterministic
   child-eval budget).
3. **Decisive result** → the solver's proof-tree reconstruction is exported
   as `*.bin`, replay-validated, and appended to the manifest; the census
   `job:` line carries the `tag` and `shard_nodes`.
   **Censored result** (draw from any cause, budget exhausted) → no shard,
   no manifest entry, a ledger record only.
4. After the batch, run `proofdb_merge` (§8.1, recipe R2): every shard is
   replay-validated _again_ at merge time, grafted at its manifest path,
   and only then becomes DB rows.

**Manifest entry anatomy** (one standing-set entry, verbatim):

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
  "tag": "h_013861d97dce5d48",
  "validate": "ok"
}
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

## 7. CLI reference (operator)

Every tool: `1` = usage error, `2` = defect abort where applicable, `0` =
success. Options are shown with their defaults; "required" options have no
default.

### 7.1 `proofdb_merge` — shards → SQLite (single writer)

**Purpose.** Build (or rebuild) the derived DB from a manifest + shard dir.
The only writer of the DB; deterministic and byte-identical for an
identical shard set. Never merges anything else; never patches conflicts.
All three paths default into `data/proofdb/` (plan14) — invoked bare it is
a production merge; the committed fixture is consumed only via explicit
flags (recipe R1).

**Prerequisites.** A manifest whose entries all say `validate: "ok"`, and
the shard files they name, readable from `--shard-dir`. No DB is needed —
`--db` is _overwritten_. A missing default manifest aborts cleanly naming
`data/proofdb/shards/manifest.json` (bootstrap: create the empty manifest,
§3); reading never creates anything, only the DB write creates
`data/proofdb/` as needed.

    usage: proofdb_merge [--manifest <index.json>] [--shard-dir <dir>]
                         [--db <out.db>] [--dump <nodes.txt>] [--sample-lines <n>]

| Option           | Default                             | Meaning                                            |
| ---------------- | ----------------------------------- | -------------------------------------------------- |
| `--manifest`     | `data/proofdb/shards/manifest.json` | manifest JSON (entries sorted by tag internally)   |
| `--shard-dir`    | `data/proofdb/shards`               | directory holding the `*.bin` shard files          |
| `--db`           | `data/proofdb/proofdb.db`           | output SQLite path (overwritten)                   |
| `--dump`         | off                                 | canonical text dump of the merged tree             |
| `--sample-lines` | `0`                                 | print `n` random root-to-leaf walks as spot-checks |

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
db: data/proofdb/proofdb.db (nodes 55703), manifest built_from e91ad57b44008fee
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
- with `--sample-lines n` (printed _before_ the census): `n` lines like
  `sample 1: e2e4 h7h6 d1h5 e7e5 h5f7 (5 plies, end: loss bound 0 (exact))`
  — random root-to-leaf walks as spot-checks.

**Worked invocation.** Recipe R1 (standing fixture shard set, executed
2026-10-02 with explicit flags — the census above is verbatim); the
production bootstrap merge is in §3.1.

### 7.2 `proofdb_harvest` — grow the shard set

**Purpose.** Select frontier jobs, solve them under deterministic
child-eval budgets, and append validated shards + manifest entries for the
decisive ones. Censored jobs (no decisive result, budget exhausted) record
a fact of absence in the work ledger only.

**Prerequisites.** A merged DB whose `meta.built_from` equals the
manifest's digest (stale pair → clean abort); a writable shard dir (the
default `data/proofdb/shards/` is created on the first shard or manifest
write); a ledger path (missing file = fresh ledger, §4). A missing
default manifest aborts cleanly naming
`data/proofdb/shards/manifest.json` (§3 bootstraps it).

    usage: proofdb_harvest [--db <proofdb.db>] [--manifest <manifest.json>]
                           [--shard-dir <dir>] [--policy frontier-walk]
                           [--ledger <path>] [--budget-evals <n>]
                           [--max-total-evals <n>] [--tt-mb <mb>]
                           [--max-jobs <n>] [--max-runtime <s>]
                           [--stop-file <path>] [--moves '<uci path>']
                           [--out-db <grown.db>] [--dump <nodes.txt>]

| Option               | Default                             | Meaning                                                                             |
| -------------------- | ----------------------------------- | ----------------------------------------------------------------------------------- |
| `--db`               | `data/proofdb/proofdb.db`           | the merged DB to work against (read-only)                                           |
| `--manifest`         | `data/proofdb/shards/manifest.json` | manifest to extend (rewritten atomically after every decisive job and at batch end) |
| `--shard-dir`        | `data/proofdb/shards`               | where new shard files are written (created on first write)                          |
| `--policy`           | `frontier-walk`                     | kept for command-line compatibility; only `frontier-walk` is accepted (plan18)      |
| `--ledger`           | `data/proofdb/work.json`            | pick-up state; a missing file is a fresh ledger                                     |
| `--budget-evals`     | `0` → base `4,000,000`              | the flat per-visit child-eval budget `B` (no ladder)                                |
| `--max-total-evals`  | `0` (unlimited)                     | session cap across jobs                                                             |
| `--tt-mb`            | `128`                               | per-session transposition table (MB)                                                |
| `--max-jobs`         | `0` (unlimited)                     | stop after n jobs                                                                   |
| `--max-runtime`      | `0` (unlimited)                     | stop after n seconds                                                                |
| `--stop-file`        | `STOP`                              | batch stops (gracefully, after the in-flight job) when this file appears            |
| `--moves`            | off (global session)                | scoped harvest: whitespace-separated UCI moves naming a startpos path — the session runs with its world rooted at that node (selection only: same ledger, DB, shards, manifest); the path is replay-validated before any state is loaded (nonsense or prefix-illegal → usage exit 1); `--moves ''` is rejected (the flag's absence already means global) |
| `--out-db` / `--dump`| off                                 | recorded in the summary only — **the caller runs `proofdb_merge` separately** (by design; this CLI never merges) |

**Selection policy — `frontier-walk` (plan16; the only policy since the
plan18 consolidation).** A live selection loop over the work ledger plus
the merged DB's rows; the frontier mutates between iterations, so there
is no precomputed job sequence. The mechanism, in operator terms:

- **Candidates** are the ledger records, minus records whose path (or a
  proper prefix, the empty prefix included) is a *proven* DB row —
  lineage/ancestor drops, the pick-up gate — and records decided or
  complete-marked earlier in the session. Selection is the shallowest
  candidate first, `(ply, path)`-lexicographic.
- **Search visit** (candidate with `work_done < B`): replay the path,
  search at the flat budget `B` (retained TT, so a revisit is never a
  byte-identical repeat); decisive → shard pipeline; censored → bump the
  record's `work_done`/`passes_failed` and save the ledger atomically.
- **Expansion visit** (candidate with `work_done ≥ B`): enumerate the
  legal children in lex UCI order and search the first *unhandled* one at
  `B`. *Handled* = a stored DB row (any outcome), a manifest/shard path,
  an in-session shard path, or a ledger record already at `≥ B` — so the
  sweep walks *down* the tree breadth-first instead of deepening a censored
  node.
- **Complete rule**: a candidate with `work_done ≥ B` and no unhandled
  child is complete-marked (a pick-only iteration, no evals); each
  candidate completes at most once, so the loop cannot stall. A censor
  that spends zero child-evals (a non-budget draw, e.g. an on-path
  repetition) is also complete-marked.
- **Root rule**: when the candidate set is empty, the scope root (see the
  next bullet) becomes a virtual candidate (this is how a root-only layer
  bootstraps, §3.2) — unless the scope root is proven-stored, in which
  case the session stops `exhausted`.
- **Scoped harvest (`--moves`)**: `--moves 'a2a4 b7b5 b1a3'` re-anchors
  the session at the node reached by replaying the moves from the
  startpos — the scope node *is* the walk's root (a re-anchor, not a
  filter): the virtual-root bootstrap, the lineage/ancestor drops, the
  ordering, and the census all evaluate scope-relative values, so no
  budget is spent above the scope and no in-scope record is silently
  excluded behind settled territory. Out-of-scope ledger records are
  never candidates, never visited, and never written — but stay in the
  shared ledger untouched (censors only bump visited nodes), so later
  global sessions pick them up unchanged; they are counted in the census
  as `out_of_scope`. Proven DB rows *above* the scope root are ignored
  (the operator chose the scope deliberately); a proven-stored scope root
  (a proven DB row or a manifest/shard path at the scope key) ends the
  session `exhausted` with an explanatory stderr note — no work is done
  beneath proven nodes. Everything else (budget, caps, stop file, ledger,
  manifest, shards) keeps its tool-wide semantics, and the searches and
  their results are identical wherever the walk starts (every job replays
  the full startpos path).
- **No ladder**: a node censored at `B` is never re-searched deeper by
  this policy. Raising `--budget-evals` between sessions is the
  deepening mechanism (underworked records are re-searched once at the
  new `B`, §3.2). The walk never retires: on an undecided layer it always
  has work, so a session needs a cap (`--max-total-evals`, `--max-jobs`,
  …) to stop.

**Exit codes.** `0` batch completed (then the batch-end manifest line is
printed); `1` usage error; `2` ABORT on any defect. The manifest is
atomically rewritten (tmp + rename) immediately after every decisive
job's shard write, so after every finished job the durable layer is
consistent: every `*.bin` written so far is referenced by the on-disk
manifest, and an unexpected crash or ctrl-c loses at most the in-flight
job's work — at most one unreferenced orphan shard remains (from a job
interrupted between the shard write and its manifest entry, or whose
manifest write failed), deterministically overwritten by the retry. There
is no signal handling: the stop conditions below stay the only graceful
exits; this contract only narrows what an unclean exit costs. Stop
reasons in the summary: `budget` (`--max-total-evals` cap reached),
`max-jobs`, `max-runtime`, `stop-file`, `exhausted` (no candidate can
yield a visit). All stop conditions are checked between jobs only — a
started job always runs to its granted budget.

**Output grammar.**

_stderr, session start_ (informational; the numbers are the ledger-frontier
state):

```text
harvest: db data/proofdb/proofdb.db (55703 rows, built_from e91ad57b44008fee) — policy frontier-walk over ledger data/proofdb/work.json
frontier-walk: records 8446 (lineage 6, ancestor 0, out_of_scope 0, complete 0) — root open; base budget 200000; scope global
```

The census line reports: total ledger records, the lineage/ancestor drops
(records on or behind a proven DB row — not candidates), the out-of-scope
records (outside the session scope — never candidates, preserved for later
global sessions; `0` for a global session), the in-session complete marks
(0 at session start), the root state of the session's scope root (`root
open` / `root proven` / `root fresh`), the flat budget, and the scope (the
`--moves` path, or `global`; the reference boot on a root-only DB prints
`records 0 … — root open; base budget 4000000; scope global`). When
scoped, the `harvest: db` echo above carries `; scope <path>` as well.

_stdout, per job_ — one `job: {…}` JSON line per visit. Field table (as
emitted by `examples/proofdb/session/record.rs`; `pass`/`work_before`/
`kind` are the walk census and are omitted on a non-census record):

| field          | meaning                                                                                                                             |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `path`         | UCI path of the job position (space-separated)                                                                                       |
| `policy`       | the `--policy` used (`frontier-walk`)                                                                                                |
| `class`        | frontier class of the job (`L` on a ledger record, `C3` on a fresh child, `C1` on the virtual root)                                  |
| `tier`         | job tier (the walk labels all jobs `frontier-walk`)                                                                                  |
| `budget`       | child-eval budget granted for this visit                                                                                             |
| `child_evals`  | child-evals actually spent                                                                                                           |
| `wall_s`       | job wall time (seconds)                                                                                                              |
| `outcome`      | `"win"` / `"loss"` (decisive, shard exported) or `"censored"` (no fact)                                                              |
| `exit_reason`  | solver exit: `Complete` (decisive) or `BudgetExhausted` (censored)                                                                   |
| `tag`          | shard tag if decisive (manifest tag; matches `provenance` in the DB)                                                                 |
| `shard_nodes`  | exported shard's node count if decisive                                                                                              |
| `pass`         | visit rung: `passes_failed + 1` (the walk census)                                                                                    |
| `work_before`  | ledger `work_done` recorded before this visit (the walk census)                                                                      |
| `kind`         | visit kind: `search` (a picked underworked candidate) or `expand` (the first unhandled child of a worked candidate)                  |

**Censored** job records (quickstart §3.1, the smoke's two visits;
`wall_s` varies by host):

```text
job: {"budget":200000,"child_evals":200000,"class":"C1","exit_reason":"BudgetExhausted","kind":"search","outcome":"censored","pass":1,"path":"","policy":"frontier-walk","shard_nodes":null,"tag":null,"tier":"frontier-walk","wall_s":0.02,"work_before":0}
job: {"budget":200000,"child_evals":200015,"class":"C3","exit_reason":"BudgetExhausted","kind":"expand","outcome":"censored","pass":1,"path":"a2a3","policy":"frontier-walk","shard_nodes":null,"tag":null,"tier":"frontier-walk","wall_s":0.02,"work_before":0}
```

A **decisive** job record (the plan16 reference boot's first decisive
expansion visit, shown in the current grammar):

```text
job: {"budget":4000000,"child_evals":6990,"class":"C3","exit_reason":"Complete","kind":"expand","outcome":"win","pass":1,"path":"c2c3 g7g5","policy":"frontier-walk","shard_nodes":334,"tag":"h_2fd82f9a8230a668","tier":"frontier-walk","wall_s":0.0006,"work_before":0}
harvest: policy frontier-walk stop=budget jobs 553 decisive 53 censored 500 evals 2001311002 new_shards 53 wall 216.2s
```

_stdout, after the last job_ — the manifest line (printed only when the
manifest actually changed, else `manifest: unchanged (…)`), then the
summary line; `out-db:`/`dump:` echo lines follow when those options were
given:

```text
manifest: unchanged (0 entries, digest d801aa1fb7ddcc33…)
harvest: policy frontier-walk stop=max-jobs jobs 2 decisive 0 censored 2 evals 400015 new_shards 0 wall 0.1s
```

- `manifest:` — entries + full manifest digest after the rewrite
  (byte-identical when no decisive job occurred; the per-job sync means
  the rewrite at batch end is an idempotent no-op byte-wise — the line is
  the batch-end confirmation, not the first write).
- `harvest:` — the `stop=` reason, then `jobs N decisive D censored C
  evals E new_shards S wall Ws`. `decisive` counts jobs that exported a
  shard; `censored` counts facts of absence.

Under `frontier-walk`, `stop=exhausted` means no candidate can yield a
visit (every candidate complete and the scope root proven-stored or
complete-blocked — a scoped session with a proven-stored scope root
exhausts immediately, with the explanatory stderr note); on any undecided
layer the walk always has work, so capped stops (`budget`/`max-jobs`) are
the normal way a session ends.

_stderr, during jobs_ — `[bounded_search] chunk done: …` progress lines
(one per budget chunk); informational, safe to ignore in scripts.

**Worked invocation.** Initial run §3.1 (the smoke shape: two censored
jobs at 200k evals, manifest unchanged, exit 0; production budgets in
§3.2). The ledger pick-up the smoke run seeds — the sweep continuing from
the recorded censor history — is described in §4.

### 7.3 `proofdb_flip` — implied-flip analysis (read-only)

**Purpose.** Sanity check over a grown DB: for every open row whose legal
replies are all resolved (stored or implied), compute the implied outcome +
bound, iterated to fixpoint; every reported flip is then recomputed by an
independent recursive verifier. Reads the DB; writes only `--out`
(default `data/proofdb/flip.json`, created on demand).

**Prerequisites.** A merged DB whose `built_from` matches the manifest
digest (the stale-DB guard, §10). A missing default manifest aborts
cleanly naming `data/proofdb/shards/manifest.json`.

    usage: proofdb_flip [--db <grown.db>] [--manifest <manifest.json>]
                        [--out <flip_analysis.json>]

| Option       | Default                             | Meaning                       |
| ------------ | ----------------------------------- | ----------------------------- |
| `--db`       | `data/proofdb/proofdb.db`           | the grown DB (read-only)      |
| `--manifest` | `data/proofdb/shards/manifest.json` | manifest digest for the guard |
| `--out`      | `data/proofdb/flip.json`            | report JSON (overwritten)     |

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

**Worked invocation.** Recipe R1 then R3 (0 flips over the standing set,
exit 0); in a production cycle the flip check is recipe R2's step 3.

### 7.4 `proofdb_ledger_union` — N-way ledger merge

**Purpose.** Merge work ledgers (per-worker harvest state) into one.
Keeps, per path, the record with the highest `passes_failed` (ties: higher
`work_done`). `N ≥ 1` inputs; `N = 1` normalizes (deterministic rewrite).

**Prerequisites.** Ledger JSONs from `proofdb_harvest --ledger` runs (or
committed snapshots).

    usage: proofdb_ledger_union --out <ledger.json>
                                [--expect <file>=<sha256>]... <input.json>...

| Option          | Default  | Meaning                                                              |
| --------------- | -------- | -------------------------------------------------------------------- |
| `--out`         | required | merged output ledger (deterministic, sorted bytes)                   |
| `--expect`      | none     | pin an input's sha256 digest (repeatable; verified before the merge) |
| `<input.json>…` | required | 1..N ledger files                                                    |

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

**Worked invocation.** Recipe R4 (N = 1 normalization of the committed
plan10 snapshot → 8,446 records).

## 8. Recipes

### 8.1 Operator recipes

**R0 — first production start (bootstrap).** Create the empty manifest at
the default location, then merge it (bare tools; everything lands inside
`data/proofdb/`):

```bash
mkdir -p data/proofdb/shards
printf '{"entries":[]}' > data/proofdb/shards/manifest.json
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

On the reference host a 2G-capped boot at the 4M base budget grew the
layer from 1 to 6,706 nodes with 53 shards in ~4 minutes (§3.2). From
batch 2 on, grow with R2. The committed fixture shard set keeps a
development/validation role only (R1, tests); the plan10 ledger snapshot
remains an optional §4 optimization for resuming censor history, never
part of the boot path.

**R1 — rebuild the fixture DB (development, explicit flags).** Merge the
committed fixture shard set into a fresh DB, then compare the digest:

```bash
proofdb_merge --manifest docs/plans/proofdb/shards/manifest.json \
    --shard-dir docs/plans/proofdb/shards --db /tmp/fixture.db \
    --dump nodes.txt
sha256sum /tmp/fixture.db
```

A rebuild is _always_ allowed; it is also the response to a stale-DB abort
(§10). Byte-identical output for an identical manifest is the standing
regression test
(`tests/proofdb.rs::standing_layer_rebuilds_byte_identical`). For the
standing fixture shard set the census is the one in §7.1 and the digest is
`0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8`.
The production rebuild is the same merge with the bare defaults:
`proofdb_merge` (R2 step 2); its digest is pinned per manifest, not
universally.

**R2 — grow the DB by one batch, then merge (production, bare defaults).**

```bash
# 1. Harvest (a capped walk batch; stop conditions as needed).
proofdb_harvest --policy frontier-walk --budget-evals 4000000 \
    --max-total-evals 10000000000
# 2. Merge the grown set (the harvest never merges).
proofdb_merge
# 3. Flip sanity over the grown DB.
proofdb_flip
```

Note: after a _decisive_ batch the manifest digest changes, so the new
DB's `built_from` — and therefore its byte content — differs from the old
DB. Byte-identity is guaranteed per manifest, not across growth steps.

**R3 — check for implied flips.** Recipe R2 step 3 (`proofdb_flip`, bare
defaults); expected on a sound tree: `flips 0 verified 0`. Any flip is a
soundness signal — stop and investigate (§9), never re-merge to make it
disappear.

**R4 — normalize or merge work ledgers.**

```bash
# N = 1: canonicalize a ledger (deterministic rewrite).
proofdb_ledger_union --out ledger.norm.json ledger.json
# N = 1: normalize the committed plan10 snapshot → 8,446 records.
proofdb_ledger_union --out work_union.json \
    docs/plans/proofdb/measurements/plan10/ledger_union.json
# N > 1: merge per-worker ledgers, digest-pinned.
proofdb_ledger_union --out merged.json \
    --expect w1.json=9c060103… --expect w2.json=4e14f46f… w1.json w2.json
```

Union is a conservative max-rule merge (`passes_failed` max, tie by
`work_done`) — it never invents facts; it only preserves the strongest
recorded censor history per path.

**R5 — retire an abandoned working layer.** The DB and ledger are
derived/selection state: delete `data/proofdb/proofdb.db` and
`data/proofdb/work.json`, re-merge (bare `proofdb_merge`), and either
start with a fresh ledger or seed it from a committed snapshot (§4).
Nothing in the durable layer (`data/proofdb/shards/`) is touched.

Operators with a pre-plan14 working layer (flat paths) migrate it first:

```bash
mkdir -p data/proofdb && mv data/proofdb.db data/proofdb/proofdb.db && \
    mv data/proofdb_work.json data/proofdb/work.json
```

### 8.2 Consumer recipes (SQL)

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
FROM shards ORDER BY tag;                             -- the manifest as rows
```

**DB census (the three numbers to display as a data version stamp).**

```sql
SELECT (SELECT COUNT(*) FROM nodes),
       (SELECT COUNT(*) FROM nodes WHERE outcome IS NOT NULL),
       (SELECT value FROM meta WHERE key = 'built_from');
-- → 55703 | 55628 | e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd
```

## 9. Soundness contract (operator terms)

- Only shards whose manifest entry says `validate: "ok"` enter a merge, and
  each is replay-validated again at merge time and once more as a
  re-assembled skeleton after the depth fixpoint.
- Proven-outcome conflicts between shards abort the merge (exit 2). They
  are soundness bugs, never patched over.
- The DB is a derived view: delete it and rebuild with `proofdb_merge`
  anytime — same shard set, byte-identical result. The rebuild is a
  standing regression test (`tests/proofdb.rs::
standing_layer_rebuilds_byte_identical`) and recipe R1's `sha256sum`
  step.
- A DB must carry `meta.built_from` = the sha256 of the manifest it was
  built from; `proofdb_harvest`/`proofdb_flip` refuse a DB whose digest
  differs (stale DB or stale manifest — re-merge first).
- Censored (draw) results are facts of absence only: no shard, no DB row,
  a ledger record. Non-terminal draws are not representable as facts in
  this tree model.

## 10. Troubleshooting and known limitations

**Stale DB (`built_from` mismatch).** `proofdb_harvest`/`proofdb_flip`
abort with e.g.

```text
proofdb_flip: DB built_from deadbeef != manifest digest e91ad57b44008fee (stale DB or stale manifest — re-merge the standing shard set first)
```

The DB was built from a different manifest than the one on disk (usually a
harvest batch completed since the last merge). Fix: re-run the bare
`proofdb_merge` (recipe R2 step 2; it rebuilds
`data/proofdb/proofdb.db` from the on-disk default manifest).

**Missing default manifest.** A bare `proofdb_merge`, `proofdb_harvest`,
or `proofdb_flip` with no `data/proofdb/shards/manifest.json` aborts
cleanly, naming the missing path:

```text
proofdb_flip: cannot read manifest data/proofdb/shards/manifest.json: No such file or directory (os error 2)
```

Fix: bootstrap the working layer (recipe R0), or point `--manifest` at the
fixture for a development run (§2). Reading never creates anything — only
output writes create directories. The guard is
length-safe on malformed DB values (a short or empty `built_from` aborts
cleanly, regression-tested).

**Merge aborts with exit 2.** A CONFLICT/DEFECT abort (§7.1) leaves no
partially written DB. Read the abort message: it names the shard/manifest
defect. Never "fix" a proven-outcome conflict by editing the manifest —
that is a soundness bug in the pipeline and belongs in a bug report.

**Orphan shards after a harvest abort.** The manifest is atomically
rewritten after every decisive job, so a mid-session crash or ctrl-c
(exit 2, or killed) leaves a consistent layer: everything collected up to
the last finished job is referenced and a subsequent bare `proofdb_merge`
folds it in. At most one orphan remains — the shard of the job that was
in flight (interrupted between the shard write and its manifest entry, or
whose manifest write failed; the rename keeps the previous complete
manifest). The retry deterministically overwrites it; no manual cleanup
is needed, but an unreferenced `*.bin` in the shard dir is a symptom
worth noting in the operator log. A merge after any batch with decisive
jobs is still required before the next harvest session (the stale-
`built_from` guard above).

**Fresh vs seeded ledger.** A fresh ledger (missing `--ledger` file) is
sound but re-censors every frontier reply at the base budget on first
revisit — lower first-batch yield. Seeding from a committed snapshot (§4)
or a `proofdb_ledger_union` merge (§7.4) recovers the censor history. A
ledger never changes DB facts; getting it wrong costs work, not soundness.

**Digest drift after growth.** Recipe R1's pins (`0d929f4c…`,
55,703 nodes) describe the _standing fixture_ shard set. After a decisive
harvest batch the manifest digest changes and a fresh merge produces a
different DB — that is growth, not drift. Compare DBs per manifest digest
(`built_from`), never across manifests.

**Startpos-rooted only.** Subtree-scoped harvesting (`--root-fen`) is
parked (initiative item 7); every job replays from the standard start
position.

**Merge is a separate step.** `proofdb_harvest --out-db/--dump` only
record the intended targets in the summary; after each batch, run
`proofdb_merge` yourself (recipe R2).

## 11. History

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
