# Plan 1: merger MVP + SQLite schema + `global_proof_store.md` spec + seed graft

Initiative: `proofdb`. Executes **backlog item 1** (the working layer).
This plan builds the merger tool, the schema spec, and the seeded database
from the existing solve plan4 artifact. It is docs + `examples/`-side code
only; the product solver is untouched. Per repo convention the final task
is `report1.md`.

## 1. Background (self-contained)

**Seed artifact.** `docs/plans/solve/measurements/plan4/artifact/` holds 95
validated proof-tree shards (`lad_*.bin`) plus `index.json` (the manifest):
per entry `tag`, `fen` (the shard's *own* root position, i.e. a job position
2–38 plies deep), `moves` (UCI path from the startpos to that root),
`outcome` (`win`/`loss`), `dtm_length`, `pv`, `validate` (`ok` on all 95),
`tree_nodes`, `wall` etc. Totals: 12,592 tree nodes, 82,644 bytes, find
wall 14.6 s, verify wall 2.4 s. 34 members report duplicate keys (overlapping
shard content) — the merge must dedupe by path, not assume disjointness.

**Shard root fidelity.** The `proof_tree.bin` format (v1) stores a root FEN
but no hashes; `ProofTree::from_bin` parses it and each `ProofNode` carries a
Zobrist `hash`. Grafting therefore works by: replay the manifest `moves` from
`Position::STARTPOS_FEN` with `notation::uci_to_move` + `Position::do_move`,
then require `Position::from_fen(shard.root_fen).hash() == shard_root_node
.hash()` **and** `replayed_position.hash() == shard_root_node.hash()`. Hash
equality subsumes FEN normalization and the halfmove clock.

