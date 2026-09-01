# Plan: Gate 4 — measurement of the learned move-ordering network

Status: implemented (see `docs/plans/nn/report6.md`).

This plan executes Gate 4 of `docs/plans/nn/concept.md`: re-run the Gate-0
measurement and the benchmark harness with identical `--epsilon/--tt-size`,
comparing the tuned `StaticAtomicScorer` baseline against the Gate-2-trained
network loaded via the Gate-3 inference path, and judge the result against
the hard success bar.

## Preconditions (all satisfied)

- Gate 2 weight file: `data/corpus/weights.v1.bin` (967,312 bytes, §10
  layout, trained per `data/corpus/weights.v1.bin.json` on the
  `atomic-corpus/2` corpus; m20–m22 move-order cases held out).
- Gate 3 inference: `src/nn/` + `Search::set_nn_scorer` + CLI
  `--nn-weights` (`docs/plans/nn/plan5.md`/`report5.md`).

## Step 1 — harness plumbing

Neither measurement example could load weights, so both gain an
`--nn-weights <FILE>` flag:

1. `examples/benchmark.rs`: parse `--nn-weights`, load the file once
   (hard-error on failure), set `NnMoveScorer` on every `Search` via
   `set_nn_scorer`, record the path in the JSON output as `nn_weights`.
   Bundle the per-run settings into a `RunConfig` struct (keeps
   `bench_case`/`run_once` under the clippy argument limit).
2. `examples/move_order_fractions.rs`: parse `--nn-weights`, use the
   network for the search itself, and rank the proven decisive children in
   `rank_samples` by `NnMoveScorer::move_scores` instead of
   `score_with_map` — mirroring `sort_moves`, which replaces (not adds to)
   the static term; history/killer are runtime state and are not part of
   the measured ordering either way.

## Step 2 — benchmark measurement (wall time + child_evals)

Identical settings for both configs: `--suite move-order --first-outcome
--epsilon 0.125 --tt-size 64 --timeout 5 --runs 3` (Gate-0 defaults;
`--runs 3` only stabilizes the wall-time mean, child_evals is
deterministic per case).

- Baseline: no `--nn-weights`.
- NN: `--nn-weights data/corpus/weights.v1.bin`.

Exit criteria: `wrong == 0` in both runs; per-case and aggregate
`child_evals` and `time_mean` deltas recorded.

## Step 3 — deconfounded long-timeout run

At `--timeout 5` the six hardest cases (m20–m22, white and black) hit the
timeout in both configs, so their wall time is pinned at 5 s and the
child_evals delta is confounded by eval *throughput* (the network lowers
the eval rate, so fewer evals fit into the same wall budget). Re-run the
m20–m22 cases with `--timeout 60 --runs 1` both ways to observe real solve
times where they solve at all.

## Step 4 — Gate-0 ordering-quality measurement

`examples/move_order_fractions --suite move-order` (defaults) both with
and without `--nn-weights`; compare flat and work-weighted rank-1
fractions over the finalized trees.

## Step 5 — verdict and report

Judge against the concept.md Gate-4 bar:

> >=10-15% reduction in `child_evals` **and** wall time versus the tuned
> `ScorerParams` baseline, with `wrong == 0`.

Write `docs/plans/nn/report6.md` with the raw numbers, the throughput
analysis, an honest verdict, and next steps. Store the raw measurement
outputs under `docs/plans/nn/measurements/gate4/` for reproducibility.
