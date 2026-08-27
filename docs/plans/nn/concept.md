# Concept: move-ordering neural network for atomic chess

## Status

Refined general idea for the PoC pipeline. The architecture spec is
`docs/spec/nn.md`. Execution status: Gate 0 (measurement,
`docs/plans/nn/plan1.md` + `report1.md`) and Gate 1 (corpus generation,
`plan2.md` + `report2.md`) are done; the `subtree_size` proxy was ablated and
rejected (`plan3.md` + `report3.md`) and replaced by real per-child `work`
counters in the proof tree (design B, `plan4.md` + `report4.md`, corpus
`atomic-corpus/2`); Gate 2's implementation plan is
`docs/plans/nn/plan_external_trainer.md` and its Docker setup handoff is
`docs/plans/nn/trainer_handoff.md`. This document records the _why_, the
pipeline, the decisions, and the honest risk assessment so later plans can
refer to a stable baseline of reasoning.

## 1. Idea

Replace (or augment) the hand-crafted `StaticAtomicScorer` with a learned
`MoveScorer` that ranks legal moves at a df-pn node by predicted resolution
cost, instead of by hand-tuned heuristics. The solver labels its own
training data: solve positions, harvest the finalized proof tree, train a
small network, and load the weights back at inference.

End-to-end loop:

```
solve batch  ->  harvest proof trees  ->  train (external)  ->  load weights
                                                        ^                 |
                                                        +----- measure ---+
```

## 2. Why this is worth attempting

- `StaticAtomicScorer` (src/search/ordering.rs) already demonstrates that
  move ordering is the performance lever: the repo contains a tuned parameter
  set, dedicated move-order benchmark fixtures, and an external optimizer
  contract (docs/spec/optimizer_interface.md) built around `child_evals`.
- In df-pn, the initial child order feeds the pn/dn initialization that steers
  node selection; a bad order is exponential, not a constant factor.
- The data is free: every solved position produces a finalized proof tree
  (`docs/spec/proof_tree_dump.md`) with no instrumentation required.
- The measurement harness already exists and is deterministic
  (`examples/benchmark --suite move-order --json`), so an honest comparison is
  cheap to produce.

## 3. Why it might fail (risk assessment)

| Risk                        | Assessment                                                                                                                                                                                                                                                                 | Mitigation                                                                                                                       |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| OR-node labels are censored | `evaluate_all_children` stops at the first decisive child and leaves the rest unexplored (src/search/dfpn/children.rs). At the nodes that matter most there is only one label per node: "the decisive child was found at rank k". No sibling cost pairs exist at OR nodes. | Use two target families: replicate-to-top at OR nodes; subtree-size ranking at AND nodes where all children are expanded.        |
| Multiple winning moves      | In atomic chess a node often has several decisive captures; ordering among "also winning" moves does not matter, reducing the effective signal.                                                                                                                            | Accept noise; the rank-1 criterion is the runtime-relevant one with `--first-outcome`.                                           |
| Headroom is thin            | `score_winning_capture` + atomic SEE already handle commoner captures that decide most games. The learnable surplus is concentrated in quiet-move and defusal ordering.                                                                                                    | The whole point of Gate 0 (plan1) is to measure whether the surplus is worth chasing before building the pipeline.               |
| Inference overhead          | A dense forward pass on every child of every node can cost more wall time than the node reduction it buys.                                                                                                                                                                 | Success bar requires BOTH `child_evals` and wall time to improve.                                                                |
| Echo chamber                | Training on the solver's own trees teaches "cheap under the current ordering"; positions the current ordering never expands never appear in the data.                                                                                                                      | Split train/validation; treat the learned scorer as a residual on top of the static bonuses rather than a wholesale replacement. |
| Small data                  | ~10^5-10^6 rows against ~250k+ weights in the 4096-policy spec. Effective signal is closer to 10^4-10^5 positions (one OR label per node).                                                                                                                                 | Tiny net (spec nn.md), rank-only loss, validate rather than fit.                                                                 |

## 4. Pipeline

### Gate 0: measurement (plan1)

Build an example (`move_order_fractions`) that solves positions and reports, for
every OR (Win) node in the finalized proof tree, the rank of the proven
decisive child under the current static ordering, both flat and weighted by
subtree size. Answers: _"what fraction of OR nodes already rank the decisive
child first, and how much of the search work sits at badly-ordered nodes?"_ If
the recoverable waste is small (<~15%), stop the PoC here.

### Gate 1: corpus generation (Rust side)

