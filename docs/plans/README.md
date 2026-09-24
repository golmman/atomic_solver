# Initiatives index

`docs/plans/` holds one directory per initiative. Each initiative may be
described by its `initiative.md` (status, goal, backlog, conventions,
history); the plan/report pairs in the directory are the work record.

**This file is a status index only.** The per-initiative `initiative.md`
is authoritative — do not duplicate backlogs or details here. Update a
row only when an initiative opens, pivots, or closes (not per plan).

Status vocabulary:

- **active** — has an open backlog or a pending plan; worked on.
- **dormant** — no open items, but a realistic trigger exists to reopen
  it (e.g. a housekeeping pass).
- **closed** — finished or superseded; kept for history. Its open
  threads, if any, moved to the successor noted in its `initiative.md`.

## Active

| Initiative                               | Focus                                                                | Next lever(s)                                                                                                        |
| ---------------------------------------- | -------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| [`lean`](lean/initiative.md)             | Wall time / nodes, bit-identical drift gate                          | #10 history/killer re-tune or the `dfpn`/`conversion` algorithmic items; #2 parallel search moved to `parallel` (2026-09-21)                 |
| [`parallel`](parallel/initiative.md)     | Parallel proof search (opt-in wall-time scaling)                     | **closed measured no-go (2026-09-21)**: plan1 reading round (Čížek 2025, JLPNS, Young 2016) excluded A/B/D by soundness/economics; plan2 sizing spike measured option C — the only constraint-satisfying shape — at 0.27× wall / 15.5× work inflation on m22 (`design_space.md`, `measurements/plan2/`); no parallel mode ships; deterministic parallelism stays a measured no-go (lean plan7) — **scoped to the product solver's single-position mode**; the campaign architecture for the startpos solve lives in [`solve`](solve/initiative.md) |
| [`solve`](solve/initiative.md)           | Solve atomic chess from the starting position (verifiable proof artifact) | plan2 M1 gate fired **NO-GO** (censored-run solved sections are search-local, not substrate; `report2.md`). **plan3 executed (2026-09-23, `report3.md`)**: quiet-root finishability **NOT-FINISHABLE at 2 h** (both TT arms censor; linear 198k nodes/s; 8× TT slower) and ±1-ply substrate GO only on the cheap tactical class. **plan4 executed (2026-09-23, `report4.md`)**: sharpness-first pilot passed both gates — DENSITY **GO** (55/400 = 13.75% of ply-2 replies are cheap forced White wins; 1.Nf3 refutes 17/20; d/c-pawn systems 0/200), VALIDATE **100%** (95-member artifact: 10,177 proven positions, DTM 1–13, 83 KB, find+verify ≈ 17 s); the cheap class **saturates** at startpos (deeper reach = the measured-blocked quiet moat), so close-with-artifact and sharpness-first converged; `pt_keys` off-by-one found by the census and fixed. **re-scoped, active (2026-09-23)**: the plan1–4 campaign line closed with the plan4 artifact frozen as the interim deliverable (95 validated proofs, 10,177 proven positions, DTM 1–13, `measurements/plan4/artifact/`); `pt_keys` off-by-one found by the plan4 census and fixed. **plan5 executed (2026-09-23, `report5.md`)**: SOUND **PASS** (all artifacts validate, 96 dual-checked facts, 0 contradictions); ECON as pre-registered (C2) **NO-GO**, but the C4 scaling gate passed with **GO-band economics** (1.29× wall / 1.98× inflation on shuffle-win, 5/5 proven) and retention demonstrated (C2-nr 0/10); recommendation: one bounded scheduler iteration before plan7. **plan6 executed (2026-09-24, `report6.md`)**: the bounded scheduler/budget iteration fired **Not-GO** — SOUND PASS (144 dual-checked facts, 0 contradictions), ECON **NO-GO** (winner V1/one-step-ladder completes 5/5 but at 0.63× sequential wall — the new 5/5-completeness clause exposed work-vs-wall substitution; V2 best economics 0.95×/1.57× but 3/5), C4′ FAIL, V3's registered abandon triggers structurally inert (0 fires in 30 reps); attribution recorded as **A5** in `campaign_architecture.md`. **plan7 is not draftable (cancelled)**; the measured open problem is the coverage race over the root's ~41 children (budget policy prices but cannot win it). Remaining lever: **plan8** off-sandbox resource sizing (independent; blocked on hardware access) — `solve` falls dormant only if plan8 also fires negative. Roadmap in [`solve/initiative.md`](solve/initiative.md). Falls to **dormant** only if the plan6 iteration and the plan8 sizing run both fire negative (then reopeners = mechanism innovation or resource step-change); never closes while the startpos value is the project goal |
| [`conversion`](conversion/initiative.md) | Deep tempo/progression conversions (`make stress` class)             | #6 threshold-cut-frame pricing **closed no-go** (plan6 diagnostic GO via the sweep short-circuit, but plan7 measured a 2.2–3.3× net loss with outcome regressions — `report7.md`); #7 default-ε 0.375 **closed won't-fix** (plan8: default-mode confirms FO gains, but quick 57/59 + thorough 34/66 regressions at 0.375 — `report8.md`); remaining: #5 item (e) reading; #2a ordering guidance parked; #4 parallel spike moved to `parallel` (2026-09-21) |
| [`cleanup`](cleanup/initiative.md)       | Housekeeping: DRY, YAGNI, lints, module sizing                       | dormant-as-needed; `dfpn/mod.rs` size watch (40 KB)                                                                   |
| [`movegen`](movegen/initiative.md)       | _Special case:_ cross-repo plans for the `atomic_movegen` dependency | new upstream asks follow the same standalone-plan pattern                                                            |

