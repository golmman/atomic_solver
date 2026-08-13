# Plan 1: Implement the optimizer interface contract

## Goal

Make `atomic_solver` comply with the CLI contract in
`docs/spec/optimizer_interface.md`. After this plan is implemented, an external
optimizer must be able to evaluate a candidate `ScorerParams` by invoking
`target/release/examples/benchmark --config <TOML> --suite <SUITE> --json` and
parsing the JSON output.

## Background

- `examples/benchmark.rs` already runs suites, loads `--config` files, and
  collects `nodes`, `child_evals`, `time`, and PV information.
- `ScorerParams` supports partial TOML via `#[serde(default)]`, so an optimizer
  can write only the parameters it wants to vary.
- The contract requires JSON output from the benchmark, two optimizer-facing
  suites (`quick` and `thorough`), committed baseline files, and a documented
  parameter/loss contract. The optimizer itself implements parameter mapping,
  projection, and loss computation.

## Scope

In scope:

- `--json` and `--output-file` flags for `examples/benchmark.rs`.
- `Suite::Quick` and `Suite::Thorough`.
- A `scripts/generate_baselines.sh` convenience script that uses the CLI to
  produce `tune/baselines_quick.json` and `tune/baselines_thorough.json`.
- Committing baseline files for the default `config.toml`.
- Integration tests for the JSON output.
- Documentation update linking the contract to the workflow.
- A recommended first-pass tunable subset, projection rules, and example loss
  formula, documented in `docs/spec/optimizer_interface.md`.

Out of scope for this plan:

- Any language-specific optimizer wrapper.
- `tune_server` stdin/stdout mode.
- Tuning history/killer parameters.
- True gradients or differentiable surrogates.

## Design

### 1. JSON output

Add `Serialize` derive structs to `examples/benchmark.rs`:

```rust
#[derive(Serialize)]
struct JsonOutput { ... }

#[derive(Serialize)]
struct JsonResult { ... }

#[derive(Serialize)]
struct JsonAggregate { ... }
```

- `--json` prints `serde_json::to_string_pretty(&output)` to `stdout`.
- `--output-file <PATH>` writes the same JSON to a file.
- Without `--json`, the existing markdown table is printed unchanged.

The schema must match `docs/spec/optimizer_interface.md`:

```json
{
  "suite": "quick",
  "mode": "first-outcome",
  "timeout": 3,
  "runs": 1,
  "epsilon": 0.125,
  "tt_size": 64,
  "config_path": "/tmp/...toml",
  "results": [ ... ],
  "aggregates": { ... }
}
```

### 2. Quick and thorough suites

Add two new `Suite` variants to `examples/benchmark.rs`:

- `Suite::Quick` — `decisive_positions.txt` (10 cases) plus the move-order
  positions `m23_white`/`m23_black` through `m29_white` (13 cases). Use
  `--first-outcome --timeout 3 --runs 1`. Expected wall time is roughly
  10–15 seconds on the reference machine.
- `Suite::Thorough` — all move-order cases (19 cases) plus all decisive cases
  (10 cases). Use `--first-outcome --timeout 5 --runs 3`.

The exact case lists come from the existing fixtures in
`tests/fixtures/decisive_positions.txt` and
`tests/fixtures/move_order_positions.txt`, loaded through the same parser
already used by `examples/common.rs`.

### 3. Baseline generation

`scripts/generate_baselines.sh` builds the release benchmark and runs:

```bash
target/release/examples/benchmark \
    --config config.toml \
    --suite quick \
    --json \
    --timeout 3 \
    --runs 1 \
    --first-outcome \
    --output-file tune/baselines_quick.json

target/release/examples/benchmark \
    --config config.toml \
    --suite thorough \
    --json \
    --timeout 5 \
    --runs 3 \
    --first-outcome \
    --output-file tune/baselines_thorough.json
```

The optimizer reads the baseline file for the suite it is running and extracts
the `results` array. A baseline file is the same JSON output as a normal
benchmark run, but produced from the default `config.toml`.

### 4. Recommended first-pass tunable subset

Document this subset in `docs/spec/optimizer_interface.md` as guidance for
optimizer implementers. The parameters below are a starting point; the optimizer
may choose a different subset as long as the resulting TOML passes
`ScorerParams::validate()`.

| Parameter | Default | Lower | Upper | Notes |
|---|---|---|---|---|
| `score_capture` | 5000 | 1000 | 50000 | Keep above `score_kamikaze` if both are tuned. |
| `capture_net_scale` | 10 | 1 | 100 | |
| `score_threat` | 1000 | 0 | 5000 | Keep below `score_kamikaze`. |
| `score_kamikaze` | 3000 | 0 | 10000 | Keep below `score_capture`. |
| `score_approach` | 100 | 0 | 50000 | |
| `score_approach_step` | 10 | 0 | 10000 | |
| `score_center` | 50 | 0 | 50000 | |
| `score_center_step` | 10 | 0 | 10000 | |
| `score_pawn_storm` | 5500 | 0 | 500000 | |
| `score_pawn_storm_step` | 100 | 0 | 100000 | |
| `score_rook_open_file` | 2000 | 0 | 500000 | |
| `score_rook_open_file_step` | 50 | 0 | 100000 | |
| `score_rook_back_rank` | 300 | 0 | 500000 | |
| `and_pawn_storm_scale` | 50 | 0 | 100 | Percent multiplier. |
| `and_rook_attack_scale` | 50 | 0 | 100 | Percent multiplier. |
| `and_approach_scale` | 75 | 0 | 100 | Percent multiplier. |

