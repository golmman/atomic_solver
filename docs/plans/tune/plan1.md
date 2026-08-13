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
  suites (`quick` and `thorough`), and documented `ScorerParams` validation
  constraints. Parameter mapping, projection, baselines, and loss computation
  are the optimizer's responsibilities.

## Scope

In scope:

- `--json` and `--output-file` flags for `examples/benchmark.rs`.
- `Suite::Quick` and `Suite::Thorough`.
- Integration tests for the JSON output.
- Documentation update pointing to the contract and clarifying optimizer
  responsibilities.
- Updated `docs/spec/optimizer_interface.md` with the `ScorerParams` validation
  constraints.

Out of scope for this plan:

- Any language-specific optimizer wrapper.
- Baseline files or baseline-generation scripts.
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

### 3. Baselines, parameter mapping, and loss

`atomic_solver` does not generate baselines, choose a parameter subset,
implement projection, or compute a loss. The optimizer does all of that. The
contract in `docs/spec/optimizer_interface.md` documents:

- how to generate a baseline by running the default config,
- the preferred `child_evals` metric,
- the `ScorerParams` validation constraints.

### 4. Tests

- Add an integration test under `tests/` that runs
  `target/release/examples/benchmark --config config.toml --suite quick --json
  --timeout 1 --runs 1 --output-file <temp>` and validates the JSON schema.

### 5. Documentation

Append a short "Tuning workflow" section to `AGENTS.md` that points to
`docs/spec/optimizer_interface.md` and makes clear that the optimizer owns
baselines, parameter mapping, projection, and loss.

## Implementation steps

1. Add `serde_json` to `[dev-dependencies]` in `Cargo.toml`. If a version
   conflict appears with the pinned `serde`, fall back to manual JSON string
   formatting for the small schema.
2. Define `JsonOutput`, `JsonResult`, and `JsonAggregate` structs in
   `examples/benchmark.rs` and add `--json` / `--output-file` parsing.
3. Add `Suite::Quick` and `Suite::Thorough` and the corresponding
   `quick_suite()` / `thorough_suite()` loaders.
4. Update `docs/spec/optimizer_interface.md` with the `ScorerParams` validation
   constraints.
5. Add the JSON integration test.
6. Update `AGENTS.md`.
7. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, and the
   manual validation commands.
8. Write `docs/plans/tune/report1.md`.

## Files changed

- `Cargo.toml`
- `examples/benchmark.rs`
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
```

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| `serde_json` conflicts with pinned `serde` | Use `[dev-dependencies]`; if conflict, fall back to manual JSON. |
| Quick suite too slow for the optimizer loop | Reduce timeout or subset; measure before committing. |
| Invalid configs crash the optimizer | The app returns a non-zero exit; the optimizer must assign a large loss. |

## Success criteria

1. `examples/benchmark --json` emits valid JSON matching the contract.
2. `quick` and `thorough` suites are selectable and finish in the expected
   time.
3. `cargo test` passes, including the new integration test.
4. `AGENTS.md` links to the contract and explains optimizer responsibilities.
5. `docs/plans/tune/report1.md` documents any deviations and measurements.

## Final task

Write `docs/plans/tune/report1.md` describing:

- which files changed and why,
- the exact JSON schema and suite definitions,
- any deviations from this plan,
- measured wall times for `quick` and `thorough` on the default config,
- unresolved parts and next steps.