**Design decisions (fixed in the founding session, restated for a fresh
executor):** the DB is a *partial AND/OR tree keyed by path from the
startpos*; proven nodes carry `depth_bound` (subtree-proven "forced outcome
within ≤ d plies") and `depth_status` (`bound`|`exact`); nodes searched
without a decisive result are `open` (`outcome = NULL`); non-terminal draws
are represented as `open`, never as a finitely expanded draw claim. Durable
truth is the shard set; the SQLite file is derived and rebuildable;
`docs/spec/global_proof_store.md` is the sole external contract (website
reads only the DB).

## 2. Deliverables

- **D1 — `docs/spec/global_proof_store.md`** (standalone, normative; may
  reference `docs/spec/proof_tree_dump.md` for the shard format, nothing
  else outside `docs/spec/`). Content:
  - SQLite schema v1:
    ```sql
    CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
    -- rows: schema_version=1, root_fen=STARTPOS_FEN, node_count, built_from (manifest digest), generator
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
  - Semantics/invariants (the contract the website may rely on): every
    decisive node is replay-verifiable from its parent chain; a node with
    `outcome NOT NULL AND depth_bound = 0` is rule-terminal; `exact`
    implies minimal depth; `open` means "searched to no decisive result
    under some bounded budget" *or* "not yet searched" — the DB does not
    distinguish (no unexpanded-node table in v1); ancestors of a graft are
    `open` nodes created by path replay.
  - The manifest contract: required per-shard fields (`tag`, `fen`, `moves`,
    `outcome`, shard file, validator pass); plan4's `index.json` entry shape
    is a conforming v1 manifest.
  - Explicitly out of spec: how the DB was produced, any tool name.
- **D2 — merger `examples/proofdb_merge.rs`** (+ shared module
  `examples/proofdb/`, mirroring the `examples/campaign/` pattern; each file
  ≤ 10 KB). CLI:
  ```sh
  proofdb_merge --manifest <index.json> --shard-dir <dir> [--db <out.db>]
                [--dump <nodes.txt>] [--sample-lines <n>]
  ```
  Pipeline per shard: `read_proof_tree` → cross-check manifest fields
  (fen/outcome vs tree root) → `validate_proof_tree` (abort on any defect)
  → graft onto the startpos ancestor chain → overlay into an in-memory
  path-keyed tree → write SQLite + canonical dump. Provenance per node =
  contributing shard tags. Sample lines: `--sample-lines` walks N random
  root-to-frontier paths and prints UCI lines (human spot-check).
- **D3 — seeded DB**: run over the plan4 artifact into
  `docs/plans/proofdb/measurements/plan1/` (DB + canonical dump committed —
  derived, small, reusable; per AGENTS.md measurement layout).
- **D4 — `report1.md`.**

## 3. Merge semantics (pre-registered)

Node identity = the UCI path from the startpos (normalized: generator moves
only; no FEN strings). Global node ids are assigned by **lexicographic path
order** so ids are independent of shard processing order. Overlay rules:

- *Agreement*: same path, same proven outcome → union children, dedupe,
  provenance = union of tags.
- *Refinement*: same path, both proven → keep the smaller `depth_bound`;
  at equal bound `exact` dominates `bound`; provenance = union.
- *Contradiction* → **hard abort, non-zero exit**: (a) same path, different
  proven outcomes; (b) one shard claims `exact` at depth d, another proves
  a bound < d. A conflict is a soundness bug in a shard or the manifest —
  never patched, per constraint 1.
- Ancestor nodes from graft paths are inserted as `open`; they are upgraded
  by any shard that proves them through another route (agreement rules
  apply as above).

**Depth-status mapping for the seed:** inspect `spike4.py` / `report4.md`
(plan4) for how `dtm_length` was established. Map `exact` only if the
record documents a minimality proof (e.g. ladder exhaustion below the
bound); otherwise map `bound`. If it cannot be settled from the record,
the whole seed maps `bound` — conservative, recorded as a finding, never
guessed per node.

## 4. Pre-registered gates (fixed before any run)

- **G1 graft census**: all 95 shards parse and validate (`validate: ok`
  per shard); all manifest↔tree cross-checks and both hash-fidelity checks
  pass; conflicts 0; reported node arithmetic (shard nodes + ancestor
  insertions − dedupes) reproducible from the canonical dump.
- **G2 merged-tree validation**: the merger reassembles the merged tree as
  one in-memory `ProofTree` rooted at `STARTPOS_FEN` and `validate_proof_
  tree` passes on it (the "always again at finalization" leg).
- **G3 determinism**: two full runs from the same shard set produce
  byte-identical canonical dumps.
- **G4 replay spot-checks**: `--sample-lines 10` yields 10 UCI lines; the
  known line `1.c3 g5 2.Qa4` (tag `p2_c2c3_g7g5`: win, bound 7 per the
  manifest) replays from the startpos and the g5 node reads
  `outcome=win, depth_bound=7` in the DB.
- **G5 hygiene**: `make test` green; no `src/` changes (product surface
  untouched); `cargo clippy` / `cargo fmt` clean on the new code.

## 5. Tasks

1. Read `spike4.py` + `report4.md`; settle the `dtm_length` provenance and
   fix the seed's depth-status mapping (§3).
2. Write D1 (`docs/spec/global_proof_store.md`).
3. Implement D2 (schema.rs → merge.rs → CLI; tests inline: fixture shard,
   conflict-abort, refinement rules, id-assignment determinism).
4. Run the seed build (D3); check G1–G4; iterate on failures (a gate
   failure is a defect in the merger or the plan's model — stop and
   investigate, do not loosen the gate).
5. Write D4 (`report1.md`), including census numbers, gate verdicts, and
   findings.

## 6. Non-goals

No `src/` changes; no harvest loop (backlog item 2); no DTM-upgrade pass
(item 3); no website; no support for multiple roots/components (startpos-
rooted only, per the initiative); no compression or incremental-DB
optimization — v1 sizes are tiny.

## 7. Budget

One session. Compute is minutes: the seed shards' original find wall was
14.6 s total and verify wall 2.4 s; the merger re-validates (replay) at a
similar or lower cost. The plan fits the default gate (`make test`).

## SESSION COMPLETE

- `docs/plans/proofdb/initiative.md` written; `plan1.md` written;
  README row added; no code changed; gate not applicable (docs only)
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan1.md (merger MVP +
   global_proof_store.md spec + seed graft of the 95 plan4 shards); run
   the gates; write report1.md."
2. Alternative: first skim plan1 §2/§3 with the user if any schema or
   merge-rule detail needs a product-level tweak before code exists.
