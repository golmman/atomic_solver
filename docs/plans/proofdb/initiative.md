# Initiative: `proofdb` — the startpos proof-line database

## Status

**Opened 2026-09-28.** A new initiative, deliberately *not* a reopening of
`solve` (see Relationship to other initiatives below). Goal: build and grow a
**startpos-rooted, machine-verifiable database of proven and disproven lines**
of atomic chess, structured as a partial AND/OR proof tree, consumable by an
external exploratory website.

## Motivation

The `solve` initiative's plan4 pilot demonstrated that a real, one-sided,
cheap tactical class of startpos-provable lines exists (55 ply-2 refutations;
a validated 10,177-position proof artifact at 83 KB) — but that the artifact
is a one-shot deliverable with no growth mechanism. The user's product idea:
harvest proven/disproven lines incrementally (sessions started and stopped at
will) into a *global proof* that a small external website lets users replay
and explore. The website itself is out of scope; this initiative owns the
orchestrator side: shards → merger → derived SQLite tree → spec'd schema.

The design was converged in the 2026-09-28 brainstorm session (this
initiative's founding conversation). Key decisions:

- **Structure: a partial AND/OR tree keyed by path from the startpos**, not a
  position-keyed fact store. At an OR-node (winning side to move) exactly one
  proving child is stored; at an AND-node (opponent to move) every refuted
  reply is stored with its own refutation subtree; nodes searched without a
  decisive result are `open` frontier. OR/AND roles flip with the proven
  outcome and are derivable — not stored. Path identity (not position-hash
  identity) naturally represents clock-dependent repetition verdicts.
- **Durable truth = validated shards (`proof_tree.bin` + manifest); the
  SQLite DB is a derived, rebuildable view**, produced by a single-writer
  merger. Nothing unverified enters the DB; a proven-outcome conflict between
  shards is a soundness bug and aborts the merge (hard tripwire, never a
  patch). The DB can be deleted and rebuilt from shards at any time.
- **Depth vocabulary: bound-by-default, exactness as an annotation.** Every
  proven node's subtree proves "forced outcome within ≤ d plies" (`depth_
  bound`); `depth_status = exact` is attached only where minimality is proven
  (1-move wins, preflight-certified ≤3-men subtrees, or future ladder
  passes). An optional background DTM-upgrade pass promotes `bound` →
  `exact` opportunistically; each upgrade is an independent unit of work
  (resume-friendly by construction, unlike the solve plan9 frozen-frontier
  resume, which was the measured failure mode there).
- **Draws are displayed as undecided** (`open`), except rule-terminal draws.
  A non-terminal draw claim cannot be finitely expanded in this tree model
  (repetition draws are path-dependent).

## Goal

A growing, verifiable, startpos-rooted partial proof tree:

- **Durable layer**: append-only shard directory. One shard = one validated
  `proof_tree.bin` subtree + manifest entry (root FEN, UCI path from the
  startpos, outcome, validator pass, provenance).
- **Working layer**: one SQLite file (`proofdb.db`), the merged tree:
  parent-linked nodes with `move_uci`, `outcome` (`win|loss|NULL=open`),
  `depth_bound`, `depth_status` (`bound|exact`), `provenance`, terminality
  derivable (`outcome NOT NULL AND depth_bound = 0`).
- **Contract layer**: `docs/spec/global_proof_store.md` — the SQLite schema
  and its semantics as the *sole* external contract (the website project
  reads only the DB; `proof_tree_dump.md` stays internal).

The seed is the existing solve plan4 artifact (95 validated shards, 12,592
tree nodes, 83 KB, all `validate: ok`). Its shards are rooted at their job
positions, not at the startpos, so the merger **grafts**: replay the manifest
path from the startpos, create open ancestor nodes, and attach the shard
tree at the graft node (fidelity check: shard root Zobrist hash == replayed
position hash).

## Design constraints (normative)

1. **Verifiable composition** (inherited verbatim from `solve`): no
   in-flight or unverified fact reaches the DB. A shard enters the merge
   only after replay validation passes; decisive outcomes are
   replay-verified facts; conflicts abort the session. Rejected, never
   patched.
2. **Path identity, no cross-path merging.** Node identity is the move
   sequence from the startpos. Two nodes with the same position at different
   halfmove clocks are distinct nodes (repetition/GHI discipline). Same-path
   facts from different shards are merged only under the agreement rules
   below.
3. **Derived-DB discipline.** The SQLite file is a cache of the shards;
   deterministic rebuild (identical shard set → identical canonical dump)
   is a gate, not an aspiration.
4. **Merge rules (pre-registered here):**
   - *Agreement*: same path, same proven outcome → union children, dedupe.
   - *Refinement*: same path, both proven, different `depth_bound` → keep
     the tighter (smaller) bound; `exact` dominates `bound` at equal depth.
   - *Contradiction*: same path, different proven outcomes → hard abort
     (soundness bug). Also abort if an `exact` depth is contradicted by a
     strictly smaller proven bound.
5. **Scope discipline.** All code lives in `examples/` (campaign
   precedent); the product solver (`src/`, CLI, benchmark gates) is
   untouched. Merger uses the public lib API only
   (`ProofTree::from_bin`, `validate_proof_tree`, `notation`, `Position`).

## Handover rule (to `solve`)

This database is a precursor of the startpos proof: if the merged tree ever
closes the startpos root (all ~41 children resolved, OR-node proven), the
artifact hands over to `solve`, which reopens per its own status rule with
the proof already built (shards are its master proof state's content).
Until then, no DB milestone is claimed as progress toward the startpos
value.

## Relationship to other initiatives

- `solve` (dormant): predecessor and eventual beneficiary; its plan4
  artifact is the seed; its constraint-1 soundness text is inherited.
- `proof` (dormant): the shard format is its `proof_tree_dump.md` (v1,
  unchanged); no reopening.
- `parallel` / campaign tooling (`examples/campaign*`): the future harvest
  loop (backlog item 2) reuses the campaign worker shape (persistent
  workers, deterministic budgets, TT retention) but runs *fresh roots per
  shard*, which is exactly the regime plan9 did not falsify (it resumed one
  frozen quiet root).

## Backlog

| # | Item | Mechanism | Potential | Effort | Status |
|---|------|-----------|-----------|--------|--------|
| 1 | **Merger MVP + schema + spec + seed graft** | `examples/proofdb_merge`: manifest+shard reader, per-shard replay validation, grafting, overlay into SQLite, canonical dump; `docs/spec/global_proof_store.md`; seed = the 95 plan4 shards | the working layer exists; website can be built against the schema | M | **done (plan1, 2026-09-28)**: 95/95 shards merged, 12,081 nodes, gates G1–G5 pass |
| 2 | **Harvest loop** | campaign-worker-shaped runs producing shards (fresh root per job → solve → reconstruct → validate → manifest entry); merger runs after each batch | the DB grows at will, stoppable anytime | M | open |
| 3 | **DTM-upgrade pass** | background jobs re-searching bound ladders to promote `bound` → `exact` where affordable | depth quality improves without re-harvesting outcomes | M | open |
| 4 | **Coverage policy** | which frontier children to open next (cheap-first gradient; sharpness like plan4's, not enumerative layers) | keeps per-fact cost low as the tree deepens | S–M | open |
| 5 | **Website handoff** | ship `global_proof_store.md` to the external website project; agree on read-only DB consumption | the product surface of this initiative | S | open |

## Non-goals

- The website itself (external project; consumes the spec'd SQLite schema).
- Product-solver features or CLI changes; no claim of progress toward the
  startpos value (handover rule above).
- Any parallel-search mechanism for a *single* position (the measured no-gos
  stand); the harvest loop is many independent roots, not a parallel solve.
- Exact-DTM everywhere; exactness is an annotation earned opportunistically.

## Measurement conventions

- Reference hardware: this sandbox. Seeds and merged DBs are small (KB–MB);
  the validated seed DB is committable under `measurements/plan<N>/` as a
  reusable validated artifact. Raw run transcripts are not committed.
- Every DB build records a census (shards, nodes, ancestors added,
  conflicts, validator pass) next to it; a census without a validator pass
  is not a result.

## History

- **2026-09-28** — **item 1 executed** (plan1): merger MVP in
  `examples/proofdb_merge` + `examples/proofdb/`, spec
  `docs/spec/global_proof_store.md`, seed DB built from the 95 plan4
  shards (12,081 nodes; 87 open); gates G1–G5 pass. One merger defect
  found and fixed by the pre-registered gates (parent-row id inversion —
  the run DB was wrong; see `report1.md`).
- **2026-09-28** — **initiative opened** (docs only): brainstorm converged
  on the path-keyed partial AND/OR tree, shards-as-truth / SQLite-as-view,
  single-writer merger in `examples/`, bound-by-default depth vocabulary,
  and the handover rule; `plan1.md` drafted (merger MVP + spec + seed
  graft). No code changed; product surface untouched.
