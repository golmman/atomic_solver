# Initiative: `solve` — atomic chess from the starting position

## Status

Opened 2026-09-21 as the repo's umbrella for the ultimate goal: establish
the game-theoretic value of the atomic chess starting position, delivered
as a **machine-verifiable proof artifact**.

**Numbering.** Roadmap entries are *items* (item 7, item 8, …); execution
plans are numbered sequentially as `planN.md`, independent of item
numbers — each plan's header states which item it executes. plan6's
execution established that item 7 (campaign product surface) is not
draftable, so item 8 (resource sizing) is drafted as plan7 instead; the
earlier plan7 name is retired with it.

Next plan number: **plan9** (item 8, stage 3 — multi-session
checkpoint-resume accumulation; to be drafted in its own session).
**plan7 executed 2026-09-24 (`report7.md`)**: stage-1 gate **LINEAR**
(η = 1.0081; R_seq = 198,229 nodes/s certified over 1–4 h; N_floor =
2.87 G; max-RSS flat 234.7 MB; TT fill ≤ 1 h). **plan8 is drafted and
executed (2026-09-25, `report8.md`)**: stage-2 gate **FEASIBLE,
GO-band** — m2 = 2.8176 (R_camp = 558,529 nodes/s, DECAY not fired,
N_floor′ = 8.043 G, memory check passed at 4 workers); m2 is locked in
GO band, so the item-8 verdict (plan7 §4 bands) hinges on ρ alone.

**Pilot (2026-09-23, plan4):** the sharpness-first rescope (report2 §9
option 2) was tested before pivoting and passed both pre-registered
gates (DENSITY GO, VALIDATE 100%) — the cheap class is real, one-sided
and patterned (55 ply-2 refutations, 1.Nf3 refutes 17/20 replies), and
the pilot built a validated 10,177-position proof artifact (83 KB).
But it also **saturates**: deeper reach crosses the quiet plateau that
plans 1–3 measured blocked. The full four-option matrix is in
`report4.md` §4.

