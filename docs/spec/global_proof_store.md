# Global proof store (SQLite), schema v2

**Status: normative.** This document is the sole external contract for the
global proof-line database of atomic chess. A consumer (e.g. an exploratory
website) reads only the SQLite file described here; how the file was produced
is out of scope for this document.

The database is a **partial AND/OR proof tree rooted at the standard starting
position**, keyed by move path. It answers: *from the startpos, which move
sequences are proven to win or lose, within how many plies, and with what
evidence?*

## 1. Data model

- Every node is identified by its **path**: the sequence of moves (in UCI
  notation) from the startpos to the node's position. Node identity is the
  path, not the position: the same position reached at different halfmove
  clocks (via transpositions or repetitions) is a *different* node. This makes
  clock-dependent repetition verdicts representable and keeps every node
  replay-verifiable from its parent chain.
- The tree is a **partial AND/OR tree**. The OR/AND role of a node is derivable
  from its `outcome` and is not stored: a node with `outcome = 'win'` is an
  OR-node (the side to move wins; the tree stores the proving child), a node
  with `outcome = 'loss'` is an AND-node (the side to move loses; the tree
  stores a refutation for every legal reply).
- Every stored fact is **replay-verifiable**: for each node, replaying its
  parent chain from the startpos reproduces exactly its position (Zobrist
  hash, halfmove clock included).

## 2. Schema

```sql
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
-- rows: schema_version=2, root_fen=STARTPOS_FEN, node_count, built_from
--       (SHA-256 fold digest over the shards table, §2.2), generator

CREATE TABLE shards (
  tag TEXT PRIMARY KEY, file TEXT, sha256 TEXT,
  root_fen TEXT, path TEXT,            -- path as space-separated UCI ('' for the root)
  outcome TEXT, depth_bound INTEGER, n_nodes INTEGER
);
CREATE TABLE nodes (
  id INTEGER PRIMARY KEY,              -- assigned in lexicographic path order; root = 0
  parent_id INTEGER,                   -- NULL for the root
  ply INTEGER NOT NULL,                -- plies from the startpos root
  move_uci TEXT,                       -- NULL for the root
  move_code INTEGER,                   -- 16-bit atomic_movegen code, NULL for root
  outcome TEXT,                        -- 'win' | 'loss' | NULL (= open / undecided)
  depth_bound INTEGER,                 -- proven plies to terminal from this node
  depth_status TEXT,                   -- 'exact' | 'bound'; NULL iff outcome IS NULL
  provenance TEXT                      -- shard tags, comma-separated sorted-unique
);
CREATE INDEX idx_nodes_parent ON nodes(parent_id);
CREATE TABLE censor (                -- schema v2, additive (see §2.1)
  path TEXT PRIMARY KEY,             -- space-separated UCI ('' for the root)
  work_done INTEGER NOT NULL,        -- cumulative child-evals spent
  passes_failed INTEGER NOT NULL     -- censored (budget-exhausted) visits
);
```

`move_code` is the 16-bit move encoding of `docs/spec/proof_tree_dump.md`
(bits 0–5 `to_sq`, 6–11 `from_sq`, 12–13 move type, 14–15 promotion piece;
`0xFFFF` is reserved as the null sentinel).

### The censor table (schema v2, additive)

Schema v2 adds one table over v1: `censor`, the relocated harvest work
ledger. One row per **known-open frontier node** the harvester has censored
(a bounded search ended without a decisive result — a *fact of absence*, not
representable in the tree model, since a non-terminal draw is `open`).
`work_done` is the cumulative child-evals spent at the node (all visits);
`passes_failed` counts its censored visits. `path` uses the same
space-separated-UCI convention as `nodes`-derived paths, with `''` for the
root; it is selection-state identity, not a foreign key into `nodes` (most
censor paths have no row).

The table is **harvest scheduling state**, not proof data:

- The harvest loop is the only writer of `censor` rows (each censor commits
  one `INSERT OR REPLACE` transaction); the merger never modifies them — a
  fold leaves the table untouched in place.
- Consumers may ignore the table entirely: every v1 surface (`meta`,
  `shards`, `nodes`, and all invariants below) is unchanged in v2. No
  invariant of §4 involves `censor`.
- `censor` rows are replay-independent scheduling hints; deleting them
  costs re-searched work, never soundness.

### Meta rows

| key | value |
| --- | --- |
| `schema_version` | `2` (v1 files carry `1`; v2 is additive — the `censor` table only) |
| `root_fen` | the startpos FEN every path is relative to |
| `node_count` | number of rows in `nodes` |
| `built_from` | SHA-256 (hex) fold digest over the shard provenance set (§2.2) |
| `generator` | free-form producer identification |