A new example that solves the `quick` + `decisive` suites at fixed
`--timeout/--epsilon/--tt-size`, and a loader that replays each `.bin`
(root FEN + move paths), materializing per node:
`{hash, fen, legal_moves, outcome, depth, subtree_size, first_decisive_rank}`,
deduplicated by Zobrist hash, serialized to NDJSON for the external trainer.
Train on these suites; the move-order cases m23+ are part of `quick` and
therefore train too — the honestly held-out move-order set is `m20..m22`
(6 cases).

### Gate 2: training (external)

External toolchain (PyTorch/numpy) consumes the NDJSON corpus and emits a
float32 weight file plus a small header; it runs in a Docker container outside
this repo. Network shape and loss are specified in `docs/spec/nn.md`; the two
open format questions are pinned: `policy_size` = 4096 (§5) and the exact
weight-file layout (§10). Implementation plan:
`docs/plans/nn/plan_external_trainer.md`; container setup:
`docs/plans/nn/trainer_handoff.md`.

### Gate 3: inference integration (Rust side)

- Generalize the scorer slot in `Search` (currently a concrete
  `StaticAtomicScorer`, see src/search/dfpn/mod.rs) to `Box<dyn MoveScorer>` or
  an enum so `sort_moves` (src/search/dfpn/history.rs) can use a
  `NeuralMoveScorer`.
- `NeuralMoveScorer` implements the incremental stage-1 accumulator from
  `docs/spec/nn.md` section 4 (make/unmake on a stack) and recomputes stages
  2-5 densely; the legal-move mask comes from the `MoveList` the search already
  generates.
- History, killer, and best-from-TT ordering stay; the network replaces only
  the static term.

### Gate 4: measurement (matching Gate 0)

Re-run the Gate-0 example and `examples/benchmark --suite move-order
--first-outcome --json` with identical `--epsilon/--tt-size`. Success requires

> =10-15% reduction in `child_evals` **and** wall time versus the tuned
> `ScorerParams` baseline, with `wrong == 0`.

## 5. Label semantics

- **OR node** (`outcome == Win` for the side to move): the decisive children
  are those with `outcome == Loss`. One-vs-rest pairwise target: the first
  decisive child must rank above every other legal move. Siblings with no
  recorded work are censored, never treated as "cheap".
- **AND node** (`outcome == Loss`): every child is expanded and solved, so
  rank the children by their recorded real work — the cumulative `child_evals`
  spent proving each child's subtree, carried by `ProofNode.work` (design B,
  `docs/plans/nn/plan4.md`) and emitted per child by `corpus_gen`
  (`atomic-corpus/2`).

Design A (`plan3.md`) measured whether derived post-order `subtree_size`
proxies this real work; the pair flip rate (28–29%) and work-weighted flip
share (45–48%) rejected the proxy, which is why the labels are now defined on
recorded `work` directly.

## 6. Key decisions

- **Labels**: AND nodes rank children by the recorded real `work`
  (`ProofNode.work`, dump v2); OR nodes carry `first_decisive_rank`, both
  derived offline from the dump by `corpus_gen`.
- **Toolchain split**: Rust emits the corpus; an external trainer produces the
  weight file; Rust loads it at inference.
- **Metrics**: `child_evals` (deterministic) AND wall time; hard success bar.
- **Ordering composition**: the network ranks legal moves; history/killer/TT
  remain additive. Final score is `nn + history + killer`.

## 7. Relation to existing artifacts

- `docs/spec/nn.md` — network architecture, features, layers, update rule.
- `docs/spec/proof_tree_dump.md` — `.bin` layout; subtree sizes are derivable
  post-order from the full adjacency dump.
- `docs/spec/optimizer_interface.md` — evaluator contract re-used verbatim as
  the NN measurement harness.
- `examples/benchmark.rs` — suite definitions (`move-order`, `decisive`,
  `quick`) and JSON output.
- `src/proof_tree/worker.rs` — `finalize()` + `tree()` give the authoritative,
  canonicalized proof tree in memory; no dump needed for analysis.

## 8. Phasing

1. `docs/plans/nn/plan1.md` + `report1.md` — Gate 0 measurement example.
   Done.
2. `docs/plans/nn/plan2.md` + `report2.md` — Gate 1 corpus generation. Done.
3. `docs/plans/nn/plan3.md`/`report3.md` + `plan4.md`/`report4.md` — the
   subtree-size proxy ablation; proxy rejected; real per-child `work` labels
   (design B, corpus v2). Done.
4. `docs/plans/nn/plan_external_trainer.md` + `trainer_handoff.md` — Gate 2
   external trainer (next; `trainer_handoff.md` is the Docker handoff).
5. Gate 3 Rust inference integration (future plan).
6. Gate 4 full measurement and report (future plan).

