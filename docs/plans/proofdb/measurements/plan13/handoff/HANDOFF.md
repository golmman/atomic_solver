# Handoff — the startpos proof-line database (SQLite, schema v2)

**For:** the external exploratory-website project (read-only consumer).
**Date:** 2026-10-20 · **From:** the producer of the database — the
`atomic_solver` chess-solver project. This delivery is a **re-cut** of the
2026-10-16 package: same contract (schema v2), the same tree content, and
a migration of the producer's standing layer to the v2 source-of-truth
model (see Provenance). Any copy of the 2026-10-16 *pre-migration* re-cut
(sha256 `02fc7d60…`, `built_from 70a160a6…` — the manifest-digest era) is
superseded by this one and should be discarded; the same holds for the
original 2026-10-02 v1 delivery (sha256 `0d929f4c…`).

## What is in this directory

| file | content |
| --- | --- |
| `proofdb.db` | the proof-line database — one SQLite file, the only data artifact a consumer needs |
| `global_proof_store.md` | **the normative contract**: schema v2, field semantics, invariants, example consumer queries (copy of the producer's current contract, lightly edited only to inline two references to producer-internal format documents — the encoding itself is fully specified inline) |
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
`'draw'`. The DB also carries a `censor` table (schema v2) — producer-side
harvest scheduling state, no proof facts, no invariant touches it; ignore
it entirely. Read-only consumption; schema and semantics are exactly the
copied spec — nothing else is guaranteed to a consumer.

## Version stamp (verify after receiving)

```text
sha256(proofdb.db) = 375a2b1fc8a2959e3d60aa6737ba6b479988f23de3e953a3233fbae72b69f430
meta.schema_version = 2
meta.node_count     = 43092        (43028 proven: 21514 win + 21514 loss; 64 open)
meta.built_from     = 47b25861d0f56776e04dc534cd8c781fdc45e180a54898e179264fbc6c3c5d72
meta.root_fen       = rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1
```

`built_from` identifies the source data set the DB was built from: it is
the producer's SHA-256 **fold digest** over the shard provenance set
recorded in the DB's `shards` table (spec §2.2). Future deliveries will
carry a different `built_from` and node census — that is growth, not
drift (the census may move in either direction; a delivery is not
necessarily a node-count superset of a previous one). **The DB is the
single source of truth, not a derived view**: it grows by incremental
folds and is never rebuilt, so unlike a rebuildable artifact its tree
rows are the durable facts themselves. A file's sha256 is per-delivery
provenance — verify `built_from` and the census against a claim, and the
delivered file by its sha.

## Read-only consumption statement

- Consume the SQLite file read-only; the producing pipeline owns all
  writes. Any future update is a new file delivery (new `built_from`).
  This includes the `censor` table — never write to any table, never
  delete "unused-looking" rows.
- Do not infer facts from absent rows (§ contract: partial tree, open
  semantics). A row's `outcome IS NULL` is genuinely undecided.
- The spec's §4 invariants are the guarantee surface (replay-verifiable
  paths, proven decisive nodes, depth monotonicity, path-unique identity,
  stable ids). §7 of the spec has ready-to-run example queries.

## Provenance

Built and maintained by the producer's single-writer incremental fold
from its validated source set (405 proven shards, recorded in the DB's
`shards` table with per-shard digests — that table is the permanent
provenance record). The 2026-10-20 delivery re-stamps the standing layer
onto the v2 source-of-truth model: an in-place fold whose `nodes`,
`censor`, and `shards` rows are byte-identical to the previous delivery's
— only `built_from` (fold digest instead of manifest digest) and the file
bytes changed. Independent checks at delivery time (2026-10-20): all §4
structural invariants verified by the producer's whole-DB replay
validator; an independent implied-value flip analysis reported 0
unverified flips over the 64 open rows.

Questions about the data or the pipeline: contact the owner (the
producer's operator manual is available on request).
