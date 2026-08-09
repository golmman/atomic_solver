# Plan 1: Move-order benchmark suite for the m20–m29 winning line

## Start

1. Read `AGENTS.md` to confirm project conventions and quality gates.
2. Read `docs/plans/move_order/ideas.md` for the move-ordering changes the
   benchmark is meant to test.
3. Read the current benchmark harness and shared helpers:
   - `examples/benchmark.rs`
   - `examples/common.rs`
   - `tests/common/mod.rs`
4. Read the existing corpus and regression fixtures:
   - `tests/fixtures/positions.txt`
   - `tests/test_corpus.rs`
   - `tests/test_plan6.rs`
   - `tests/stress.rs`

## Goal

Create a dedicated, reproducible benchmark suite around the provided 19-position
winning line (m20 to m29) so that the upcoming move-ordering changes can be
measured quickly, correctly, and without regressions.

Specifically:

- Make `examples/benchmark.rs` suite-aware, supporting the existing default
  suite and a new `move-order` suite.
- Store the 19 positions and their expected outcomes in a single fixture file
  (`tests/fixtures/move_order_positions.txt`) that is shared between the
  benchmark example and regression tests.
- Add per-position metrics that are sensitive to move-ordering quality: nodes,
  `child_evals`, PV length, and time to first decisive outcome vs. fully refined
  outcome.
- Add a release-only integration test (`tests/test_move_order.rs`) that asserts
  every position in the line solves to the expected side-to-move outcome within
  a documented timeout.
- Reduce duplication with `tests/test_plan6.rs` and `tests/stress.rs` by making
  the new fixture the canonical source for these positions.

## Background

The 19 positions form a known winning line for White, alternating side to move,
and ordered from hardest to easiest:

- White-to-move positions are expected `Outcome::Win`.
- Black-to-move positions are expected `Outcome::Loss` (Black is losing, which
  is still a win for White).

This line exercises the exact motifs targeted by the move-ordering ideas in
`docs/plans/move_order/ideas.md`: capture-blast valuation, direct commoner
threats, quiet approach moves, and forcing checks. A good move-ordering change
should disproportionately reduce `child_evals` and search time on the hardest
positions (m20–m22) while still solving the later positions correctly.

The current `examples/benchmark.rs` has a single hardcoded suite and prints a
fixed table. It cannot select a different suite, validate expected outcomes,
report first-outcome time, or compare against a baseline. `tests/test_plan6.rs`
and `tests/stress.rs` contain overlapping positions but are not integrated with
the benchmark harness, making before/after measurement tedious.

## Implementation tasks

### 1. Create a shared fixture for the move-order suite

Create `tests/fixtures/move_order_positions.txt` with the 19 positions in a
stable CSV-like format:

```text
# Format: name;fen;expected;note
# Expected is the side-to-move outcome: Win (White to move) or Loss (Black to move).
# Positions are listed from hardest to easiest, as provided for move-order testing.
m20_white;4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20;Win;hardest
m20_black;4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/1R4RK b - - 5 20;Loss;hardest
m21_white;4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21;Win;very hard
m21_black;4r2k/3p4/2pB2p1/p4p1p/6PP/2N1PP2/P1PP4/1R4RK b - - 0 21;Loss;very hard
m22_white;4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22;Win;hard
m22_black;4r2k/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK b - - 0 22;Loss;hard
m23_white;4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23;Win;medium
m23_black;4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K b - - 2 23;Loss;medium
m24_white;4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24;Win;medium
m24_black;4r1k1/3p4/2pB2p1/6Pp/p6P/2N2P2/P1PP4/1R2R2K b - - 0 24;Loss;medium
m25_white;4r1k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R2R2K w - - 0 25;Win;easy
m25_black;6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25;Loss;easy
m26_white;6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26;Win;easy
m26_black;1R4k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 1 26;Loss;easy
m27_white;1R6/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 2 27;Win;easier
m27_black;6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27;Loss;easier
m28_white;6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28;Win;very easy
m28_black;5R2/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 5 28;Loss;very easy
m29_white;5R2/3p4/3Bk1p1/6Pp/2p4P/p1N2P2/P1PP4/7K w - - 0 29;Win;easiest
```

Notes:

- Keep the names consistent (`m<ply>_<side>`) so filters and baselines are
  stable.
- `expected` must be parseable by the existing `Outcome::from_str` or an
  equivalent helper used in `tests/test_corpus.rs`.
- Add an inline comment at the top explaining that the line is won for White
  and that `Win`/`Loss` are side-to-move outcomes.

### 2. Make `examples/benchmark.rs` suite-aware

Refactor the example so it can run the existing suite, the new move-order suite,
or all positions:

1. Add an enum or string-based `--suite` argument. Valid values: `default`,
   `move-order`, `all`. Unknown values exit with an error.
2. Keep the current default suite (`SUITE`) as the default.
3. Load `move-order` positions from `tests/fixtures/move_order_positions.txt`
   using `env!("CARGO_MANIFEST_DIR")` to locate the file.
4. Add an optional `--first-outcome` flag that calls
   `search.set_first_outcome_only(true)`. This reports the time to first decisive
   outcome, which is one of the most direct measures of move-ordering quality.
5. Preserve `--runs`, `--timeout`, `--epsilon`, and the positional filter.
   The filter should match against the position `name`.
6. If a position has an `expected` outcome and the actual outcome differs, print
   a warning in the table and exit with a non-zero status after all cases have
   run. Do not panic mid-run.

The table output should remain easy to read. For `first-outcome` mode, replace
`pv_len` with `outcome` (PVs are not refined) and label the mode in the header.

### 3. Extend the `Case` / `BenchResult` data model

In `examples/benchmark.rs`:

