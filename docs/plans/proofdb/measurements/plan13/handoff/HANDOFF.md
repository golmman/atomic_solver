# Handoff — the startpos proof-line database (SQLite, schema v1)

**For:** the external exploratory-website project (read-only consumer).
**Date:** 2026-10-02 · **From:** the `atomic_solver` proofdb pipeline.

## What is in this directory

| file | content |
| --- | --- |
| `proofdb.db` | the proof-line database — one SQLite file, the only artifact a consumer needs |
| `global_proof_store.md` | **the normative contract**: schema v1, field semantics, invariants, example consumer queries (verbatim copy of the repo's `docs/spec/global_proof_store.md`) |
| `HANDOFF.md` | this file |

## The contract in one paragraph

The DB is a **partial AND/OR proof tree rooted at the standard atomic-chess
starting position**, keyed by move path: every row of `nodes` is a position
reached by a specific UCI move sequence from `meta.root_fen`; the same
position by a different route is a different row. `outcome` is from the
side-to-move perspective (`'win'` / `'loss'`, `NULL` = undecided);
`depth_bound`/`depth_status` carry "forced outcome within ≤ d plies" with
`'exact'` as the proven-minimal annotation. Every row is replay-verifiable
from its parent chain on a rules-correct atomic-chess implementation. The
tree is partial: a missing child row means "no proven fact recorded", not
"illegal" or "refuted". Non-terminal draws are undecided (`NULL`), never
`'draw'`. Read-only consumption; schema and semantics are exactly the
copied spec — nothing else is guaranteed to a consumer.

## Version stamp (verify after receiving)

```text
sha256(proofdb.db) = 0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8
meta.schema_version = 1
meta.node_count     = 55703        (55628 proven: 27768 win + 27860 loss; 75 open)
meta.built_from     = e91ad57b44008fee65ab0b124fab51365c17d08f891777cde525cfecb61fb5fd
meta.root_fen       = rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1
```

`built_from` identifies the shard set (the durable layer) the DB was built
from; future deliveries will carry a different `built_from` and node census
— that is growth, not drift. The DB is a derived view: it can be rebuilt
byte-identically from the shard set for the same `built_from`.

## Read-only consumption statement

- Consume the SQLite file read-only; the producing pipeline owns all
  writes. Any future update is a new file delivery (new `built_from`).
- Do not infer facts from absent rows (§ contract: partial tree, open
  semantics). A row's `outcome IS NULL` is genuinely undecided.
- The spec's §4 invariants are the guarantee surface (replay-verifiable
  paths, proven decisive nodes, depth monotonicity, path-unique identity,
  deterministic ids). §7 of the spec has ready-to-run example queries.

## Provenance

Built by `proofdb_merge` (the single-writer merger) from the standing
validated shard set — 262/262 shards, each replay-validated twice at merge
time (per shard, and as a re-assembled skeleton). Merge census:

```text
census: shards merged 262/262; shard tree nodes 56226; overlay new 55360; dedupes 866;
        open-ancestor insertions 342; open nodes upgraded 268; merged nodes 55703
        (proven 55628, open 75); skeleton re-validations 262
db: proofdb.db (nodes 55703), manifest built_from e91ad57b44008fee
```

Questions about the data or the pipeline: refer to the `atomic_solver`
repository's `docs/proofdb_pipeline.md` (operator and user manual) or
contact the owner.
