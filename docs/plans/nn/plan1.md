# Plan 1: `move_order_fractions` example (Gate 0 measurement)

## Goal

Add `examples/move_order_fractions.rs`, an example binary that solves
positions with the stock solver and reports, for every OR (Win) node in the
finalized proof tree, the rank of the proven decisive child under the current
static move ordering. Output is a per-case and aggregate table with two views:

- **flat**: fraction of OR nodes whose decisive child ranks 1st, 2nd,
  3rd, or lower;
- **work-weighted**: the same fractions weighted by the subtree size of each
  OR node, i.e. how much of the solver's node-work sits at nodes whose
  decisive child was ranked late.

This answers Gate 0 of `docs/plans/nn/concept.md`: *is there measurable
headroom for a learned move ordering?* If e.g. 80% of OR nodes already rank
the decisive child first, a perfect ranker can at most touch 20% — and the
work-weighted view shows whether the remaining 20% is cheap or expensive.

## Background

- The search stops expanding an OR node at the first decisive child
  (src/search/dfpn/children.rs, `evaluate_all_children`), so a Win node's
  children in the finalized tree are exactly the proven decisive moves — in
  practice one Loss child per Win node. OR nodes are therefore the natural
  unit for measuring ordering quality.
- The finalized proof tree is available in memory after
  `ProofTreeWorkerHandle::finalize()` + `tree()`
  (src/proof_tree/worker.rs); each node carries `mv`, `hash`, `outcome`,
  `depth` (src/proof_tree/node.rs).
- The production sort path scores moves with
  `StaticAtomicScorer::score_with_map(board, m, &state, &nearest, is_or_node)`
  plus history, killer, and best-from-TT promotion, sorted descending,
  stably, by `sort_by_key(Reverse(score))` (src/search/dfpn/history.rs).
  `score_with_map` and `nearest_commoner_map` are public
  (src/search/ordering.rs).
- The worker wiring (spawn, event sender, solve, finalize, tree) is
  demonstrated in src/main.rs.

## Scope

In scope:

- `examples/move_order_fractions.rs` with CLI and per-case/aggregate reporting.
- A light integration test under `tests/` exercising the example binary.
- `AGENTS.md` example list entry.
- `docs/plans/nn/report1.md` (final report).

Out of scope:

- Any change to the solver, the proof-tree worker, the `.bin` dump format, or
  the scorer.
- Weighted-by-`child_evals` labels (real work counters); the subtree-size
  proxy per concept.md is sufficient for Gate 0.
- AND-node analysis (Loss nodes rank all children; needs more thought, and the
  Gate-0 question is about the decisive-child signal at OR nodes).
- Suites `quick`/`thorough`; the move-order and decisive fixtures suffice.

## Design

### 1. CLI

```
move_order_fractions [OPTIONS]
  --fen <FEN>        Solve a single position; printed as case name "fen"
  --suite <NAME>     move-order | decisive | all   (default: move-order)
  --timeout <S>      Search budget in seconds       (default: 5)
  --epsilon <F>      DF-PN+ threshold               (default: 0.125)
  --tt-size <MB>     TT size                        (default: 64)
  --pt-size <MB>     Proof-tree memory budget       (default: 256)
  -h, --help         Print help and exit
```

- Unknown options exit with an error (same convention as the other examples).
- Suites load through `examples/common.rs` (`load_move_order_suite`,
  `load_decisive_suite`); `all` concatenates both.
- The scorer is always `StaticAtomicScorer::default()`, matching the default
  solver config. No `--config` flag in v1: the solve and the ranking pass
  must use the same parameters by construction, which the default does.

### 2. Solving

For each case, mirror src/main.rs wiring:

1. `Position::from_fen(fen)`.
2. `Search::new(tt_size)`, `set_timeout`, `set_epsilon`. **Do not** set
   `first_outcome_only`: the default solve path keeps refining within the
   timeout, growing the proven tree and giving the measurement more OR nodes.
3. `ProofTreeWorkerHandle::spawn(fen, pt_size, Arc::new(AtomicBool::new(false)))`;
   `search.set_proof_event_sender(handle.event_sender())`.
4. `search.solve_with_progress(&mut pos, |o, line| eprintln!(...))` for
   per-case progress on stderr.
5. `handle.finalize()`; `let tree = handle.tree()`.

### 3. Analysis

1. **Subtree sizes**: compute `Vec<u64>` post-order over `tree.children(id)`
   (iterative traversal; tree depth is bounded by solution depth but keep the
   stack explicit).
2. **DFS replay**: walk the tree with a single mutable `Position`,
   `pos.do_move(child.mv)` on descent and `pos.undo_move` on ascent, so each
   node is materialized once. Descend all children regardless of outcome.
3. For each node `n` with `outcome == Some(Outcome::Win)` that has at least
   one child with `outcome == Some(Outcome::Loss)`:
   - `let mut moves = MoveList::new(); pos.legal_moves(&mut moves);`
   - `StateInfo::new()` + `pos.populate_state(&mut state)`; `nearest =
     nearest_commoner_map(pos.board(), them)`.
   - Score every legal move with
     `scorer.score_with_map(pos.board(), m, &state, &nearest, /*is_or_node*/ true)`
     and sort descending, **stably**, by `(score)` as in
     src/search/dfpn/history.rs (no history/killer/TT — see Limitations).
   - `rank(m) = index + 1` in the sorted list. `min_rank = min rank` over
     children of `n` with `outcome == Some(Loss)`.
   - Record `(min_rank, subtree_sizes[n])`.
