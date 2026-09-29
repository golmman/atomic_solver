# Global proof store (SQLite), schema v1

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
-- rows: schema_version=1, root_fen=STARTPOS_FEN, node_count, built_from
--       (SHA-256 of the manifest file), generator

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
```

`move_code` is the 16-bit move encoding of `docs/spec/proof_tree_dump.md`
(bits 0–5 `to_sq`, 6–11 `from_sq`, 12–13 move type, 14–15 promotion piece;
`0xFFFF` is reserved as the null sentinel).

### Meta rows

| key | value |
| --- | --- |
| `schema_version` | `1` |
| `root_fen` | the startpos FEN every path is relative to |
| `node_count` | number of rows in `nodes` |
| `built_from` | SHA-256 (hex) of the manifest file the DB was built from |
| `generator` | free-form producer identification |

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
   valid proof: at an OR-node exactly one proving child (`outcome = 'loss'`)
   is stored; at an AND-node every legal reply of the replayed position is
   stored as a child with `outcome = 'win'`, except for terminal nodes
   (`depth_bound = 0`), which have no children.
3. **Terminal claims are static.** A proven node with `depth_bound = 0` is
   rule-terminal; its outcome follows from the replayed position alone.
4. **Depth monotonicity.** For a proven non-terminal OR-node,
   `depth_bound = child.depth_bound + 1` (its single proving child); for a
   proven non-terminal AND-node, `depth_bound = max(child.depth_bound) + 1`
   over all reply children. `depth_status = 'exact'` additionally asserts the
   stored bound is the true distance-to-terminal.
5. **Open nodes are undecided.** `outcome IS NULL` means either "searched to
   no decisive result under some bounded budget" or "not yet searched"; the DB
   does not distinguish the two (v1 has no unexpanded-node table). Open nodes
   may have proven descendants (a graft's ancestor chain).
6. **No cross-path merging.** Two nodes with identical positions but different
   paths are distinct rows; facts are never transferred across paths.
7. **Determinism.** `id`s are assigned in lexicographic path order (paths
   compared elementwise by `move_code`, a prefix sorting before its
   extensions; the root path is least — equivalently, a depth-first walk
   visiting children by increasing `move_code`). Rebuilding the DB from an
   identical shard set yields identical `nodes` content.

## 5. Manifest contract

A **manifest** lists the shards a database is built from. It is a JSON object
with an `entries` array; each entry requires:

| field | type | meaning |
| --- | --- | --- |
| `tag` | string | unique shard identifier |
| `file` | string | shard binary file name (`proof_tree_dump.md` format), resolved against the shard directory |
| `fen` | string | the shard's own root position FEN |
| `moves` | string array | UCI path from the startpos to the shard root (`[]` iff the shard root is the startpos) |
| `outcome` | string | `'win'` or `'loss'`, the proven outcome of the shard root, side-to-move perspective |
| `validate` | string | validator verdict; a shard enters a merge only on `'ok'` |

Additional per-entry fields (tier, node counts, timings) are permitted and
ignored by the schema contract. `dtm_length`, when present, is the shard
root's proven depth bound. A manifest with a duplicate `tag` or a duplicate
`moves` path across entries is invalid.

## 6. Out of spec

How the database is produced (merger tooling, harvest loop, DTM upgrade
passes, coverage policy) is outside this contract. The DB is a derived,
rebuildable view: the durable truth is the shard set plus its manifest.
