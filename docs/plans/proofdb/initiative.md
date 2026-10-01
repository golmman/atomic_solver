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
| 2 | **Harvest loop** | campaign-worker-shaped runs producing shards (fresh root per job → solve → reconstruct → validate → manifest entry); merger runs after each batch | the DB grows at will, stoppable anytime | M | **done (plan2, 2026-09-29)**: standing shard dir + `proofdb_harvest`; first batch 12/87 decisive (all wins, plies 4–27), DB 12,081 → 12,655 nodes; gates H1–H5 pass; censored tail measured ≥ 40M evals at the 5 deepest quiet nodes |
| 3 | **DTM-upgrade pass** | background jobs re-searching bound ladders to promote `bound` → `exact` where affordable | depth quality improves without re-harvesting outcomes | M | open |
| 4 | **Coverage policy** | which frontier children to open next (cheap-first gradient; sharpness like plan4's, not enumerative layers) | keeps per-fact cost low as the tree deepens | S–M | **done (plan3, 2026-09-29)**: frontier classes C1/C2/C3 + policies in `proofdb_harvest` (`--policy`); measured A/B at 300M evals each: `sharp-siblings` 90 facts/300M (29.99 facts/100M, median 44.7k evals/fact) vs `open-deepest` 0/300M; winner is the default; gradient saturates after its head (marginal P2: 0 facts) |
| 5 | **Website handoff** | ship `global_proof_store.md` to the external website project; agree on read-only DB consumption | the product surface of this initiative | S | open |
| 6 | **Independent harvesters** | partition the frontier across worker processes (by root-child subtree or class; per-worker shard staging, serialized manifest merge); campaign-prototype precedent (`examples/campaign*`, solve plans 5/9) | wall-time scaling of harvesting without touching solver semantics | M | open — the sequential prerequisite is done (plan4, 2026-09-30: the ledger holds exactly the pick-up state a per-worker partition needs); note report4 finding 3: within-layer selection needs a work-aware key before large batches are worth scaling |
| 7 | **Subtree-scoped harvesting** | `--root-fen` (+ optional `--root-path`) plumbs the harvest root by FEN, resolved within the merged DB tree (no match or ambiguous match aborts); a subtree *view* over the same startpos-rooted tree — shards, manifest paths, and grafting stay startpos-relative unchanged | harvest any position, the primary user surface for item 6's per-worker subtree partition | S | open — parked from plan4 (2026-09-30, docs-only amendment, plan4 decision 12 tombstone): the first batch runs at the startpos root, and the MVP limitation ("FEN must be a node of the merged DB tree") is expected to be revisited under item 6 |

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

- **2026-10-01 (latest)** — **plan8 drafted (docs-only plan session)**:
  the §4.6 closure-rule response (report7: sixth consecutive 0-fact batch;
  "another identical default batch is not a valid plan8"). The design
  dialogue **rejects the depth-rationing lever on arithmetic** (exposure
  caps cannot shrink the structural number-1 pool — unvisited children are
  counted via movegen; layer completion is unaffordable at ~27× layer
  growth; the ladder is plan6's measured no-go) and adopts the
  **fact-yield-oriented job-set change**: the new policy `and-close` whose
  job set is the tree's actual completion work — the missing replies of
  undecided rows, completion-gradient ordered. The plan names the
  **starvation mechanism** behind the six 0-fact sessions (the number ≥ 2
  pool — where all completion work sits — is unreachable under the fresh
  cascade) and pins the tree's near-closure structure by probe:
  `g1f3` (3 missing replies), `e2e3` (7), `e2e4` (7), `g1h3` (9), the root
  (13); completing any head row implies the root (either direction — the
  handover contingency is pre-registered with a conservative trigger).
  Two arms pre-registered: the **deep probes** (1B child-evals on `g1f3`'s
  three defenses — the first budgets one-to-two orders above the measured
  censor band on those positions: pilot 8-s ≈ 10–20M-equiv, 120-s sample
  ≈ 100–250M-equiv, 2-h ≈ 7–15B-equiv, in-harness 4M censors) and the
  **fresh-tail screen** (970 never-searched C3 replies at 4M — the
  class-closing probe). Normative deltas: the monotone revisit budget
  `max(2^passes × base, work_done)`, bump-only censors (decision 11
  superseded for this policy — no child exposure), flips stay derived
  (decision 10 stands, no propagation pass). `and-close` becomes the
  harvest default in every outcome; 0 facts everywhere escalates the base
  ×10 per factless batch. Ledger advance = union(arm1, arm2) — the plan7
  union mechanism's first production use. See `plan8.md`.
- **2026-10-01** — **plan7 drafted (docs-only plan session)**:
  batch 3 pre-registered as a **two-arm A/B over the standing post-plan6
  state** — arm 1 (control): the shipped default (rationed expansion,
  layer caps 24, reserve 0) from the standing ledger as-is; arm 2 (union):
  the identical config from a **ledger-unioned** copy that recovers the
  censor knowledge plan6's standing-ledger advance dropped (report6
  finding 4). Ledger records are selection state only (no outcomes), so
  the union is a conservative max-rule merge (`passes_failed` max, tie by
  `work_done`) with a soundness-empty surface — and the N-way primitive
  item 6's per-worker ledger merge needs. Probes on throwaway copies
  pinned both censuses (arm 1: 4,608 jobs; arm 2: 5,987 jobs) and the
  union composition (5,950 records = 4,569 standing + 1,381 recovered
  paths from arm A + 57 upgrades from A/B; ply-2 fresh 190 → 141). The
  re-censor metric (finding 4 made measurable: expected 24 vs 0,
  ≈ 96M-eval waste bound) and the decision rule are pre-registered;
  **§4.6 fixes the plan8 closure rule**: if the expected 0-fact result
  holds (sixth consecutive), another default batch is invalid — the
  depth-rationing lever (finding 1) or a yield-oriented selection change
  is mandatory next. Finding 1 is otherwise deferred to its own plan. See
  `plan7.md`.
- **2026-10-01 (later still)** — **plan6 executed (the selection
  mechanism + the A/B)**: the plan5 §2 decision implemented as the
  pre-registered config surface (`--pns-config`: eligibility,
  `interleave_k`, `reserve_share`, per-ply-layer visit caps, growth,
  rotation, rung cap; census gains `kind: expand|rung`) and A/B'd over
  the identical post-batch-2 state: arm A (plan4 selector control) ≡ arm
  A′ (degenerate config) **byte-for-byte** (the equivalence contract
  holds at full-batch scale); arm B fired exactly 9 reserve-bound rungs
  (root + seven ply-1 nodes eligible at start + c2c3's mid-session
  eligibility flip — the trigger works, all rungs censored); arm C (no
  ladder) reached ply 5 vs B's 4 at the same cap. **§4.5 decision rule →
  the ladder is a measured no-go**: the shipped default config is arm
  C's (compiled `reserve_share = 0.0`; the mechanism stays
  config-reachable). 0 facts in every arm; the standing ledger advanced
  to arm C's post-run state (`058a6202…`, 4,569 records). Findings:
  layer caps ration breadth per layer, not depth (arm B's ply-8 reach
  prediction wrong — measured 4); the reserve, not the rung cap or the
  eligibility, binds. Gates H1–H6 pass. See `report6.md`,
  `measurements/plan6/`, and `research_pns_ladder.md` (theory linkage:
  df-pn threshold escalation is the ladder's ancestor; the eligibility
  trigger is the original part).
