# Optimizer Interface Contract

## Version

Version 1.

## Overview

This document defines the command-line interface that `atomic_solver` exposes
to an external optimizer. The optimizer's goal is to find `ScorerParams` values
that minimize the work required to solve the benchmark positions.

`atomic_solver` provides a deterministic, stateless evaluator. The optimizer is
responsible for proposing parameter values, writing a valid `config.toml`,
generating its own baselines, invoking the benchmark binary, parsing the JSON
output, and computing a scalar loss.

The application is responsible only for validating the config, running the
requested benchmark suite, and returning raw metrics as JSON.

## Provided evaluator

The evaluator is the release example binary:

```bash
target/release/examples/benchmark \
    --config <CANDIDATE_TOML> \
    --suite <SUITE> \
    --json \
    --timeout <SECONDS> \
    --runs <N> \
    [--first-outcome]
```

The JSON can be written to `stdout` or to a file:

```bash
--json                    # print JSON to stdout
--output-file <PATH>      # also write JSON to a file
```

The application does **not** compute a loss, does **not** store baselines, and
does **not** know about the optimizer's search space. It validates the config,
runs the suite, emits a JSON document, and exits `0` on success.

## Input contract

### Config file

The file passed to `--config` is a TOML file containing a partial or full
`[scorer]` table. Missing keys use the compiled-in defaults because
`ScorerParams` is annotated with `#[serde(default)]`. The application validates
the final config with `ScorerParams::validate()`; invalid configs cause a
non-zero exit.

Example partial config:

```toml
[scorer]
score_pawn_storm = 5600
score_pawn_storm_step = 110
and_pawn_storm_scale = 52
```

### Suite selection

The `--suite` argument selects the benchmark set. The optimizer-facing suite
names are:

- `quick` — fast feedback loop, roughly 10–15 seconds per evaluation.
- `thorough` — validation set, several minutes per evaluation.

The exact positions and timeouts are defined by the suite implementations in
`examples/benchmark.rs` and the fixtures in `tests/fixtures/`.

### Search controls

| Flag | Meaning |
|---|---|
| `--timeout` | Time budget in seconds. |
| `--runs` | Number of repeated runs for timing statistics. `nodes` and `child_evals` are deterministic, so one run is normally enough for the metric. |
| `--first-outcome` | Stop after the first decisive line. Recommended for tuning move ordering. |
| `--epsilon` | DF-PN+ threshold. Should be kept fixed during a tuning study. |
| `--tt-size` | Transposition-table size in megabytes. Should be kept fixed during a tuning study. |

## Output contract

With `--json`, the benchmark prints a single JSON object to `stdout` and, if
`--output-file` is supplied, writes the same JSON to a file. `stderr` may contain
progress logs (for example `[bounded_search] chunk done`) which the optimizer
must ignore.

### JSON schema

```json
{
  "suite": "quick",
  "mode": "first-outcome",
  "timeout": 3,
  "runs": 1,
  "epsilon": 0.125,
  "tt_size": 64,
  "config_path": "/tmp/atomic_solver_cand_abc123.toml",
  "results": [
    {
      "name": "dec01",
      "status": "ok",
      "outcome": "win",
      "expected": "win",
      "nodes": 284480,
      "child_evals": 5713086,
      "time_mean": 1.576,
      "time_min": 1.576,
      "time_max": 1.576,
      "pv_len": 27,
      "timeout": false,
      "wrong": false
    }
  ],
  "aggregates": {
    "total_nodes": 284480,
    "total_child_evals": 5713086,
    "total_time": 1.576,
    "solved": 1,
    "timeouts": 0,
    "wrong": 0,
    "mean_pv_len": 27
  }
}
```

### Status values

| Status | Meaning |
|---|---|
| `ok` | Returned a decisive outcome matching the expected value. |
| `timeout` | Hit the time limit before a decisive outcome. |
| `wrong` | Returned a decisive outcome that does not match the expected value. |
| `unknown` | Position has no expected outcome (neither `ok` nor `wrong` applies). |

## Error contract

| Condition | Exit code | `stdout` | `stderr` |
|---|---|---|---|
| Valid config, successful run | `0` | JSON | optional progress logs |
| Invalid config | non-zero | empty | error message |
| Invalid FEN or other runtime error | non-zero | empty | error message |

The optimizer must treat any non-zero exit as a failed evaluation and assign a
large loss.

## Loss computation

The application does not compute a loss. The optimizer loads its own baseline
run and computes a scalar. `child_evals` is the preferred efficiency metric
because it is deterministic. `WRONG_PENALTY` should dominate the loss, reflecting
correctness as the highest priority.

## Baseline generation

Baseline generation is the optimizer's responsibility. Run the benchmark with
the default `config.toml` for the target suite and save the JSON output. The
baseline file has the same schema as the benchmark output; the optimizer
extracts the `results` array.

Example command:

```bash
target/release/examples/benchmark \
    --config config.toml \
    --suite quick \
    --json \
    --timeout 3 \
    --runs 1 \
    --first-outcome \
    --output-file /path/to/optimizer/baseline_quick.json
```

The optimizer may store baselines wherever it chooses.

## Parameter constraints

Any config passed to the benchmark must pass `ScorerParams::validate()`. The
exact constraints are:

- All `score_*` fields (`score_winning_capture`, `score_promotion`,
  `score_capture`, ...), all `*_step` fields, all `and_*_scale` fields, and all
  `pieces.*` values must be non-negative integers.
- `and_pawn_storm_scale`, `and_rook_attack_scale`, and `and_approach_scale` must
  be in `[0, 100]`.
- `pieces.commoner` must be strictly greater than the sum of `pieces.pawn`,
  `pieces.knight`, `pieces.bishop`, `pieces.rook`, and `pieces.queen`.
- `score_winning_capture` must be strictly greater than the highest possible
  promotion score:
  `score_winning_capture > score_promotion + pieces.queen`.
- `score_promotion` must be strictly greater than the highest possible
  non-winning capture score. The highest non-winning capture removes every
  non-commoner enemy piece with a pawn and is valued as:
  `score_capture + capture_net_scale * ((pieces.queen + pieces.rook +
  pieces.bishop + pieces.knight) - pieces.pawn)`.
- No intermediate computation in the hierarchy checks may overflow `i32`.

These rules are enforced by `ScorerParams::validate()` in
`src/search/ordering/params.rs`.
