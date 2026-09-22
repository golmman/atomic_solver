# Initiative: `solve` — atomic chess from the starting position

## Status

Opened 2026-09-21 as the repo's umbrella for the ultimate goal: establish
the game-theoretic value of the atomic chess starting position, delivered
as a **machine-verifiable proof artifact**. Next plan number: **plan1**.

This initiative renegotiates the scope of two standing no-gos (both remain
valid *in their recorded scope*):

- `lean` plan7 (deterministic parallelism, 1.47× ceiling) and `parallel`
  plan2 (option-C root-child race, 0.27×; `design_space.md` §6) measured
  **product-solver, single-position modes** against the product contract
  (bit-identical sequential path, deterministic budgets, interactive RAM
  envelope). A startpos campaign is a different computational object:
  a months-scale, throughput-oriented computation whose soundness story is
  the **verifiability of the final artifact**, not per-query properties.
  `parallel/report2.md` next steps and `design_space.md` §0 carry the
  scope note.
- `egtb` plan1 (≤4-men tables NO-GO *for faster solves of benchmark
  positions*, 0.000% child-eval share) does not bind the campaign: the
  men-count distribution at startpos-proof *leaves* is a different
  population and is measured by this initiative's plan1 (backlog #1).

## Motivation

Every cheaper lever family is measured out at the diagnostic level
(`research/structural_floor.md`); the parallel option space for the
product mode is closed by measurement (`parallel/report2.md`). The
remaining ambition — solving the game itself — is exactly what the repo's
tooling has been converging toward: proof-tree reconstruction + replay
validation + binary dumps (the verifiable-artifact backbone), a 3-man
atomic EGTB generator with an independent proof oracle, TT snapshots for
checkpointing, and a mined parallel-PNS literature with a proven
1024-core blueprint (Čížek 2025: job-level decomposition over persistent
private-state workers, 333×; JLPNS/PPN₂ partial pn/dn feedback; SPDFPN
for the shared-memory layer).

Why atomic chess is a realistic (if decade-scale) target: the explosion
rule liquidates material fast, so proof lines terminate quickly *given*
deep-enough anchoring tables; the hard part is width, which is the
job-level-parallel literature's home turf, and the GHI/repetition layer,
which is the genuinely open research risk (all mined papers are silent on
repetitions under parallelism; the journal contract
`dfpn/research_ghi_journal.md` stays load-bearing *within* workers).

## Goal

Prove the value of the startpos with an artifact that the existing
verification pipeline accepts offline: replay-validated proof tree(s)
/ PPV, anchored (where used) by independently cross-validated EGTB
layers. The optimization target is **wall(find outcome) + wall(verify
proof), combined**, on the reference hardware at each milestone.

## Design constraints (normative for this initiative)

1. **Verifiable artifact.** The deliverable is a proof the offline
   pipeline accepts (`reconstruct_pt` + validator, `verify_ppv`). No
   in-flight fact is trusted: composition into the master proof state
   only of (a) rule-derived facts, (b) replay-verified subtrees, or
   (c) facts independently re-derived (dual-build oracle class).
   Search-time soundness inside a worker is per-worker quality, not the
   gate — the gate is that nothing unverified reaches the artifact.
2. **Worker GHI correctness.** Repetition draws are real in this
   ruleset; every worker maintains the journal contract internally
   (repetition-dependent results never cached as path-independent).
   EGTB entries may compose as anchors only if generated GHI-correct
   and cross-validated (the `egtb_gen3` oracle discipline).
3. **Combined metric.** wall(find) + wall(verify) is the reported
   number at every milestone; designs that inflate the proof tree pay
   twice (search *and* verification) and the metric punishes them
   automatically.
4. **Checkpointability.** Months-scale runs on interruptible hardware:
   every level (worker TT, job store, master proof state, proof-tree
   events) must be dumpable and restartable; no correctness-relevant
   state may live only in one process's RAM.
5. **Sandbox envelope = dev/test target (≤ ~10 cores, 32 GB RAM).**
   The architecture must scale by *adding worker processes/nodes*, never
   by redesigning around shared memory beyond one node. Cluster-readiness
   criterion: the stage-3 prototype's software runs unchanged at 10
   workers here and at 100+ workers there.

## Current best architecture hypothesis

Job-level PNS over **persistent workers** (each worker an
unmodified-or-nearly-unmodified sequential solver instance with its own
TT, retained across jobs for the whole run) + a **master** holding the
proof state and feeding partial pn/dn feedback (PPN₂/PNS-PDFPN shape;
`research_cizek2025.md`, `research_jlpns.md`), anchored by verified EGTB
layers, with offline verification of the aggregated proof tree. Known
gaps: no prototype exists; GHI-correct EGTB generation beyond 3 men is
unproven; the durable job store does not exist. The plan2 spike's
negative result (root-child race without retention/feedback) is *not*
evidence against this shape — it measured the degenerate one-job-per-
worker corner of it.

## Backlog