### 2.2. The fold digest (`built_from`)

The database grows by an **incremental fold**: each fold validates a batch
of shards, grafts them into the standing tree, and commits one transaction.
`built_from` is the digest of the *provenance set* after the last fold: the
SHA-256 over the sorted lines `"<tag> <sha256>\n"` where `tag`/`sha256` are
the `shards`-table rows. It is deterministic (the same DB content and shard
set always digest identically) and never NULL, but it is **not** a
rebuildability stamp: the DB is the source of truth (§6), not a derived
view. A tag with the same bytes folding again is a no-op; the same tag with
different bytes is a defect the fold refuses (it never patches).

## 3. Field semantics

- `outcome` is **from the side-to-move perspective** at the node's position:
  `'win'` = the side to move forces a win, `'loss'` = the side to move is
  lost. `NULL` (**open**) means no decisive proven result is recorded for this
  node. A non-terminal draw is represented as `open`; only rule-terminal
  draws could be decisive, and the proof-tree format does not emit draw nodes,
  so no `draw` outcome occurs in v1.
- `depth_bound` (proven nodes only): the node's subtree proves *the decisive
  outcome within ≤ `depth_bound` plies from this node*.
- `depth_status` (proven nodes only):
  - `'bound'` — an upper bound; the true distance-to-terminal may be smaller.
  - `'exact'` — the bound is proven minimal.
  - Terminality is derivable: `outcome IS NOT NULL AND depth_bound = 0` is a
    rule-terminal node (its position statically yields its outcome).
- `provenance`: the shard tags that contributed any fact at or below the node
  (its own subtree facts, or — for open ancestors — the shards grafted in its
  subtree). Sorted lexicographically, comma-separated, unique.

## 4. Invariants (the contract a consumer may rely on)

1. **Replay-verifiable paths.** For every node, replaying `move_uci` down the
   `parent_id` chain from `meta.root_fen` on a rules-correct atomic-chess
   implementation yields exactly the node's position (same Zobrist hash).
2. **Decisive nodes are proven.** A node with `outcome NOT NULL` carries a
   valid proof: at an OR-node at least one proven-loss child (`outcome =
   'loss'`) is stored (additional proven-loss children are permitted —
   annotations from merged alternatives); at an AND-node every legal reply
   of the replayed position is stored as a child with `outcome = 'win'`,
   except for terminal nodes (`depth_bound = 0`), which have no children.
3. **Terminal claims are static.** A proven node with `depth_bound = 0` is
   rule-terminal; its outcome follows from the replayed position alone.
4. **Depth monotonicity.** For a proven non-terminal OR-node,
   `depth_bound = min(child.depth_bound) + 1` over its proven-loss
   children; for a proven non-terminal AND-node,
   `depth_bound = max(child.depth_bound) + 1` over all reply children.
   `depth_status = 'exact'` additionally asserts the stored bound is the
   true distance-to-terminal.
5. **Open nodes are undecided.** `outcome IS NULL` means either "searched to
   no decisive result under some bounded budget" or "not yet searched"; the DB
   does not distinguish the two (v1 has no unexpanded-node table). Open nodes
   may have proven descendants (a graft's ancestor chain).
6. **No cross-path merging.** Two nodes with identical positions but different
   paths are distinct rows; facts are never transferred across paths.
7. **Row identity.** `id`s are stable surrogate keys; **node identity is
   the path**, and consumers must never attach meaning to a row id beyond
   within-file reference stability. A fold preserves existing ids forever
   and appends new ones past `max(id)`; when a fold initializes a fresh
   (empty) DB, ids are assigned in lexicographic path order (paths compared
   elementwise by `move_code`, a prefix sorting before its extensions; the
   root path is least — equivalently, a depth-first walk visiting children
   by increasing `move_code`). Re-running the same fold sequence over the
   same shard set yields identical `nodes` content; two different fold
   sequences over the same facts may differ in ids only.
8. **The censor table is scheduling state.** The `censor` table (schema v2)
   carries no proof facts and participates in no invariant above; only the
   harvest writes it, and a fold leaves it untouched in place (see §2.1,
   §6.1).

## 5. Shard transport

### 5.1. The per-shard sidecar (current)

A shard is transported as a self-describing pair in the shard directory:
the shard binary `<tag>.bin` (`proof_tree_dump.md` format) plus a JSON
sidecar `<tag>.meta.json` with exactly these fields:

| field | type | meaning |
| --- | --- | --- |
| `tag` | string | unique shard identifier |
| `file` | string | shard binary file name, resolved against the shard directory |
| `fen` | string | the shard's own root position FEN |
| `moves` | string array | UCI path from the startpos to the shard root (`[]` iff the shard root is the startpos) |
| `outcome` | string | `'win'` or `'loss'`, the proven outcome of the shard root, side-to-move perspective |
| `sha256` | string | hex digest of the bin bytes (the producer writes it; a consumer refuses a bin/sidecar mismatch) |

Write order is the commit marker: a sidecar present ⇒ its bin is complete;
a bin without a sidecar is an interrupted job's orphan (skippable, the
retry overwrites determinically — tag = hash of path); a sidecar without a
bin is a transport defect. Sidecars are **ephemeral transport**: once their
content is folded into the DB (recorded in the `shards` table), the pairs
are deletable.