- **2026-10-01 (later)** — **plan6 drafted; plan5's decision refined
  (docs-only, design dialogue)**: the within-layer decision became an
  explicit hybrid mechanism — breadth-PNS expansion stays the default;
  ladder escalation is granted only to nodes whose children are all
  visited (the eligibility trigger, from the user's argument that
  expansion *is* the parent's proof work), paced by an interleave and
  rationed by a reserve share + per-ply-layer visit caps; full config
  surface (`--pns-config` TOML) so rule variants are benchmarkable, with
  census gaining a `kind: expand|rung` field so transcripts trace where
  the split fires. plan6 pre-registers the A/B: status-quo control from
  the post-batch-2 state, its degenerate-config equivalence replay,
  mechanism defaults, and the reserve=0 ladder-deletion arm — the
  ladder's marginal value is the measured question, with a measured
  no-go as a valid outcome. Research note `research_pns_ladder.md`
  (plan6 D3) mines the vendored PNS/DF-PN theory (threshold escalation,
  Nagai) and records the provenance chain and rejected alternatives.
  See `plan5.md` (amendment) and `plan6.md`.
- **2026-10-01** — **plan5 pre-registered (docs-only plan session)**: the
  within-layer selection question (report4 finding 3) decided as a
  **per-layer budget split with a scheduled ladder reserve**; work-aware
  effective numbers rejected by a degeneracy lemma (every fresh number-1
  record has zero ledger state, so any ledger-derived key collapses to the
  current `(number, ply, path)`), and the pool-growth lemma shows the
  ladder can never fire in-session under any ordering (censor exposure
  ≈ +18.2 fresh records/visit vs ≤ 1 removal). Batch 2 is pre-registered
  as the unchanged-policy control over the exposed frontier (measured
  census: 1,483 jobs / 1,408 number-1 / 75 number-2, superseding report4's
  1,417 figure); the split itself is plan6 implementation work, A/B'd
  against the batch-2 control. See `plan5.md`.
- **2026-09-30** — **plan4 executed** (breadth-first PNS harvester):
  `--policy breadth-pns` shipped as the default (live `(number, ply, path)`
  queue over the open frontier; sidecar work ledger `data/proofdb_work.json`
  with censor-time frontier exposure and the lineage gate; working layer at
  gitignored `data/`). First batch: **0 facts / 300.0M child-evals — exactly
  the pre-registered expectation**; the live queue swept plies 0–2 fully
  (75 censored jobs at 4M), exposed 1,368 undecided children into the
  ledger (1,443 records total), and the first ladder rung never fired
  in-session. Gates H1–H6 pass (H5: 75/75 records + byte-identical
  ledger). Findings: plan4 §1's pre-registered census (7 implied wins +
  1 implied loss + 41 jobs) came from a no-movegen probe — the normative
  §2 rule (unvisited = 1 via movegen) yields 49 jobs, the 8 extra rows
  genuinely undecided; and a factless session degenerates to pure BFS —
  within-layer selection needs a work-aware key before the next large
  batch (see `report4.md` findings 1/3). DB unchanged (35,055 nodes,
  197 shards); `data/proofdb.db` byte-identical to plan3's committed
  grown2 DB.
