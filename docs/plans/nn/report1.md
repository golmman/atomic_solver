# Report 1: `move_order_fractions` example (Gate 0 measurement)

This report documents the implementation of `docs/plans/nn/plan1.md`: a new
example binary `move_order_fractions` that solves positions with the stock
solver and measures, for every OR (Win) node in the finalized proof tree, the
rank of the proven decisive child under the default static move ordering —
flat and weighted by subtree size. It answers Gate 0 of
`docs/plans/nn/concept.md` (is there headroom for a learned move ordering?)
and ends with a go/no-go recommendation.

## Summary

- Added `examples/move_order_fractions.rs` with CLI
  (`--fen`, `--suite move-order|decisive|all`, `--timeout`, `--epsilon`,
  `--tt-size`, `--pt-size`, `-h`), per-case and aggregate tables, and stderr
  progress. Unknown options exit with an error.
- Added `tests/test_move_order_fractions.rs` (RUN_LOCK-serialized, release)
  asserting exit code 0, an `or_nodes=` line, a rank table, and that every
  table block's `pct` column sums to ~100.
- Added the example to the `examples/` list in `AGENTS.md`.
- No changes to `src/`, `Cargo.toml`, or any fixture.

## Files changed

- `examples/move_order_fractions.rs` (new)
- `tests/test_move_order_fractions.rs` (new)
- `AGENTS.md` (examples list)
- `docs/plans/nn/report1.md` (this report)

## CLI and output schema

```
move_order_fractions [OPTIONS]
  --fen <FEN>        Solve a single position; printed as case name "fen"
  --suite <NAME>     move-order | decisive | all   (default: move-order)
  --timeout <S>      Search budget in seconds       (default: 5)
  --epsilon <F>      DF-PN+ threshold               (default: 0.125)
  --tt-size <MB>     TT size                        (default: 64)
  --pt-size <MB>     Proof-tree memory budget       (default: 256)
  -h, --help
```

Per case, after a normal search (no `--first-outcome`, so the refined solve
grows the proven tree for the full budget), the worker is finalized and each
OR node with at least one proven Loss child contributes one sample: the
minimum rank of its Loss children under the default `StaticAtomicScorer`
ordering (stably sorted descending by score, matching `sort_moves` minus
history/killer/TT), paired with the node's post-order subtree size. Output
on stdout:

```
=== m23_white  outcome=win  tree_nodes=16650  or_nodes=8325  timeout=yes
rank    nodes  pct      work    work_pct
1        5523    66.3%    28942     24.8%
2         717     8.6%     5254      4.5%
3         149     1.8%     6688      5.7%
>3       1936    23.3%    75906     65.0%
```

followed by a suite-level aggregate with the same rows over all cases.
Positions whose search hit the timeout are marked `timeout=yes` on the case
line; their partial trees are still analyzed. Cases with `or_nodes == 0`
print the case line and no table body.

## Measured numbers

Move-order suite (`--suite move-order --timeout 5 --tt-size 64`, 19 cases,
19×5 s ≈ 30 s wall; 14,863 OR-node samples):

| rank | nodes | pct | work | work_pct |
|------|------:|----:|-----:|---------:|
| 1 | 10334 | 69.5% | 55802 | 31.4% |
| 2 | 1759 | 11.8% | 22178 | 12.5% |
| 3 | 599 | 4.0% | 10392 | 5.9% |
| >3 | 2171 | 14.6% | 89156 | 50.2% |

`nodes` sums to 14,863 (the `or_nodes` aggregate); each `pct` column sums to
99.9 within rounding.

Decisive suite (`--suite decisive --timeout 5`, 23 cases, 23×5 s ≈ 50 s wall;
96,050 OR-node samples; includes the transposition-heavy `dec*` fixtures):

| rank | nodes | pct | work | work_pct |
|------|------:|----:|-----:|---------:|
| 1 | 62972 | 65.6% | 691278 | 43.4% |
| 2 | 11627 | 12.1% | 142800 | 9.0% |
| 3 | 7392 | 7.7% | 68742 | 4.3% |
| >3 | 14059 | 14.6% | 688168 | 43.3% |

The numbers above are from single runs at `--timeout 5` with the default
64 MB TT. The search is wall-clock driven, so a re-run shifts the counts by a
few percent; the two headline findings (rank-1 is ~⅔ of nodes but only ~⅓–½
of the work) were reproduced in both verification runs of each suite.

### Gate-0 answer

The Gate-0 question was: *what fraction of OR nodes already rank the decisive
child first, and how much of the search work sits at badly-ordered nodes?*

