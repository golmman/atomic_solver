# Initiative: `proof` — providing an independent proof for a discovered outcome

## Status

Active, maintained **agile**: plans are single-lever and sized to one session,
the backlog is re-ranked after every report, and the decisive experiment
(plan5) gates the default-flipping plan (now plan7 after the 2026-09-11
renumbering). Plans 1–5 are done (`report1.md`–`report5.md`). The initiative
pivoted after plan3 (decision record 2026-09-10, below).

## Motivation

The initiative originates in `docs/plans/storage/concept.md` (core problem:
the transposition table is volatile, so a reported outcome has no independent
proof, and PPV extraction / post-hoc debugging are unreliable). The approach
chosen there — a dedicated proof tree built alongside the search and persisted
to a binary adjacency dump — was implemented across three plans:

- **plan1 — Authoritative incremental proof tree.** Every node/event carries
  the position Zobrist hash; a `finalize()` pass copies fully expanded
  canonical subtrees onto unexpanded transpositions, making the in-memory tree
  authoritative without a TT reconstruction pass.
- **plan2 — Dummy-parent proof tree with path traversal.** Events attach
  immediately; missing ancestors become dummy nodes realized later, removing
  the path index and pending buffer.
- **plan3 — Compact memory layout.** `ProofNode` 56 → 32 bytes, intrusive
  first-child/next-sibling links, one global child index
  `(parent_id << 32) | move_bits`, `dump_to_bin` straight from the worker.

### The constraint that forced the pivot

All three plans attack the same symptom: the proof tree must fit into the
search process (`--pt-size`, default 256 MB), and when it does not,
`ExitReason::MemoryLimit` aborts the search — destroying the decisive
outcome, the single most valuable product of the solver. The tree size of a
Win proof grows like the product of defender branchings along the proof
(AND/OR structure), so for the deep positions that are AGENTS.md's top
priority (~30 full moves / 60 plies) the budget is exceeded long before the
search is done. A multi-day run therefore cannot produce a proof tree at all
under the current design.

Meanwhile the search itself has the resource profile we want: DF-PN+ is
memory-bounded by the TT (RAM-fitted, disk untouched), so it can run for days
on compute-optimized machines. The goal is to get that property *back* by
removing the coupling to proof-tree generation, and to relocate proof
construction to a memory-optimized machine.

### Decision record 2026-09-10 (pivot)

Three transfer artifacts were evaluated for the decoupled design:

1. **Streaming event log — rejected.** It grows with proof size over the
   whole run (every proven node plus TT-hit re-emissions), so it violates the
   search's resource-bounded invariant: multi-day runs would fill the disk.
2. **Restore from FEN + PV — rejected, insufficient in principle.** A PV is
   one spine of the AND/OR tree. For a Win, the PV carries the attacker's
   choice, but every defender reply needs its own proven refutation — and
   those sibling subtrees are the bulk of the proof and are absent from a PV.
   Additionally `pv_status` explicitly qualifies that the PV is not a
   validated proof. FEN+PV remains a *verification* feature
   (`examples/verify_ppv`), not a reconstruction path.
3. **FEN + TT snapshot — selected.** The TT is a lossy summary of the search;
   solved entries store `outcome + depth + best_move`, which is everything
   reconstruction needs. The snapshot is **O(TT RAM), written once at exit** —
   days of compute compress into a bounded artifact. Reconstruction is a
   top-down walk from the root FEN: static terminal classification, Win nodes
   follow `best_move`, Loss nodes expand *all* legal replies via plain
   movegen + lookups (the bulk of the tree is movegen, not search). Holes
   (evicted entries, halfmove-clock key mismatches, uncached GHI repetition
   results) are filled by prefix-aware local solves seeded with the snapshot
   as the starting TT (`search_depth_with_prefix` already exists and preserves
   the repetition context).