Fixed for the first pass:

- `score_winning_capture`
- `score_promotion`
- `score_threat_last`
- `score_kamikaze_last`
- `score_rook_center`
- all `pieces` values

The exact hierarchy and overflow constraints are enforced by
`ScorerParams::validate()`.

### 5. Example loss formula

Document an example loss formula in `docs/spec/optimizer_interface.md`:

```
loss = 0
for r in result["results"]:
    base = baselines[suite][r["name"]]
    if r["wrong"]:
        loss += WRONG_PENALTY
    elif r["timeout"] and not base["timeout"]:
        loss += TIMEOUT_PENALTY + log(r["child_evals"] / base["child_evals"])
    else:
        loss += log(r["child_evals"] / base["child_evals"])
```

Default weights for an example implementation:

- `WRONG_PENALTY = 100.0`
- `TIMEOUT_PENALTY = 10.0`

The optimizer may use a different loss. `atomic_solver` only provides the raw
metrics.

### 6. Tests

- Add an integration test under `tests/` that runs
  `target/release/examples/benchmark --config config.toml --suite quick --json
  --timeout 1 --runs 1 --output-file <temp>` and validates the JSON schema.
- Optionally add a small shell test that runs `scripts/generate_baselines.sh`
  and checks the generated baseline files.

### 7. Documentation

Append a "Tuning workflow" section to `AGENTS.md` that points to:

- `docs/spec/optimizer_interface.md`
- `tune/baselines_quick.json`
- `tune/baselines_thorough.json`
- `scripts/generate_baselines.sh`

## Implementation steps

1. Add `serde_json` to `[dev-dependencies]` in `Cargo.toml`. If a version
   conflict appears with the pinned `serde`, fall back to manual JSON string
   formatting for the small schema.
2. Define `JsonOutput`, `JsonResult`, and `JsonAggregate` structs in
   `examples/benchmark.rs` and add `--json` / `--output-file` parsing.
3. Add `Suite::Quick` and `Suite::Thorough` and the corresponding
   `quick_suite()` / `thorough_suite()` loaders.
4. Update `docs/spec/optimizer_interface.md` with the parameter constraints and
   recommended subset appendix.
5. Create `scripts/generate_baselines.sh` and produce
   `tune/baselines_quick.json` and `tune/baselines_thorough.json`.
6. Add the JSON integration test.
7. Update `AGENTS.md`.
8. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, and the
   manual validation commands.
9. Write `docs/plans/tune/report1.md`.

## Files changed

- `Cargo.toml`
- `examples/benchmark.rs`
- `scripts/generate_baselines.sh` (new)
- `tune/baselines_quick.json` (new)
- `tune/baselines_thorough.json` (new)
- `tests/test_benchmark_json.rs` (new)
- `docs/spec/optimizer_interface.md`
- `AGENTS.md`
- `docs/plans/tune/report1.md` (new, final report)

## Verification

```bash
cargo fmt
cargo clippy --all-targets
cargo test

# Manual JSON check.
target/release/examples/benchmark \
    --config config.toml \
    --suite quick \
    --json \
    --timeout 3 \
    --runs 1 \
    --first-outcome \
    --output-file /tmp/quick_default.json

# Verify the output is valid JSON and contains the expected fields.

# Generate baselines.
./scripts/generate_baselines.sh
```

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| `serde_json` conflicts with pinned `serde` | Use `[dev-dependencies]`; if conflict, fall back to manual JSON. |
| Quick suite too slow for the optimizer loop | Reduce timeout or subset; measure before committing. |
| Loss dominated by timeouts | The optimizer, not the app, owns the loss weights. Document defaults. |
| Overfitting to move-order positions | Use `thorough` for validation and consider a held-out test set later. |
| Invalid configs crash the optimizer | The app returns a non-zero exit; the optimizer must assign a large loss. |

## Success criteria

1. `examples/benchmark --json` emits valid JSON matching the contract.
2. `quick` and `thorough` suites are selectable and finish in the expected
   time.
3. `scripts/generate_baselines.sh` produces
   `tune/baselines_quick.json` and `tune/baselines_thorough.json`.
4. `cargo test` passes, including the new integration test.
5. `AGENTS.md` links to the contract and baseline files.
6. `docs/plans/tune/report1.md` documents any deviations and measurements.

## Final task

Write `docs/plans/tune/report1.md` describing:

- which files changed and why,
- the exact JSON schema and suite definitions,
- any deviations from this plan,
- measured wall times for `quick` and `thorough` baselines,
- sample loss values for the default and a perturbed parameter set,
- unresolved parts and next steps (server mode, more parameters, etc.).