- Add `expected: Option<Outcome>` and `note: Option<&'static str>` to `Case`.
- Add `first_outcome: Option<Duration>` to `BenchResult` when the benchmark is
  run with `--first-outcome`.
- Keep `outcome`, `nodes`, `child_evals`, `mean`, `min`, `max`, and `pv`
  unchanged.
- If `expected` is set, include a pass/fail marker in the printed row.

### 4. (Optional) Add baseline comparison

Add a simple baseline file format and CLI flags:

- `--save-baseline <FILE>` writes a CSV of `name;outcome;nodes;child_evals;mean;pv_len`.
- `--baseline <FILE>` reads a baseline and prints a diff with percentage changes
  in `nodes`, `child_evals`, and `mean` time.

Store a committed baseline in `tests/fixtures/move_order_baseline.csv`. Update
it manually after verified move-order improvements. This is optional for the
first pass; if it is skipped, document it in the final report as a follow-up.

### 5. Add regression tests for the move-order suite

Create `tests/test_move_order.rs`:

1. Use `include_str!("fixtures/move_order_positions.txt")` to load the suite.
2. For each position, run `Search` via the helpers in `tests/common/mod.rs`
   (e.g. `solve_with_timeout`) with a 60-second timeout.
3. Assert the returned `Outcome` matches the `expected` column.
4. Mark the whole test with
   `#[cfg_attr(debug_assertions, ignore = "slow move-order benchmark")]` so it
   runs in release CI but not in normal debug builds.
5. If the hardest positions (m20–m22) currently time out in release, split the
   test into two files:
   - `tests/test_move_order_fast.rs` — m25–m29, expected to pass in release.
   - `tests/test_move_order_hard.rs` — m20–m24, release-only and possibly
     `#[ignore]` until the move-ordering changes in `ideas.md` land.

Avoid duplicating assertions already in `tests/test_plan6.rs`; if a position is
moved to the fixture, remove it from `test_plan6.rs` unless `test_plan6.rs` has
additional assertions (e.g. a specific first move or PV length) that should be
kept.

### 6. Audit `tests/stress.rs` and `tests/test_plan6.rs`

- `tests/stress.rs` currently treats m20/m21 positions as "unproven in 60s".
  Once the benchmark fixture is the canonical source, update `stress.rs` to
  load from the fixture or remove the now-redundant entries. The stress tests
  should guard only positions that are genuinely unsolved in the CI timeout.
- `tests/test_plan6.rs` contains several of the same m23–m29 positions. Decide
  per position whether the assertion belongs in `test_plan6.rs` (fast regression)
  or in `test_move_order.rs` (benchmark-backed). Document the split in the test
  file comments.

### 7. Update examples and documentation

- Update the `examples/benchmark.rs` doc comment to show `--suite move-order`
  and `--first-outcome` usage.
- Update `examples/move_order_debug.rs` so it can take a `--suite` or `--name`
  argument to inspect ordering for any benchmark position.
- Update `README.md` and `AGENTS.md` to mention the new `move-order` benchmark
  suite and the `--suite` / `--first-outcome` options.

### 8. Validate every FEN in the fixture

Before committing the fixture, run a small one-off check (or add a unit test)
that every FEN parses and that the legal-move count is non-zero for
non-terminal positions. This catches typos like the `p6Pp` → `p5Pp` issue that
`test_plan6.rs` had.

## File changes

- `tests/fixtures/move_order_positions.txt` (new)
- `tests/test_move_order.rs` (new; may be split into fast/hard variants)
- `examples/benchmark.rs`
- `examples/common.rs` (if shared parsing helpers or the default suite are moved
  here)
- `tests/test_plan6.rs` (remove duplicate positions, if any)
- `tests/stress.rs` (update or remove now-redundant entries)
- `README.md`
- `AGENTS.md`
- `tests/fixtures/move_order_baseline.csv` (new, optional)

## Risks

- **CI runtime.** A 19-position suite with a 60-second timeout is slow. Mitigate
  by splitting fast/hard subsets and using `cfg_attr(debug_assertions, ignore)`.
- **Expected outcomes currently fail.** The hardest positions may still return
  `Draw` on timeout. Do not assert they pass until move-ordering improvements
  land; use ignored or stress tests and document the status.
- **Fixture duplication with examples.** Both the benchmark example and the test
  want to read the same fixture. Use `env!("CARGO_MANIFEST_DIR")` in the example
  and `include_str!` in the test so they stay in sync.
- **Baseline staleness.** If a baseline is added, it must be updated manually
  with each verified move-order change; otherwise diffs become noise.
- **File-size limits.** `examples/benchmark.rs` may grow beyond 10 KB once suite
  loading and baseline logic are added. If it exceeds the limit, add a file-size
  justification in the file header or split parsing into a small `benchmark/`
  sub-module under `examples/`.

## Verification

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo run --release --example benchmark -- --timeout 10 --runs 1
cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 1
cargo run --release --example benchmark -- --suite move-order --timeout 10 --first-outcome --runs 1
cargo test --release --test test_move_order
cargo run --release --example move_order_debug -- --suite move-order --name m25_white
```

Additional checks:

- The default benchmark suite still runs unchanged.
- The move-order fixture parses and every FEN is valid.
- `examples/benchmark.rs` exits with an error for unknown `--suite` values.
- Outcome mismatches in the benchmark table are printed clearly and cause a
  non-zero exit code.

## Final task

Write `docs/plans/move_order/report1.md` documenting:

- The actual changes made to the benchmark harness and fixture.
- Which positions are fast vs. hard and which tests are ignored or release-only.
- Baseline numbers (if collected) and any unexpected results.
- Any duplication removed from `test_plan6.rs` or `stress.rs`.
- Follow-up items, such as enabling ignored tests after the move-ordering
  changes land or adding automated baseline comparison.
