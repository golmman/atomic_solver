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
| [`dfpn`](dfpn/initiative.md)             | Search semantics, algorithm-level node reduction (soundness-first)   | #3 refinement after cap-cut (the only open item); AND-side eval mass is threshold-cut frames (lean plan9 diagnostic)  |
| [`lean`](lean/initiative.md)             | Wall time / nodes, bit-identical drift gate                          | #10 history/killer re-tune or the `dfpn` algorithmic items; #2 parked dormant (plan7 spike)                          |
| [`proof`](proof/initiative.md)           | Independent, relocatable proofs for a discovered outcome             | #8 PPV from the finalized tree, #5 TT checkpoint, #7 deep-proof builder spike                                        |
| [`conversion`](conversion/initiative.md) | Deep tempo/progression conversions (`make stress` class)             | plan3 reading round (#5a/#5b: EWS, MOPNS), plan4 clock-pressure AND-side ordering (#3); #2a ordering guidance parked |
| [`cleanup`](cleanup/initiative.md)       | Housekeeping: DRY, YAGNI, lints, module sizing                       | pedantic-lint triage, `selection.rs` size watch                                                                      |
| [`movegen`](movegen/initiative.md)       | _Special case:_ cross-repo plans for the `atomic_movegen` dependency | new upstream asks follow the same standalone-plan pattern                                                            |
| [`egtb`](egtb/initiative.md)             | 4-man atomic endgame tablebase anchoring for faster solves           | plan1 go/no-go spike: men-count histogram + 3-man generator prototype                 |

## Dormant

_(Currently none; the section exists because "dormant" is part of the
status vocabulary — an initiative with no open items but a realistic
reopen trigger belongs here.)_

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
| [`tune`](tune/initiative.md)               | External optimizer interface (`benchmark --json`)                     | contract is normative in `docs/spec/optimizer_interface.md`         |
| [`pv`](pv/initiative.md)                   | Shortest-PV / PPV correctness (pivoted)                               | `proof` #8                                                          |