What survives the pivot unchanged: plan1's `finalize()` canonical logic
(becomes the offline builder's core), plan3's compact `ProofNode` and
`binary.rs` adjacency format (the builder's output format and the DB import
contract), and the `ProofEvent` protocol (the builder *consumes* it — the
reconstruction walk can synthesize events into the existing worker engine).

## Goal

Provide an independent, inspectable proof for a discovered outcome without
coupling the search's resource footprint to proof size:

- the search stays **resource-bounded**: RAM = TT only, no unbounded disk, so
  it can run for days on compute-optimized machines;
- proof construction is **relocatable**: it runs wherever the memory is,
  consuming a bounded artifact (FEN + TT snapshot) and producing the existing
  binary proof-tree dump for the external PostgreSQL pipeline.

Priorities follow AGENTS.md: correctness first, then performance, memory,
maintainability.

## Working agreement (agile cadence)

1. **Artifact before tooling before flipping defaults.** plan4 adds the
   snapshot writer; plan5 builds the reconstruction tool *and* runs the
   go/no-go experiment; nothing in the search's default behavior changes
   before plan5 reports.
2. **One lever per plan.** Search-behavior changes (e.g. pinning solved
   entries) are separate plans with their own drift validation.
3. **Drift protocol.** Unless a plan explicitly says otherwise:
   `benchmark --suite quick --json --first-outcome` must stay bit-identical
   per case. Documented stdout changes (e.g. a new `tt_snapshot:` log line)
   are listed in the plan.
4. **Kill criteria are explicit.** plan5 defines what hole rate / fill cost
   would refute the snapshot design; the fallback (pin solved entries) is its
   own measured lever, not a silent patch.

## Backlog (re-ranked post-plan3; confidence-weighted)

| # | Item | Mechanism | Potential | Affects | Effort | Status |
|---|------|-----------|-----------|---------|--------|--------|
| 1 | TT snapshot writer | `--tt-dump-path`: bounded binary dump of live TT entries at exit (solved entries from all generations, unsolved from current). Format versioned like `binary.rs` | enables all downstream items; standalone-useful for debugging and TT seeding | new artifact module + CLI flag | S–M | **done (plan4, `report4.md`)** |
| 2 | Offline reconstruction tool + go/no-go experiment | Top-down walk from FEN over the snapshot: static terminal classification, movegen expansion, hole filling via `search_depth_with_prefix` seeded with the snapshot | proves or refutes that a bounded artifact suffices to rebuild full proofs | new example/tool | M | **done (plan5, `report5.md`) — GO for #3, conditional on the finalize canonicalization fix found by the oracle** |
| 3 | Flip the default: worker off during search | Search emits no proof events by default; retire `memory_limited` / `ExitReason::MemoryLimit` / `--pt-size` from the search CLI; proof construction moves entirely to #2's tool | restores the resource-bounded search invariant; removes the fatal MemoryLimit path | search CLI + docs + tests | M | open, **plan7** (renumbered 2026-09-11, see History); gated on #6 landing first — plan5 experiment verdict GO with the finalize fix as precondition |
| 4 | Pin solved TT entries | Solved entries become eviction-proof (they also prevent re-proving in the live search, but shrink frontier capacity) | reduces #2's hole rate if evictions dominate | search behavior — needs drift validation + nps measurement | S–M | deprioritized: plan5 measured `absent` = 2/91,470 proof nodes on the decisive suite — revisit only under deep multi-day eviction pressure |
| 5 | Periodic TT checkpoint | Bounded-size snapshot written every N minutes (not just at exit) | crash resilience for multi-day runs (the event log's one advantage, at bounded cost) | artifact writer | S | stretch |
| 6 | Builder-side validation tooling | `inspect_pt` / `verify_ppv` runs against reconstructed trees; root-outcome and bottom-up depth cross-checks against the snapshot before dumping | trust in the reconstruction | examples/tests | S | **claimed (plan6, `plan6.md`)**: plan5's oracle found the *live* event-built tree can be an incomplete proof (TT-hit re-emission is childless; `finalize()` canonical tie-break can pick an incompletely expanded twin — see `report5.md`); plan6 = fully-expanded-twin preference in `finalize()` + replay-based Loss-completeness validator, with 46/46 oracle-isomorphic as the acceptance criterion that un-gates plan7 |
| 7 | Deep-proof capacity of the builder | `finalize()` canonical expansion and the node store may still exceed a single machine's RAM for the deepest proofs | disk-backed finalize, or canonical expansion pushed into the import side | builder | L | open — needs a design spike only when deep proofs actually overflow the builder |

Done: plan1, plan2, plan3, plan4 (see History). Statuses reference plans under
`docs/plans/proof/`; an item is *open* until a plan claims it.

## Non-goals

- **PostgreSQL importer internals.** The import pipeline stays external; the
  `binary.rs` dump format is its contract and is not changed by this
  initiative. Persistence concerns remain with `docs/plans/storage/`.
- **`ProofEvent` protocol shape.** The `arch/plan1` contract (`Clear`,
  `NodeProven`) is consumed by the builder, not changed by it.
- **Optimizer interface contract** (`docs/spec/optimizer_interface.md`).
- **Search performance and semantics.** No search-behavior change unless a
  plan explicitly specifies and drift-validates it; the deterministic
  `child_evals` budget / `ExitReason::BudgetExhausted` contract must keep
  working exactly as documented.
- **PPV extraction quality.** That is the `pv/` initiative's territory; this
  initiative consumes `validate_ppv` as-is.

## Measurement conventions

- **Hole rate by cause** is the primary plan5 metric: for each proof node the
  reconstruction needs, classify snapshot lookups as hit / evicted /
  halfmove-clock miss / GHI-repetition-uncached, and report fill cost in
  `child_evals`.
- **Dual-build diff oracle**: for positions small enough that the event-driven
  worker still succeeds, build the tree both ways (events and snapshot
  reconstruction) and require isomorphism.
- **Drift check**: `benchmark --suite quick --json --first-outcome`
  bit-identical before vs. after any plan that touches `src/`, unless the
  plan intentionally changes search behavior.
- **Snapshot size bound**: dump size must stay ≤ TT RAM (sanity-checked in
  tests).

## History

- **plan1** — authoritative incremental proof tree with hash-carrying nodes
  and the `finalize()` canonical pass (done, `report1.md`).
- **plan2** — dummy-parent tree with path traversal; removed the path index
  and pending buffer (done, `report2.md`).
- **plan3** — compact `ProofNode` layout, global child index, worker-side
  `dump_to_bin` (done, `report3.md`).
- **2026-09-10** — pivot decision (see Motivation): TT snapshot selected as
  the transfer artifact; event log and PV restore rejected; plans 1–3
  re-framed as surviving infrastructure for the offline builder. Backlog
  table created.
- **plan4** — TT snapshot writer (#1), done (`report4.md`).
- **plan5** — offline reconstruction tool + go/no-go experiment (#2), done
  (`report5.md`). Result: hole rate 0.003% (hit 76% / terminal 24% of 91,470
  proof nodes), median fill ratio F/C = 0.0, zero anomalies/unfillable, and
  43/46 decisive-suite cases oracle-isomorphic — the 3 mismatches are the
  *live* event-built tree being an incomplete proof at transpositions
  (reconstruction is a strict superset), a pre-existing proof-tree-layer
  defect now tracked under #6. Verdict: **GO for plan6** (re-justified
  coverage criterion in `report5.md`), conditional on the finalize
  canonicalization fix.
- **2026-09-11** — plan6 drafted (#6): the finalize/validator fix becomes its
  own plan (one lever per plan), and the worker-off flip is renumbered from
  "plan6" (report5's numbering) to **plan7**, gated on plan6's 46/46
  oracle-isomorphic acceptance criterion.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