## Dormant

| Initiative                   | Focus                                                                                                          | Reopen trigger(s)                                                                                                              |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| [`proof`](proof/initiative.md) | Proof construction landed (search → TT snapshot → reconstruct → validated dump); PPV productization closed won't-fix | #5 TT checkpoint when multi-day runs need crash resilience; #7 deep-proof capacity spike when builder RAM overflows              |
| [`dfpn`](dfpn/initiative.md) | Backlog empty (#1–#6 closed; #3 subsumed by `--refine-cap 0` 2026-09-17); pricing observation moved to `conversion` #6 | A new measured search-semantics diagnostic for the deep, repetition-dominated class; or a soundness regression traced to DF-PN+ semantics |
| [`egtb`](egtb/initiative.md) | Refocused to dormant (plan4): the 3-man generator lives on as the `egtb_gen3` correctness oracle; faster-solves is NO-GO | A future 4-man effort (coverage, DTZ granularity, probe integration, proof-tree anchoring) reopens the initiative with a fresh plan2 |

## Closed

| Initiative                                 | Focus                                                                 | Successor / where threads went                                      |
| ------------------------------------------ | --------------------------------------------------------------------- | ------------------------------------------------------------------- |
| [`review`](review/initiative.md)           | Full-solver code review series (18 plans)                             | `cleanup` for housekeeping                                          |
| [`ultimattt`](ultimattt/initiative.md)     | Porting `ultimattt` techniques (work-chunk ID, early exit, TT `work`) | absorbed into the solver; `dfpn` for further algorithm work         |
| [`speed`](speed/initiative.md)             | Wall-time micro-optimizations                                         | `lean` (same mandate, formalized drift gate)                        |
| [`storage`](storage/initiative.md)         | Persistence, pre-exit artifacts, proof dump                           | `proof` (live-tree path removed by its plan7)                       |
| [`arch`](arch/initiative.md)               | `search` ↔ `proof_tree` decoupling via `ProofEvent`                   | done; `ProofSink` remains an AGENTS.md stretch goal                 |
| [`testability`](testability/initiative.md) | Test tiers, deterministic eval budgets                                | conventions now normative in AGENTS.md                              |
| [`move_order`](move_order/initiative.md)   | Ordering heuristics for the m20–m29 class                             | closed by `lean`'s oracle-floor measurement; tuning via `tune`/spec |
| [`research`](research/initiative.md)       | Node-count research (plans 1–8) pivoted to the structural floor: the consolidating no-go record [`structural_floor.md`](research/structural_floor.md) (plan10, closed 2026-09-21) — what the solver is locked into and why, with evidence pointers | open threads → `parallel` (parallel search, opened 2026-09-21 from `conversion` #4 / `lean` #2) / `lean` #10; reopeners start from `structural_floor.md` §9 |
| [`tune`](tune/initiative.md)               | External optimizer interface (`benchmark --json`)                     | contract is normative in `docs/spec/optimizer_interface.md`         |
| [`pv`](pv/initiative.md)                   | Shortest-PV / PPV correctness (pivoted)                               | PPV productization closed won't-fix (proof decision record 2026-09-17); shortest-PV via refinement/`PvStatus` in the solver |
