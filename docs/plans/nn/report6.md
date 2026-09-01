# Report: Gate 4 — measurement of the learned move-ordering network

Plan: `docs/plans/nn/plan6.md`. Status: **measured; success bar NOT met —
the wall-time half of the gate fails.** Raw outputs:
`docs/plans/nn/measurements/gate4/`.

## Setup

- Baseline: tuned `StaticAtomicScorer` (default `ScorerParams`).
- Network: `data/corpus/weights.v1.bin` (Gate 2) through the Gate-3
  inference path (`NnMoveScorer` via `--nn-weights`).
- All runs: `--epsilon 0.125 --tt-size 64`, suite `move-order`
  (19 cases, m20–m29 white/black; m29 white only).

## Benchmark (`--suite move-order --first-outcome --timeout 5 --runs 3`)

| aggregate    | baseline |    nn | delta |
|--------------|---------:|------:|------:|
| total_nodes  |  5467401 | 3047980 | -44.3% |
| child_evals  | 101121638 | 56675025 | -44.0% |
| total_time s |   34.461 | 37.325 | **+8.3%** |
| solved / timeouts | 13 / 6 | 12 / 7 | |
| wrong        |        0 |      0 | pass |

Per-case highlights (full table in the raw JSONs):

- m20–m22 (held out, both colors): child_evals -45.6%…-50.1% at the pinned
  5 s timeout — both configs time out, so wall time carries no signal.
- m23_white **regressed** from solved in 3.32 s to timeout.
- On every case solved by both configs the network is slower
  (+90%…+310% wall); child_evals are mixed there (-29.5%…+59.6%).

## Why the child_evals win is an artifact

The network lowers the evaluation *throughput*: at `--timeout 5`,
m20_white performs 15.28 M evals baseline vs 8.03 M with the network
(~1.9x slower per eval). The -44% child_evals at fixed wall time mostly
measures that slower engine, not better ordering. The deconfounded runs
confirm it — `--timeout 60 --runs 1` on the six held-out cases:

| case       | baseline evals / s | nn evals / s |
|------------|-------------------:|-------------:|
| m22_white  | 37.5 M / **11.45 s** | 66.0 M / 58.51 s |
| m22_black  | 228.3 M / timeout | **52.3 M** / timeout |
| m20–m21    | 180–209 M / timeout | 83–95 M / timeout |

- m22_white (solved by both): the network needs **1.76x more evals and
  5.1x more wall time** — the ordering is genuinely worse where the
  comparison is fair.
- m22_black and m20–m21: the network does explore far fewer nodes within
  60 s, but none of them solve, so this remains a throughput artifact
  with no decisive outcome to show for it.

## Gate-0 ordering-quality measurement (`move_order_fractions`)

Finalized trees differ between configs (the search itself is steered by
the ordering), so these are indicative only:

| metric                       | baseline |    nn |
|------------------------------|---------:|------:|
| aggregate flat rank-1 share  |    69.5% | 60.9% |
| aggregate work-weighted rank-1 share | 31.4% | 43.2% |

The network ranks the decisive child first less often overall, but more
often at the heavy OR nodes that dominate work. This did not translate
into search efficiency (see above).

## Verdict against the Gate-4 bar

| criterion | required | measured | result |
|-----------|----------|----------|--------|
| child_evals reduction | >=10-15% | -44.0% (confounded) | pass* |
| wall-time reduction   | >=10-15% | **+8.3%** | **fail** |
| wrong                 | == 0     | 0 | pass |

*\* pass only at fixed wall time; at fixed effort (m22_white) the network
needs more evals. **Gate 4 fails.***

## Interpretation

- The "Inference overhead" risk from `concept.md` §3 materialized exactly:
  a dense stage-2..5 pass per child per node costs more wall time than the
  node reduction buys. The forward pass runs per legal move per node in
  `sort_moves`, with no incremental accumulator (only stage 1 is
  incremental per spec §4).
- The rank labels were learned from trees produced under the static
  ordering (echo-chamber risk); the val loss (0.53 vs train 0.25 in
  `weights.v1.bin.json`) already suggested weak generalization, and the
  held-out m20–m22 results confirm it.
- No correctness harm: `wrong == 0` everywhere and outcomes agree except
  where the network turns a solve into a timeout (m23_white).

## Tools and commands

```
cargo run --release --example benchmark -- --suite move-order --first-outcome \
  --runs 3 --json [--nn-weights data/corpus/weights.v1.bin]
cargo run --release --example benchmark -- --suite move-order --runs 1 \
  --timeout 60 --first-outcome --json [--nn-weights data/corpus/weights.v1.bin]
cargo run --release --example move_order_fractions -- --suite move-order \
  [--nn-weights data/corpus/weights.v1.bin]
```

`make test` passes after the harness changes; `cargo fmt --check` and
`cargo clippy --release` are clean.

## Problems encountered

- Neither harness could load weights; `--nn-weights` had to be added to
  `examples/benchmark.rs` and `examples/move_order_fractions.rs` first
  (planned as Step 1).
- The fixed-wall-time benchmark design cannot separate "better ordering"
  from "slower engine"; the long-timeout rerun was needed to expose the
  confound (Step 3, added to the plan while executing).

## Unresolved parts

- The network as-is is not shippable; the `--nn-weights` path stays
  behind the CLI flag (off by default), which remains correct behavior.
- No per-move wall-time profile of `NnMoveScorer::move_scores` was taken;
  the throughput ratio (~1.9x slower) bounds it but a profile would say
  how much an incremental stage-2..5 could recover.

## Missing tests

- No automated test covers the new `--nn-weights` flags on the two
  examples (examples are untested by convention here); both were exercised
  manually in every measurement above, both with and without the flag.

## Next steps (only if the PoC is pursued further)

1. Make the network competitive on cost before quality: incremental
   stage-1 updates exist, but stage-2..5 re-evaluation per move dominates;
   consider batching all children through one dense forward pass per node
   and caching, or shrinking the hidden layer.
2. Train against the static ordering as a residual (predict the *correction*
   to `StaticAtomicScorer` ranks) rather than absolute ranks, and weight
   the loss by child work.
3. Re-run this exact Gate-4 harness; the bar stays as specified.
4. Otherwise: stop the PoC here — the tuned `ScorerParams` baseline remains
   the shipped ordering, and the measurement harness + recorded-work labels
   (design B) are the durable artifacts of Gates 0–4.