4. Position children (the DFS board) must satisfy the invariant that each
   child's `mv` is legal from the parent board. This holds because the worker
   copies hash-matched subtrees (`finalize()`); verification exercises
   transposition-heavy cases (see Matrix).
5. Defensive skips: nodes with `outcome == None`, Win nodes with no Loss
   child, and childless (terminal) nodes do not contribute to `or_nodes`.

### 4. Output

stdout is reserved for the tables; progress/errors go to stderr. Per case:

```
=== m23_white  outcome=win  tree_nodes=4831  or_nodes=642
rank <= R    nodes  pct      work   work_pct
1            412    64.2%    12830  61.0%
2            98     15.3%    3410   16.2%
3            61     9.5%     2200   10.5%
>3           71     11.0%    2590   12.3%
```

Then a suite-level aggregate with the same rows over all cases. Exact column
labels may be tuned by the implementer; the four buckets (1, 2, 3, >3), the
counts, the percentages, and the work-weighted variants must all be present.
If a position does not finish (timeout) its partial tree is still analyzed and
marked `timeout=yes` on the case line.

### 5. Integration test

`tests/test_move_order_fractions.rs` (reuse the `RUN_LOCK` serialize pattern
from `tests/test_benchmark_json.rs`): run the example with `--fen` on a small
decisive fixture (e.g. `4k3/8/8/8/8/8/8/4R1K1 w - - 0 1` or the shortest
decisive fixture), `--timeout 1`, assert exit code 0, stdout contains an
`or_nodes=` line and a `rank` table, and each row's `pct` column sums to
~100.

## Implementation steps

1. Add `examples/move_order_fractions.rs` (CLI, case loading, solve loop,
   analysis, table printing) as described above.
2. Add `tests/test_move_order_fractions.rs`.
3. Add the example to the `examples/` list in `AGENTS.md`.
4. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`.
5. Run the manual verification commands below on one small and one deep
   position, including at least one transposition-heavy fixture from
   `tests/fixtures/` (e.g. a `dec*` case) to exercise the copied-subtree
   replay invariant.
6. Write `docs/plans/nn/report1.md`.

## Files changed

- `examples/move_order_fractions.rs` (new)
- `tests/test_move_order_fractions.rs` (new)
- `AGENTS.md` (examples list)
- `docs/plans/nn/report1.md` (new, final report)
- `docs/plans/nn/concept.md` exists; extend "Status" only if the report
  changes the phasing.

No changes to `src/`, `Cargo.toml`, or any fixture.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test

# Small position: should finish near-instantly and emit a table.
cargo run --release --example move_order_fractions -- \
    --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" --timeout 1

# Move-order suite, the Gate-0 target.
cargo run --release --example move_order_fractions -- \
    --suite move-order --timeout 5

# Decisive suite, includes transposition-heavy cases.
cargo run --release --example move_order_fractions -- \
    --suite decisive --timeout 5
```

Sanity checks on the output: `or_nodes > 0` on at least one decisive case;
every `%` column sums to ~100 within rounding; the work-weighted view is
reported alongside the flat view.

## Limitations to document in the report

- The measured rank is the **static** ordering only. The runtime order also
  includes history, killer, and TT-best promotion, which are not recorded
  anywhere. This is the correct proxy for Gate 0 because the network is only
  meant to replace the static term (concept.md), but the report must state the
  distinction: `move_order_debug`-style single-node comparisons vs. actual
  fractional measurements can disagree.
- The `min_rank` aggregation follows the "any decisive child ranks first is a
  win for the ordering" semantics; with the OR-node early-stop this is almost
  always a single child's rank.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Analysis replay panics on a copied transposition subtree | The copy invariant guarantees legality (hash-matched); exercise decisive fixtures in verification. Defensive: if `do_move` asserts, report the node and skip the case. |
| Deep recursion in DFS or subtree-size pass | Iterative traversal; tree depth is bounded by solution depth anyway. |
| Timeout leaves a partial tree with unresolved dummies | `finalize()` removes dummies; mark `timeout=yes` and analyze what remains. |
| Suite run too slow for development | Each case is capped by `--timeout`; use `--fen` for fast single-case iterations. |
| Output table unreadable for cases with zero OR nodes | Print the case line with `or_nodes=0` and skip the table body. |

## Success criteria

1. `examples/move_order_fractions.rs` builds, runs the three verification
   commands, and prints the flat + work-weighted tables.
2. `cargo test` passes including the new integration test.
3. The report in `docs/plans/nn/report1.md` states the rank-1 fraction and
   the work-weighted rank-1 fraction over the move-order suite.
4. The report answers Gate 0 with a clear go/no-go recommendation:
   proceed to corpus generation only if the recoverable work fraction
   (work-weighted share of OR nodes with `min_rank > 1`) is meaningfully
   large (the plan-conversation threshold was ~15%).

## Final task

Write `docs/plans/nn/report1.md` describing:

- the example's CLI and output schema,
- measured per-suite numbers (rank-1 flat and work-weighted, rank-2/3/>3,
  `or_nodes` totals),
- any deviations from this plan,
- the static-vs-runtime-ordering limitation and its impact on
  interpretability,
- the Gate-0 go/no-go recommendation and, if "go", the concrete data needed
  for the corpus-generation plan.