**Decision (2026-09-23, session close): the campaign line closes; the
initiative stays ACTIVE, re-scoped** — it is the project's umbrella, so
it never closes while the startpos value is the goal, and it does not
go dormant either: item 8 (the sizing ladder, drafted as plans 7–9) is
the pending work (the status legend's "active"). What closed is the
plan1–4 **campaign line**: the measured mechanisms (PV-ladder, SSFP
anchors, step-by-step narrowing at sandbox budgets) are dead, and the
sharpness-first rescope, as piloted, saturated. The plan4 artifact is
frozen as the interim deliverable. `solve` falls to **dormant** only if
the plan6 iteration and the item-8 sizing ladder (plans 7–9) both fire
negative — at that point it has no open items, just reopeners (mechanism
innovation or a resource step-change). `report4.md` §7 records the
amended recommendation.

## Roadmap (2026-09-23) — return path to the startpos value

The campaign line is closed; the initiative is re-scoped onto the
following pending plans (status: active).

Phase 0 (start of next session): publish the human-readable refutation
book (report4 §1 expanded with PVs) as `docs/plans/solve/book.md`.

Phase 1 — the work before `solve` returns (two independent tracks):

- **plan6 — bounded campaign iteration (scheduler/budget policy; report5
  §4 recommendation). EXECUTED 2026-09-24, Not-GO (`report6.md`).** Same
  harness, same gates; variants V0 incumbent (2/5, 0.90× wall), V1
  one-step ladder (**winner, 5/5 but 0.63× wall / 2.64× inflation → ECON
  NO-GO**), V2 two-step ladder (3/5, 0.95× / 1.57×), V3 nf + abandon
  trigger (triggers structurally inert, 0 fires in 30 reps). SOUND PASS
  (144 dual-checked facts, 0 contradictions); C4′ FAIL (0.18× wall, ~26×
  at 4 workers). Attribution recorded as **A5** in
  `campaign_architecture.md` §11: budget ladders price but cannot win the
  coverage race over the root's ~41 children; the registered
  scheduler/budget family is falsified as the fix. **item 7 cancelled**
  (not draftable under the pre-registered semantics).
- **item 8 — resource sizing (economic gate; rescoped 2026-09-24 from the
  off-sandbox 20–24 h single run into three ≤6 h in-sandbox stages).
  THE REMAINING LEVER.** The sandbox (4 CPUs, 8 GiB cgroup memory limit)
  is the reference environment; the offered step-up envelope (8 CPUs,
  16 GB, 12 h — for exceptional experiments) is the first scale-up
  target, not a prerequisite. Stages (each one session, hard 6 h cap):
  sequential d4d5-p2 ladder (1 h / 2 h / 4 h censoring arms) →
  **plan7, executed 2026-09-24 (`report7.md`): gate LINEAR** (η =
  1.0081; R_seq = 198,229 nodes/s certified over 1–4 h; N_floor =
  2.87 G; RSS flat 234.7 MB; TT fill ≤ 1 h);
  campaign arms (4 workers, memory-checked; executed as 1 h / 4 h caps
  — amended from the original "2 h / 4 h" wording at plan8 execution
  close, report8 §6, to fit the stage cap) → **plan8, executed
  2026-09-25 (`report8.md`): FEASIBLE, GO-band** — m2 = 2.8176,
  R_camp = 558,529 nodes/s, no DECAY, N_floor′ = 8.043 G nodes,
  κ ≈ 27.5 both sides, memory check passed (≈ 1 GiB tree-RSS vs the
  7 GiB watermark); multi-session checkpoint-resume accumulation →
  **plan9** (the verdict hinges on ρ: m2 is already ≥ 1). The
  combination rule (what the stages jointly imply for the finishability
  number) is pre-registered in plan7. plan3's linear rate makes the
  ladder the growth datapoint; converts "bottomless" into a number on
  the hardware actually available. Decides between "active" and
  "dormant" per the status rule.

(plan5 — the architecture design spike — is executed: `report5.md`,
`campaign_architecture.md`, the two-job prototype, and the bounded-
iteration recommendation that plan6 now carries.)

Phase 2 (only on plan6 GO + item-8 pricing, per report5 §4): item 7 =
campaign product surface (`--tt-load-path` to the plan5 soundness spec — built once and
reverted in plan2, rebuild to spec —, frontier dump, deterministic
resume, S-store v1 promoted out of `measurements/`); then a one-quiet-
system campaign pilot (gate: wall-time beat vs. single sequential run
at acceptable work inflation).

**Cancelled 2026-09-24: plan6 fired Not-GO (`report6.md`), so item 7 is
not draftable and Phase 2 is dead.** The campaign architecture's
confirmed mechanisms (retention, complete-job scheduling) and its open
problem (the coverage race, A5.4) are recorded in
`campaign_architecture.md` §11; any future campaign work starts there.

Not on the critical path (no-work recommendations): `egtb` stays
oracle-only (anchoring at generable depths is measured dead);
`parallel` stays closed (campaign scope delegated here); node-rate
work (`lean`/`tune`) only after item 8 prices the campaign.

**Open questions carried into the next session:** (a) ~~hardware/cloud
access for the sizing run~~ — resolved 2026-09-24: item 8 rescoped to
in-sandbox stages (the offered step-up envelope — 8 CPUs, 16 GB, 12 h —
is the first scale-up target, not a blocker); (b) ~~plan5 scope~~ —
resolved 2026-09-23: executed with the two-job prototype (plan5).

**Pivot (2026-09-22, after plan1's RETHINK verdict):** the campaign
mechanism is the **Solved-Set Frontier Push (SSFP)** with the
proof-cost gradient (backlog #6) — demand-driven solved-set growth, not
enumerative layers and not the ply-frontier ladder. Enumerative
bottom-up is rejected by counting (layer sizes ×16/man vs. a proof
population at 19–23 men); mechanism spec and pre-registered gates in
`plan2.md`.

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
| 2 | **EGTB depth push 4→5 (→6) men** | GHI-correct generation (value iteration / retrograde over `atomic-movegen` semantics) with the independent proof-oracle cross-validation discipline at every layer; **reopens `egtb` per its handover rule** with the campaign's requirements | proof anchors; first hardware-consuming parallel workload (billions of independent positions) | #5 | M–L | **closed (2026-09-22 pivot decision)**: report1 §4 measured no generable table depth (≤6 men) covering ≥0.001% of top-down leaf sites, and plan2 §1 rejects enumerative layer-push by counting; tables survive only as SSFP's eventual low-men anchor class (DTZ-class clock-aware semantics, not v0) |
| 3 | **Distributed job-level PNS prototype** | Master + persistent workers (one process each), durable job store, partial pn/dn feedback, TT retention measured over hundreds of jobs; runs at 10 workers in the sandbox unchanged at 100+ on a cluster | the campaign engine; tests the one architecture the no-go record never touched | #6 | L | rescope: the Čížek job-level shape is the **successor** to plan2's sequential v0 queue runner, not a parallel DFPN over one root; work starts after the SSFP pilot substrate gate (plan2 §4) |
| 4 | **Artifact pipeline at scale** | Checkpoint/restart semantics for all levels (per constraint 4); proof-tree event aggregation from N workers; storage sizing (TT snapshots, proof-tree dumps) | makes months-scale runs survivable | #5 | M | open |
| 5 | **Startpos frontier campaign** | The SSFP campaign proper: solved-set growth until the startpos's residual proof fits one run; verified artifacts composed offline per constraint 1 | the solve itself | #6 | XL | gated by #6's pilot | 
| 6 | **Solved-Set Frontier Push (SSFP) — substrate gate + mechanism + pilot** | Persistent solved set S (verified/provisional provenance classes, exact clock-ful keys, TT snapshot solved-section discipline); proof-cost gradient (cheapest frontier targets first); `--tt-load-path` / `--frontier-dump` product hooks; pre-registered M1 transposition-substrate gate and 2 h pilot | the campaign mechanism; replaces the RETHINKed ply-frontier ladder (#5's old shape) | #3, #5 | M | **executed (report2.md, 2026-09-22): pre-registered M1 gate fired NO-GO** (median cross-system value share 0.0000% over 1,438 pairs; even ±2-ply same-system pairs ~0% — censored-run solved sections are search-local, not substrate). §3 product surface not landed (gated on GO); pilot not run. Rethink pending: close-with-artifact vs. sharpness-first rescoping (report2 §9) |

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
- **2026-09-22** — **pivot decision + plan2 drafted**: user accepted the SSFP
  reframing (demand-driven solved-set push; enumerative bottom-up rejected by
  counting; product surface opened for `--tt-load-path`/`--frontier-dump`),
  chose the proof-cost gradient (easy proofs and disproofs first). Backlog
  #2 closed, #3 rescoped, #5 reshaped, #6 added. Pre-registered M1 substrate
  gate (cross-system value share: GO ≥5%, NO-GO <1%) fixed in `plan2.md`
  before any runs.
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
- **2026-09-22** — **plan2 executed** (`report2.md`): the 48 censored plan1
  snapshots regenerated sequentially (98.4 min; node fidelity 0.94–1.17×);
  pre-registered M1 substrate gate fired **NO-GO** — median directed value
  share 0.0000% over 1,438 cross-system pairs (zero-inflated: 75% of pairs
  share no keys at all), and the sharper finding: even ±2-ply same-system
  pairs are ~0% (d4_p9→d4_p11 0.08%), so censored-run TT solved sections are
  search-local, not a reusable solved-set substrate; secondary p30-tree key
  coverage ≤ 10.75%. §3 product hooks implemented early during the regen
  window, reverted after the verdict (fast gate green); pilot not run.
  Rethink pending per report2 §9 (close with the artifact vs. sharpness-first
  rescoping; recommendation: the latter).
- **2026-09-22** — **plan3 drafted** (not yet executed): the report2 session
  discussion (step-by-step proof/disproof narrowing) reduced to two untested
  assumptions; plan3 pre-registers their measurements: Task A = 2 h
  finishability probe of the d4d5-p2 quiet root (arms at 128 MB and 1 GB TT,
  gates FINISHABLE / NOT-FINISHABLE-at-10× / SPLIT), Task B = the ±1-ply
  parent→child substrate measurement plan2 skipped (median `avail` gate 5% /
  1%). No product changes; ~4.5 h sequential wall.
- **2026-09-23** — **plan5 drafted** (not yet executed): the roadmap's Phase-1
  intellectual gate, resolving the recorded open question (scope = with
  prototype, per the recommendation on record). Deliverables: D1
  `campaign_architecture.md` (GHI-correct job-level soundness contract:
  job = (subposition, clock, deterministic budget, context); split = proof
  events only, no repetition-dependent fact crosses a boundary; merge =
  global-context replay verification, per plan4's proven position-keyed
  composition contract; anchor clock-safety classes; checkpointability;
  cluster readiness) and D2 the two-job prototype (`examples/campaign*`,
  campaign code only). Pre-registered gates: SOUND hard (zero false
  decisive facts, artifact validates, dual-check) and ECON (C2: GO ≥ 1.2×
  wall AND ≤ 3.0× work inflation, calibrated against Kaneko/Čížek 2-worker
  figures vs the parallel-plan2 0.27×/15.5× corner and discounted for
  untuned-v0 overhead; NO-GO beyond MARGINAL bands) plus retention/feedback
  attribution ablations (C2-nr / C2-nf) gating GO alongside the economics.
  Independent of plan6; verdicts feed the plan7 threshold per the
  initiative's status rule.
Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
- **2026-09-23** — **plan5 executed** (`report5.md`, `measurements/plan5/`):
  D1 `campaign_architecture.md` written first (normative for plan7, §11
  records the prototype's four amendments — A1 worker-export
  self-containedness via the reconstruct pipeline, A2 hard merge tripwire,
  A3 job-local fill repetition residual, A4 read-only pn/dn accessor — the
  only lib-surface addition). D2 two-job prototype
  (`examples/campaign_{master,worker}`): SOUND gate **PASS** (0 verify
  failures post-fix, all proven-rep artifacts `validate: ok`, 96 dual-checked
  facts, 0 contradictions). ECON as pre-registered (**C2, primary**) fired
  **NO-GO** (1/5 proven, 0.76× wall, ~25× inflation); the **C4 scaling gate
  passed with GO-band economics** (5/5 proven, 1.29× wall, 1.98× inflation,
  all artifacts valid). Attribution: **retention pays decisively** (C2-nr
  0/10 proven, 227×/25× inflation); the v0 slice-and-rerank feedback loop
  does **not** pay (C2-nf beats C2 on m22) — recorded per the plan's rule.
  Exploratory d4d5-p2 pair: both censor at 30 min; the campaign shows
  steady leaf-level progress (2,356 jobs, 21 leaf proofs) without child
  resolution. Recommendation: **one bounded iteration** (scheduler/budget
  policy — the C2 failure is diagnosed as a worker-coverage pathology, not
  an architecture dead-end) before any plan7 work; the sizing run
  proceeds independently.
- **2026-09-24** — **plan6 drafted** (not yet executed): the bounded
  campaign iteration per report5 §4's recommendation — scheduler/budget
  policy only, same harness and gates, C4 as the registered design point.
  Registered variants: V0 (nf, incumbent), V1 (one-step budget ladder,
  `--slice 4M --max-slice 8M`), V3 (nf + narrow abandon trigger, the one
  campaign-side code change); V2 (two-step ladder) optional. New
  pre-registrations: ECON completeness clause (5/5 proven within the arm
  cap), C4′ no-regression, mandatory A5 attribution amendment to the
  architecture doc. The off-sandbox resource-sizing run renumbers to
  **plan8** (independent; combines only at plan7's threshold). A negative
  verdict falsifies the recorded scheduler diagnosis and is a valid
  completion; dormancy then follows the status rule.
- **2026-09-24** — **plan6 executed** (`report6.md`, `measurements/plan6/`):
  SOUND PASS (all artifacts validate, 144 dual-checked facts, 0
  contradictions); ECON NO-GO (winner V1 one-step ladder completes 5/5
  but at 0.63× sequential wall — the completeness clause exposed
  work-vs-wall substitution); C4′ FAIL (4/5, 0.18× wall, ~26×
  inflation); V3's registered abandon triggers structurally inert
  (0 fires in 30 reps). Attribution recorded as **A5** in
  `campaign_architecture.md` §11: budget ladders price but cannot win
  the coverage race over the root's ~41 children; the registered
  scheduler/budget family is falsified as the fix. Compute ≈ 3 h 50 m of
  the 4.5 h cap; item 7 (campaign product surface) not draftable.
- **2026-09-24** — **plan7 drafted** (not yet executed): item 8,
  stage 1 — the sequential d4d5-p2 sizing ladder. Fresh arms L1 (1 h)
  and L4 (4 h) at the reference config, command-identical to plan3 A1;
  plan3's 2 h A1 reused as the ladder midpoint (three fresh arms = 7 h
  breach the 6 h compute cap) under a pre-registered ±10%
  rate-comparability gate. Stage-1 gate: COMPLETED / LINEAR /
  DEGRADING / SUPERLINEAR on the work-growth exponent η. §4
  pre-registers the item-8 combination rule binding plan8/plan9:
  dfpn-node unit, R_seq → R_camp/m2 → ρ per stage, the conditional
  horizon table T(W ∈ {10^14, 10^15, 10^17}), and the verdict bands
  (POSITIVE ρ ≥ 0.7 ∧ m2 ≥ 1; NEGATIVE/dormant ρ < 0.4; MARGINAL
  between). Registered secondary: TT occupancy/fill point (not a
  progress metric — plan3's capacity fact). Compute ≈ 5.25 h of the
  6 h cap; no product changes.
- **2026-09-24** — **item/plan numbering adopted; item 8 rescoped.** The
  roadmap's plan7 name (campaign product surface) is retired — cancelled
  by plan6's execution; roadmap entries become *items* and execution
  plans stay sequential `planN.md` (each plan's header states which item
  it executes; convention also recorded in `AGENTS.md`). The off-sandbox
  20–24 h sizing run (old plan8) is split into three ≤6 h in-sandbox
  stages under **item 8**, to be drafted as plans 7 (sequential d4d5-p2
  ladder, 1 h / 2 h / 4 h censoring arms), 8 (campaign arms at 4
  workers, memory-checked, 2 h / 4 h caps), and 9 (multi-session
  checkpoint-resume accumulation); the sandbox (4 CPUs, 8 GiB cgroup) is
  the reference environment and the offered step-up envelope (8 CPUs /
  16 GB / 12 h, for exceptional experiments) the first scale-up target.
  Plan drafting happens in its own session (plan7 draft pending).
- **2026-09-24** — **plan7 executed** (`report7.md`,
  `measurements/plan7/`): stage-1 gate **LINEAR** — all three arms
  censor (L1 709.7M / M2 1,428.6M / L4 2,870.9M nodes at 1 h / 2 h /
  4 h), η = 1.0081, floors monotone, rates within ±0.7% (L1 −0.68%, L4
  +0.50% vs plan3 A1, admitted as midpoint); **R_seq = 198,229
  nodes/s certified over 1–4 h**, N_floor = 2.87 G, max-RSS flat at
  234.7 MB, TT fill point ≤ 1 h (128 MB table at capacity; TT keys
  confirmed non-metric). Stage-2/3 inputs and the §4 horizon table
  delivered with the campaign rows pending plan8.
- **2026-09-25** — **plan8 drafted and executed** (`plan8.md`,
  `report8.md`, `measurements/plan8/`): stage 2 = campaign arms at 4
  workers on d4d5-p2 (registered V1 shape, no variants; arms SM 600 s /
  S600 seq / **C4 4 h headline** / C1 1 h — the initiative's "2 h / 4 h
  caps" wording amended to 1 h / 4 h at close, report8 §6, to fit the
  stage cap). Gate: **FEASIBLE, GO-band** — m2(C4) = 2.8176
  (R_camp = 558,529 nodes/s = 2.82× R_seq; per-worker ≈ 0.70× R_seq,
  mildly declining with session length but above the 0.8 DECAY
  threshold), A5.4 falsified in throughput terms on this root (the §1
  private-TT economics confirmed); floor extended to N_floor′ =
  8.043 G nodes; κ ≈ 27.5 on both sides (conversion fixed);
  **memory check passed** (≈ 0.97 GiB peak tree-RSS, workers ≤ 609 MB,
  oom_kill 0 — no INFEASIBLE signal, CONFw2 not run). Proof state
  frozen at 0/27 children, 698 open leaves in every arm — plan9's ρ
  (multi-session accumulation) is the only remaining input to the
  item-8 verdict (POSITIVE needs ρ ≥ 0.7 ∧ m2 ≥ 1; m2 is in GO band).