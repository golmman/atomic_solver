# Initiative: `lean` — reducing wall time and nodes searched

## Status

Active. Plan 1 (`report1.md`) and plan 2 (`report2.md`) are done. Plan 3
(upstream `has_legal_move` fast path) is specified in `plan3.md`.

## Motivation

The `nn` branch (archived, `docs/plans/nn/report8.md` on that branch)
settled the move-ordering question: the headroom is slim. The oracle-floor
measurement put the best possible per-node ordering improvement at
**0.509x evals work-weighted / 1.029x unweighted**, and the decomposition
showed **90.6% of OR-node work is already spent on the decisive child**
(recoverable ≤ 9.4% locally). The learned-ordering PoC was closed
permanently. Anything that only ranks the winning OR child earlier — NN
round 2, win-length-aware rankers — cannot beat that ceiling and is out
of scope.

Ordering being closed does not mean the solver is at its speed limit. A
fresh audit of the hot path plus a live measurement on the dominant
benchmark case (`m22_white`, 2026-09-07) found levers that are
independent of ordering quality:

- **Refinement tail:** default-mode `solve` spends a large fraction of
  wall time searching futilely *after* the shortest line is already
  found. Measured on m22_white with `--timeout 20`: first win at ~9.9 s,
  refinement 155→23 plies done by ~11.1 s, then **~8.9 s (44% of wall)
  of futile searching for a <23-ply line until timeout**.
  `--first-outcome` finishes the same position in 10.3 s.
- **Redundant work per node:** legal moves are generated up to three
  times per visited node (`evaluate_child`'s terminal check via
  `Position::outcome`, the child's `dfpn` entry, and `sort_moves`' state
  rebuild); TT probes scan the same bucket up to 3× per node plus 2× per
  child evaluation; `try_use_tt` clones the whole `Position` on every
  solved hit; per-node scratch vectors allocate on the hot path.
- **The only multiplicative lever is parallelism**, which a prior plan
  attempted once and reverted (dfpn/report1) — feasible, but it must be
  designed around the deterministic child-eval budget contract first.

## Goal

Reduce solver wall time and nodes/child-evals on the benchmark suites
without changing search semantics: identical outcomes, and
(default-mode) identical proven lines. Ordering output and search
decisions must remain **bit-identical** unless a plan explicitly says
otherwise and validates the change against the move-order benchmark
suite.

Priorities follow AGENTS.md: correctness first, then performance,
memory, maintainability.

## Backlog (sorted by potential, confidence-weighted)

| # | Item | Mechanism | Potential | Affects | Effort | Status |
|---|------|-----------|-----------|---------|--------|--------|
| 1 | Cap the post-optimal refinement tail | Work-based per-round refinement cap (deterministic); stop when a round exhausts its cap without a shorter line | ~40–45% wall on default-mode hard cases | wall + evals | S | **plan1** |
| 1a | Refinement accounting | Lock cumulative node/eval accounting with tests; expose per-phase (first-outcome vs refinement) work so metrics are honest | correctness of metrics, no speed | metrics | S | **plan1** |
| 4 | TT probe consolidation + drop `Position` clone in `try_use_tt` | One bucket scan per node (probe/summary/best_move all delegate to the same slot today); `do_move`/`undo_move` round-trip instead of cloning on solved hits | ~5–10% wall | wall | S | **plan1** |
| 3 | Eliminate redundant move generation | Cache/defer the child `MoveList`+`StateInfo` between `evaluate_child`'s terminal check, `dfpn` entry, and `sort_moves` | ~10–25% wall | wall | M | plan2 candidate |
| 6 | Allocation & state-rebuild removal | Scratch reuse for per-node `Vec<ChildInfo>` / per-sort `Vec<(Move,i32)>`; avoid `move_stack.clone()` per proof event; hoist per-iteration best-move filtering | ~3–10% wall | wall | S–M | plan2 candidate |
| 8 | Cheaper static scoring | `StaticAtomicScorer` does 2–5 sliding-attack scans per quiet move; precomputed/incremental attacks | ~5–15% wall | wall | M | plan2 candidate |
| 2 | Parallel search | Lazy-SMP-style parallel sibling children or parallel refinement roots over the shared TT | 2–8× wall on multicore | wall | L–XL | plan3 candidate, needs determinism design first |
| 5 | AND-side ordering signal (non-NN) | Counter-moves, AND-specific history, TT `work` feedback — disproving work concentrates in 1–2 replies per AND node (median max child-share 52.9%) | ~5–20% evals, regression risk (oracle hurt m24_white 2.1×) | nodes | M | open |
| 7 | Lazy/staged child evaluation | Min-heap: evaluate children in rank order as needed instead of all on first iteration | ~3–10% evals | nodes | M | open |
| 9 | 2–3-man atomic endgame tablebases | Leaf probes in shallow-material positions | huge where covered, negligible elsewhere | nodes | M–L | open |
| 10 | History/killer constant re-tuning | Never re-tuned after the GHI/twin removal; side-aware killers | ~0–5% evals | nodes | S–M | open |
| 11 | Upstream `has_legal_move` early-exit terminal check | movegen-side lazy generation with first-legal-move exit; `evaluate_child`'s terminal check becomes a boolean query + checkers/`occupied==2` classification instead of a full generation per evaluated child | ~20–40% wall | wall | M | **plan3** |

Statuses reference plans under `docs/plans/lean/`; an item is *open*
until a plan claims it.

## Non-goals

- Reopening OR-side move-ordering quality in any form (NN or
  hand-written). The `nn` measurement bounds it; see Motivation.
- Changing the proof-tree layer, the `ProofEvent` protocol, or the
  optimizer interface contract (`docs/spec/optimizer_interface.md`)
  beyond what a plan explicitly specifies.
- Nondeterministic search for the sequential path: the deterministic
  `child_evals` budget (`Search::set_child_eval_budget`) and its
  `ExitReason::BudgetExhausted` semantics must keep working exactly as
  documented.

## Measurement conventions

- `child_evals` is the preferred efficiency metric (per the optimizer
  spec); wall time is secondary and machine-dependent.
- Every performance change must be validated with a **bit-identical
  drift check**: `benchmark --suite quick --json --first-outcome`
  before vs after must produce identical `child_evals` per case, unless
  the plan intentionally changes search behavior (e.g. the refinement
  cap; in that case compare with the cap disabled).
- Live measurements for reports use `m22_white`
  (`tests/fixtures/move_order_positions.txt`) as the dominant case;
  raw outputs go under `docs/plans/lean/measurements/`.

## Roadmap

- **plan1** — accounting fix + quick wins #1 and #4 (done, `report1.md`).
- **plan2** — hot-path compute: #3 (+ #6, #8 share one profiling pass)
  (done, `report2.md`).
- **plan3** — upstream `has_legal_move` fast path (#11, the
  profile-gated leftover of #3; two phases: movegen crate, then solver
  integration).
- **plan4** — parallelism (#2), only after plan3 lands and with a
  determinism story for the budget contract.
- Items #5/#7/#9/#10 are claimed by later plans as capacity allows.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
