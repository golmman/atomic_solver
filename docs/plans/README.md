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
| [`conversion`](conversion/initiative.md) | Deep tempo/progression conversions (`make stress` class)             | #6 threshold-cut-frame pricing **closed no-go** (plan6 diagnostic GO via the sweep short-circuit, but plan7 measured a 2.2–3.3× net loss with outcome regressions — `report7.md`); #7 default-ε 0.375 **closed won't-fix** (plan8: default-mode confirms FO gains, but quick 57/59 + thorough 34/66 regressions at 0.375 — `report8.md`); remaining: #5 item (e) reading; #2a ordering guidance parked; #4 parallel spike moved to `parallel` (2026-09-21) |
| [`cleanup`](cleanup/initiative.md)       | Housekeeping: DRY, YAGNI, lints, module sizing                       | dormant-as-needed; `dfpn/mod.rs` size watch (40 KB)                                                                   |
| [`movegen`](movegen/initiative.md)       | _Special case:_ cross-repo plans for the `atomic_movegen` dependency | new upstream asks follow the same standalone-plan pattern                                                            |
| [`proofdb`](proofdb/initiative.md)       | **Pivoted 2026-10-02 (owner decision): proof-db tooling, not database contents** — the deliverable is the harvester/merger/ledger/flip pipeline; harvesting batches are tooling validation, and DB growth beyond that is out of scope until the tooling is complete | **item 8 — tooling completeness audit + hardening** (the pipeline as a third party would run it: shards → merger → DB, harvest loop from a clean checkout); then website handoff (#5) and parallel harvesters (#6); plan11's pre-registered 18B `g1f3` rung and further ladder sweeps are parked unless re-framed as validation. Prior state (plan10, 2026-10-02, amended §9): 6 facts, falsified pilot pre-registration (session's cross-job private TT), arm A re-baselined by full in-session replay; standing layer DB `0d929f4c…` (55,703 nodes), 262 shards (`e91ad57b…`), ledger `de690bc1…` — the pipeline's latest validated state |

## Dormant

| Initiative                   | Focus                                                                                                          | Reopen trigger(s)                                                                                                              |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| [`proof`](proof/initiative.md) | Proof construction landed (search → TT snapshot → reconstruct → validated dump); PPV productization closed won't-fix | #5 TT checkpoint when multi-day runs need crash resilience; #7 deep-proof capacity spike when builder RAM overflows              |
| [`dfpn`](dfpn/initiative.md) | Backlog empty (#1–#6 closed; #3 subsumed by `--refine-cap 0` 2026-09-17); pricing observation moved to `conversion` #6 | A new measured search-semantics diagnostic for the deep, repetition-dominated class; or a soundness regression traced to DF-PN+ semantics |
| [`egtb`](egtb/initiative.md) | Refocused to dormant (plan4): the 3-man generator lives on as the `egtb_gen3` correctness oracle; faster-solves is NO-GO | A future 4-man effort (coverage, DTZ granularity, probe integration, proof-tree anchoring) reopens the initiative with a fresh plan2 |
| [`solve`](solve/initiative.md) | **dormant (2026-09-25)**: item-8 ladder complete — plan7 stage-1 **LINEAR** (R_seq = 198,229 nodes/s), plan8 stage-2 **FEASIBLE, GO-band** (m2 = 2.8176, R_camp = 558,529 nodes/s, N_floor′ = 8.043 G), plan9 stage-3 **ρ = 0.0 → verdict NEGATIVE** (Δfacts 30/0/0/0; restore machinery measured sound; A6). Earlier record: cheap tactical class real but saturating (plan4 artifact frozen as interim deliverable), quiet root NOT-FINISHABLE at 2 h (plan3), scheduler/budget family falsified (A5), M1 substrate NO-GO (plan2) | A reopener needs a fresh pre-registered plan: the coverage race (A5.4), budget-aware early-censor certificates (A6), or a resource step-change (re-prices T(W) at the same session economics, does not reopen) |

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