| # | Item | Mechanism | Potential | Affects | Effort | Status |
|---|------|-----------|-----------|---------|--------|--------|
| 1 | **Reach-vs-depth spike + leaf profile** | Solve positions at increasing game depth (and the deepest reachable startpos-frontier positions); measure child-eval growth vs depth and the men-count distribution at leaves | sizes everything: EGTB depth requirement, growth-curve go/rethink gate, hardware sizing | #2, #5 | S | **executed** (`report1.md`, 2026-09-22): pre-registered gate fired **RETHINK** (reach arm: 7/8 lines terminate before ply 30; growth factor not estimable — cost is bimodal, not depth-driven). Leaf profile: ≤6-men share 0.000009% of 507M leaf sites, median 19 men ⇒ no generable EGTB depth anchors top-down leaves. First verify/find datapoint: 1.18 wall ratio. Decision pending (report1 §10) |
| 2 | **EGTB depth push 4→5 (→6) men** | GHI-correct generation (value iteration / retrograde over `atomic-movegen` semantics) with the independent proof-oracle cross-validation discipline at every layer; **reopens `egtb` per its handover rule** with the campaign's requirements | proof anchors; first hardware-consuming parallel workload (billions of independent positions) | #5 | M–L | **measured against by plan1**: no generable table depth (≤6 men) covers ≥0.001% of top-down leaf sites — as a *search anchor* this is dead; oracle-role or bottom-up-frontier scoping would need a new plan (report1 §4) |
| 3 | **Distributed job-level PNS prototype** | Master + persistent workers (one process each), durable job store, partial pn/dn feedback, TT retention measured over hundreds of jobs; runs at 10 workers in the sandbox unchanged at 100+ on a cluster | the campaign engine; tests the one architecture the no-go record never touched | #5 | L | open after #1 read |
| 4 | **Artifact pipeline at scale** | Checkpoint/restart semantics for all levels (per constraint 4); proof-tree event aggregation from N workers; storage sizing (TT snapshots, proof-tree dumps) | makes months-scale runs survivable | #5 | M | open |
| 5 | **Startpos frontier campaign** | Iterative deepening from the start: solve the value frontier at ply k with verified artifacts, push k; anchored by #2's tables, driven by #3's engine | the solve itself | — | XL | gated by #1–#4 |

## Non-goals

- **Product-solver features**: this initiative does not change the CLI,
  the sequential path's contracts, or the benchmark/drift gates (those
  belong to their own initiatives); campaign code lives outside the
  product surface (`examples/`, campaign tooling) unless a plan says
  otherwise.
- **Trusting in-flight parallel search results** (constraint 1): the
  campaign may run nondeterministically at full throttle; the artifact
  may not.
- **Unverified EGTB anchoring**: a table layer composes only after
  oracle cross-validation (constraint 2).
- Reopening the product-mode parallel no-gos (`lean` plan7,
  `parallel` plan2) — their scope is the CLI, not the campaign.

## Measurement conventions

- Reference hardware: this sandbox (≤ ~10 cores, 32 GB RAM, GPU
  available if a stage justifies it); every milestone reports the
  **combined metric** wall(find) + wall(verify) and the hardware it ran
  on. Cluster stages report per-node throughput and scaling efficiency
  additionally.
- Growth data (child evals / proof-tree size vs game depth) goes to
  `measurements/plan<N>/`; the go/rethink gate of backlog #1 is
  pre-registered in its plan before the runs.
- No campaign result is claimed proven without a validator pass
  (`reconstruct_pt --validate` / `verify_ppv`) recorded next to it.

## History

- **2026-09-21** — **initiative opened** (docs only): goal, constraint
  renegotiation (verifiable-artifact soundness replaces product-mode
  contracts; scope notes added to `parallel/design_space.md` §0 and
  `parallel/report2.md`), staged backlog #1–#5. Hardware disposition
  recorded: sandbox (≤10 cores / 32 GB) as dev/test target — sufficient
  for backlog #1, #3, #4 and 4-men EGTB; 5-men generation compute-bound
  (weeks on 10 cores; GPU value-iteration candidate); 6-men and the
  frontier campaign need the cluster (order: ≥100 cores, multi-TB
  storage).
- **2026-09-21** — **plan1 drafted** (not yet executed): reach-vs-depth
  spike + leaf men-count profile (`plan1.md`). Pre-registered gate:
  projected campaign work W = b^D_eff ≤ 10^15 nodes → GO;
  > 10^17 → RETHINK (calibrated against the checkers solve's ~10^14
  node touches + modern throughput headroom). Deliverables also include
  d_e (EGTB anchor depth covering ≥95% of leaf sites) for the `egtb`
  reopening plan and the first verify/find wall ratio datapoint for the
  combined metric.
- **2026-09-22** — **plan1 executed** (`report1.md`): 200-game
  random-playout control; 8 self-play ladders (all terminal at ply 6–38,
  PV-steered reach ≤ 38); 79 cost runs — 48 censored at a flat
  20.5–46.6M-node/120 s plateau, 27 uncensored of which 26 are cheap
  tactical proofs (several opening mainlines are forced wins ≤ 9 plies);
  temporary men-count instrumentation applied/reverted with byte-identical
  drift captures (507M non-terminal leaf sites: median 19 men, ≤6 men
  0.000009%, ≥95% coverage = 23 men); 1 GB-TT addendum −22.6% nodes;
  `reconstruct_pt --validate` ok on 5 positions, verify/find wall ratio
  1.18 at the deep boundary. **Pre-registered gate: RETHINK** (reach arm;
  W not defensibly estimable). Backlog #2/#3/#5 gated on the user's
  pivot/close decision — see `report1.md` §10.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