### 5.2. The legacy manifest (migration source)

The retired **manifest** format remains readable as a migration/metadata
source: a JSON object with an `entries` array; each entry requires the
sidecar's fields plus `validate` (a shard enters a fold only on `'ok'`)
and no `sha256` (the fold computes and backfills the sidecar's digest).
Additional per-entry fields (tier, node counts, timings) are permitted and
ignored. A manifest with a duplicate `tag` or a duplicate `moves` path
across entries is invalid. Where both a sidecar and a manifest entry exist
for a tag, the sidecar wins.

## 6. Source of truth and integrity

The **SQLite file is the single source of truth**: it grows by incremental
folds and is never rebuilt from shards (an append-only truth — a buggy fold
cannot be excised by deleting shards and rebuilding; operators keep an
offsite copy, e.g. a `VACUUM INTO` snapshot, *before* any deletion). Shards
(sidecar pairs, §5.1) are transport: deletable after their fold. The
integrity instrument is **whole-DB replay validation**: the tree read back
from the DB alone is replayed from the startpos and re-checked against
§4's invariants (path replay, AND/OR structure, terminal claims, depth
monotonicity, provenance non-emptiness, reachability, and the meta
census). Recorded limitation: a non-terminal `'exact'` minimality claim is
trusted as a search fact — it is not re-derivable from the DB alone.

How the database is produced (fold tooling, harvest loop, coverage policy)
and operated (fold → validate → backup → delete) is outside this contract.

### 6.1. Fold discipline (normative summaries)

- A fold is **one transaction per invocation**: any defect (a corrupted
  standing row, a contradicting shard, a bin/sidecar mismatch) rolls the
  whole invocation back — conflicts are never patched.
- A tag already recorded in `shards` is skipped after cross-checking its
  stored `sha256` against the bin on disk; a mismatch is a hard abort.
- The loader re-derives the standing tree from the `nodes` rows by replay
  before any write: a structurally broken DB aborts the fold.
- The `censor` table is untouched by folds; a legacy censor ledger can be
  merged in place under a per-field max rule (never decreasing a stored
  value), inside the same transaction.

## 7. Example consumer queries

Read-only SQL examples over the schema above. `:nid` denotes a node id
supplied by the application; a UCI line is resolved by walking it from
the root (`SELECT id FROM nodes WHERE parent_id = ? AND move_uci = ?`, one
indexed lookup per move — no path column exists by design, since node
identity *is* the path).

Full UCI path of a node (replay its parent chain):

```sql
WITH RECURSIVE up(nid) AS (
  SELECT :nid
  UNION ALL
  SELECT n.parent_id FROM nodes n JOIN up ON n.id = up.nid
)
SELECT group_concat(move_uci, ' ')
FROM (SELECT move_uci FROM nodes WHERE id IN up AND move_uci IS NOT NULL
      ORDER BY ply);
```

Children of a node (a missing child row means "no proven fact recorded",
not "illegal" or "refuted" — the tree is partial; see §3):

```sql
SELECT move_uci, outcome, depth_bound, depth_status
FROM nodes WHERE parent_id = :nid ORDER BY id;
```

Position facts and provenance of a node (the `provenance` tags refer to
the shard identifiers of the transport layer, §5):

```sql
SELECT ply, outcome, depth_bound, depth_status, provenance
FROM nodes WHERE id = :nid;
```

The open frontier — undecided rows a future harvest pass would open:

```sql
SELECT id, ply FROM nodes WHERE outcome IS NULL ORDER BY ply, id;
```

Shallowest proven wins (bound-minimal lines; render each id's path with
the recursive query above):

```sql
SELECT id FROM nodes WHERE outcome = 'win' AND depth_bound =
  (SELECT MIN(depth_bound) FROM nodes WHERE outcome = 'win');
```

Census — the three numbers that stamp a database version:

```sql
SELECT (SELECT COUNT(*) FROM nodes),
       (SELECT COUNT(*) FROM nodes WHERE outcome IS NOT NULL),
       (SELECT value FROM meta WHERE key = 'built_from');
```