- Flat: the decisive child ranks first at **69.5%** of OR nodes
  (move-order) and **65.6%** (decisive). A perfect ranker can at most touch
  the remaining ~30–35% of nodes.
- Work-weighted: those rank-1 nodes account for only **31.4%** / **43.4%**
  of the subtree-size work. Rank-2..3 nodes add ~18% / ~13%, and **rank
  >3 nodes carry 50.2% / 43.3% of the work while being only 14.6% of the
  nodes**.
- The recoverable share (work-weighted OR nodes with `min_rank > 1`) is
  **68.6%** (move-order) and **56.6%** (decisive) — far above the ~15%
  threshold the plan conversation set.

## What was implemented vs. the plan

- The plan asked for four buckets `(1, 2, 3, >3)`, counts, percentages, and
  the work-weighted variants; all present. The column layout was adjusted to
  fit a fixed-width table; the bucket semantics are unchanged.
- The plan's "min_rank over Loss children" aggregation is implemented as
  written; with the OR-node early-stop the overwhelming majority of Win
  nodes have exactly one Loss child, so `min_rank` is that child's rank.
- The case line uses the plan's schema (`outcome`, `tree_nodes`, `or_nodes`);
  `timeout=yes` is appended only when `search.time_exceeded()`.
- The plan's risk table mentioned "defensive: if `do_move` asserts, report
  the node and skip the case." In practice the replay invariant held on all
  executed fixtures (verified by hash checks in development), so no skip
  path is wired in.

## Limitations

- **Static vs. runtime ordering.** The rank is the static `StaticAtomicScorer`
  term only. The runtime order also adds history, killer, and best-from-TT
  promotion, which are not recorded anywhere. This is the correct proxy for
  Gate 0 because the network is only meant to replace the static term
  (concept.md §6), but single-node `move_order_debug`-style comparisons and
  these fractional measurements can disagree.
- **Subtree size proxy:** the work weight is the post-order node count of the
  finalized tree, not a real per-child `child_evals` counter. This is a proxy
  per concept.md; the final decision should confirm with `child_evals`
  (already reported by the benchmark harness).
- **Timeout partial trees:** positions that hit the timeout contribute only
  the tree grown so far, which can be unrepresentatively small (e.g. `m20`
  contributes 0 OR nodes). The aggregate is dominated by the positions that
  proved something.
- **Unrealized roots.** A root that the search does not resolve within the
  budget is never realized (draw outcomes are never emitted as proof events,
  and a timeout leaves the root unproven), so the worker's `finalize()` would
  abort. The example synthesizes a Loss root, which keeps realized refuted
  (Win) children for analysis; roots with no realized children contribute
  `or_nodes=0`. In the move-order suite this affects the `m20`–`m22` cases
  that return `outcome=draw` at the 5 s budget (`m22_black` still yields 9 OR
  samples from its refuted children).

## Verification

```bash
cargo fmt --check        # clean
cargo clippy --all-targets  # clean
cargo test               # all active tests pass, including the new one
cargo doc --no-deps      # clean
```

Manual runs:

```bash
cargo run --release --example move_order_fractions -- \
    --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" --timeout 1     # instant, or_nodes=1
cargo run --release --example move_order_fractions -- --suite move-order --timeout 5
cargo run --release --example move_order_fractions -- --suite decisive --timeout 5
cargo run --release --example move_order_fractions -- --suite all --timeout 1 --tt-size 32
```

The `decisive` suite run exercises the copied-subtree replay invariant:
`dec10` alone produces 84k OR nodes (87% of the suite total), and every
stored node hash matched the hash of the replayed position, so the
hash-matched canonical copies are legal and hash-consistent.

## Next steps

1. **Go.** The recoverable-work share (~57–69% of the node work sits at OR
   nodes whose proven child is not rank-1) is a large signal that a learned
   ranker has room to act. Proceed to the corpus-generation plan (Gate 1).
2. Gate 1 needs a corpus example that replays each `.bin` dump, materializing
   per node `{hash, fen, legal_moves, outcome, depth, subtree_size,
   first_decisive_rank}` deduplicated by Zobrist hash, as NDJSON for the
   external trainer; hold out the move-order suite for evaluation.
3. Before training, confirm the subtree-size proxy with real `child_evals`
   counters (optional ablation; concept.md allows either).
4. After inference lands (Gate 3), re-run this example and the benchmark
   harness with identical `--epsilon`/`--tt-size`; success requires a
   simultaneous `child_evals` and wall-time reduction with `wrong == 0`
   (concept.md Gate 4).