- **2026-09-30** — **plan4 slimmed before execution (docs-only, second
  amendment)**: `--root-fen`/`--root-path` subtree scoping (checkpoint
  decision 12) parked as **item 7**. Reasons: the first batch runs at
  the startpos root (dead weight for the measurement), and the
  feature's only near-term consumer is item 6's per-worker subtree
  partition, whose requirements will likely reshape the MVP limitation
  anyway. Decision numbering in plan4 is unchanged (12 stays as a parked
  tombstone); the kickoff prompt and deliverables D2/task 1/§8 are
  updated in place. Frontier exposure (decision 11) and the live queue
  (decision 4 reworded) are untouched — they define what plan4 measures.
- **2026-09-30** — **checkpoint design review** (plan4 unexecuted,
  amended in place): the initiative was checked against the user's
  top-down harvest vision (N-thread runs from root or from any FEN,
  24h stoppable runs, merge appends, website consumes the extended
  tree). Agreed deltas, all folded into `plan4.md`: (1) the sidecar
  ledger is the pick-up state — a censored job's expanded-but-undecided
  children are recorded as known-open records, so the exposed frontier
  survives the run (no open-leaf shards; shard contract unchanged);
  (2) selection became a **live priority queue** with censor-time
  frontier exposure, replacing plan4's fixed passes — freshly exposed
  children (number 1) are explored before a censored node's doubled
  revisit, which is the layer discipline "next depth only when the
  current depth is explored/in flight"; (3) `--root-fen` (+ optional
  `--root-path`) plumbs the harvest root by FEN, resolved within the
  merged DB tree (ambiguity/absence aborts) — a subtree *view* over the
  same startpos-rooted tree, shards/grafting unchanged; (4) parallel
  harvesters stay item 6, sequenced after plan4's sequential
  measurement. pn/dn stay derived (DB tree + ledger), never stored —
  the ledger records exactly the non-derivable state. See `plan4.md`
  amendment note and decisions 3/4/11/12.
- **2026-09-29** — **pivot decided after plan3**: job selection moves from
  class gradients (plan3's C1/C2/C3) to **breadth-first PNS over the open
  frontier** (plan4): structural pn/dn numbers (unvisited = 1), min-pn at
  OR / min-dn at AND, prefer-closer-to-root ties, censored nodes re-visited
  at doubled work (geometric ladder). Rationale: the C2 cheap head is
  measured-exhausted, and the most proving lines are what the website's
  users want explored — the initiative goal stays *coverage AND proof*
  (metric = harvesting value, not root-solver speed; the handover rule
  stands). Numbers are selection state: sidecar work ledger under the new
  **`data/`** working layer (gitignored, with the derived DB), never in the
  DB, never facts. Parallel harvesters added as item 6 (sequential PNS
  measured first; campaign-prototype precedent). See `plan4.md`.
- **2026-09-29** — **item 4 executed** (plan3): coverage policy measured
  and shipped. `proofdb_harvest` now extracts the full frontier classes
  (C1 open / C2 unexpanded siblings of proving children / C3 unexpanded
  children of open nodes) with a free AND-completeness assert over every
  proven loss node, and orders jobs by pre-registered policies. The A/B
  at 300M child-evals each (fresh TT per policy, marginal-yield design):
  `open-deepest` 0 facts / 300M (baseline, as pre-registered);
  **`sharp-siblings` 90 facts / 300M (29.99 facts/100M, median 44.7k
  evals/fact, all ply-5 loss refutations under bound-1 win parents)**;
  `sharp-heavy-tail` marginal after P1's head: 0 facts at 300M screen +
  200M heavy — the sibling gradient saturates immediately after its head.
  Winner is now the default `--policy`. DB 12,655 → 35,055 nodes (75 open
  unchanged); standing shard dir at 197 shards. Gates H1–H6 pass; H5
  replay 385/385 screen records identical. See `report3.md`.
- **2026-09-29** — **item 2 executed** (plan2): standing shard directory
  (`shards/` + standing manifest), `proofdb_harvest` CLI
  (deterministic child-eval budgets, path-context replay, TT retention,
  screen + heavy tiers, stop conditions), first batch over the seed's 87
  open frontier nodes: 12 decisive (all wins, even plies 4–27 on quiet
  ladder lines), 75 censored (deepest 5 re-verified undecided at 40M
  evals); merge-after-batch → 12,655 nodes (75 open); gates H1–H5 pass;
  full batch replay byte-identical. One harvest-tool defect found and
  fixed (root-job false repetition from a self-seeded prefix; censored,
  no bad fact; see `report2.md` finding 1 — same latent issue noted in
  the campaign worker's `replay_path`).
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
