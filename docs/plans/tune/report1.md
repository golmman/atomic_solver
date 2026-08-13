# Report: Plan 1 — Implement the optimizer interface contract

## Summary

`atomic_solver` now complies with the optimizer interface contract in
`docs/spec/optimizer_interface.md`. An external optimizer can evaluate a
candidate `ScorerParams` by invoking `target/release/examples/benchmark` with
`--config <TOML>`, `--suite quick|thorough`, and `--json`.

## Files changed

- `Cargo.toml` — added `serde_json` to `[dev-dependencies]`.
- `src/position.rs` — derived `serde::Serialize` for `Outcome` with
  `#[serde(rename_all = "lowercase")]` so it serializes as `"win"`,
  `"loss"`, or `"draw"`.
- `examples/benchmark.rs` — added `--json`, `--output-file`, `--tt-size`,
  `Suite::Quick`, `Suite::Thorough`, JSON schema structs, and the
  `quick_suite()` / `thorough_suite()` loaders.
- `tests/test_benchmark_json.rs` (new) — integration test that runs the
  `benchmark` example with `--suite quick --json` and validates the schema.
- `docs/spec/optimizer_interface.md` — tightened the parameter-constraints
  section to match `ScorerParams::validate()` exactly.
- `AGENTS.md` — updated the `benchmark` example description and added a
  "Tuning workflow" section.

## JSON schema

With `--json`, `examples/benchmark` prints a single JSON object to `stdout` and
writes the same object to `--output-file <PATH>` if supplied.

```json
{
  "suite": "quick",
  "mode": "first-outcome",
  "timeout": 3,
  "runs": 1,
  "epsilon": 0.125,
  "tt_size": 64,
  "config_path": "config.toml",
  "results": [
    {
      "name": "dec01",
      "status": "ok",
      "outcome": "win",
      "expected": "win",
      "nodes": 284480,
      "child_evals": 5713086,
      "time_mean": 1.491958182,
      "time_min": 1.491958182,
      "time_max": 1.491958182,
      "pv_len": 27,
      "timeout": false,
      "wrong": false
    }
  ],
  "aggregates": {
    "total_nodes": 1580999,
    "total_child_evals": 23828952,
    "total_time": 6.094840330999999,
    "solved": 23,
    "timeouts": 0,
    "wrong": 0,
    "mean_pv_len": 13.956521739130435
  }
}
```

- `status` is one of `"ok"`, `"timeout"`, `"wrong"`, or `"unknown"`.
- `expected` is `null` when the benchmark case has no recorded expected outcome.
- `time_*` values are in seconds.
- `timeout` and `wrong` are booleans derived from `status`.

## Suite definitions

The two optimizer-facing suites are defined in `examples/benchmark.rs` using the
fixtures in `tests/fixtures/`.

- `quick` — the ten `dec01`..`dec10` positions plus the move-order cases
  `m23_white`/`m23_black` through `m29_white` (13 positions, 23 total).
- `thorough` — all 19 move-order cases (`m20_white`/`m20_black` through
  `m29_white`) plus the ten decisive cases (29 total).

Both suites are expected to be invoked with `--first-outcome` for tuning work,
so the timing reflects the first decisive line rather than the refined PV.

## Verification

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo doc
```

All passed.

The integration test `tests/test_benchmark_json.rs` runs
`target/release/examples/benchmark --config config.toml --suite quick --json
--first-outcome --timeout 1 --runs 1 --output-file <temp>` and validates the
JSON schema and the output-file round-trip.

## Measured wall times on the default config

The reference machine is the Linux/aarch64 container used for this session.

- `quick` (`--first-outcome --timeout 3 --runs 1`):
  - Wall time: **~12.3 s**
  - 23/23 cases solved, 0 wrong, 0 timeouts.
  - Aggregate `total_child_evals`: 23,828,952.

- `thorough` (`--first-outcome --timeout 5 --runs 3`):
  - Wall time: **~2 m 25 s**
  - 23/29 cases solved, 6 timeouts (`m20_white`, `m20_black`, `m21_white`,
    `m21_black`, `m22_white`, `m22_black`), 0 wrong.
  - Aggregate `total_child_evals`: 132,799,953.

The `quick` suite comfortably meets the 10–15 s target. The `thorough` suite
runs longer because the three hard `m20`–`m22` pairs hit the 5 s budget on all
three runs; this is still within the "several minutes" expectation and gives the
optimizer a meaningful validation set with a mix of solved and timed-out cases.

## Deviations from the plan

- The plan listed only `--json` and `--output-file` as new flags for
  `examples/benchmark.rs`; the schema already specified `tt_size`, so the
  existing `--tt-size` parameter was also exposed and passed through to
  `Search::new`. This makes the JSON output self-consistent with the search
  configuration.
- The integration test uses `--first-outcome` to keep runtime short; the plan's
  example command did not explicitly include it, but the suite definition
  assumes first-outcome mode.
- The plan suggested adding `serde_json` to `[dev-dependencies]` and falling back
  to manual JSON only if a conflict appeared. No conflict appeared, so the
  `serde_json` dependency is used directly.

## Unresolved parts and next steps

- No language-specific optimizer wrapper, baseline-generation script, or tuning
  history/killer-parameter support was added; those remain the optimizer's
  responsibility per the contract.
- The JSON schema is validated in the integration test by field presence and
  type only. A stricter JSON-Schema or serde-typed check could be added later
  if the optimizer needs it.
- No separate baseline file is shipped; the optimizer must generate its own by
  running the default `config.toml` on the target suite.
