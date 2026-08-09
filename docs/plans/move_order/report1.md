# Report 1: Move-order benchmark suite for the m20–m29 winning line

This report documents the implementation of `docs/plans/move_order/plan1.md`:
adding a dedicated, reproducible benchmark suite for the 19-position m20–m29
winning line and wiring it into the existing benchmark harness and regression
tests.

## Summary

- Added `tests/fixtures/move_order_positions.txt` with the 19 positions, names,
  side-to-move expected outcomes (`Win` for White-to-move, `Loss` for
  Black-to-move), and difficulty notes ordered from hardest to easiest.
- Extended `examples/benchmark.rs` with `--suite default|move-order|all`,
  `--first-outcome`, expected-outcome validation, and a `status` column in the
  output table (`ok` / `timeout` / `wrong`).
- Added `examples/common.rs` helper to parse the fixture, so `examples/benchmark.rs`,
  `examples/move_order_debug.rs`, and `examples/static_move_scores.rs` can all
  look up positions by name.
- Added `tests/test_move_order.rs` with a release-only regression test that
  runs the entire suite with a 5-second timeout and asserts the solver never
  returns a wrong decisive outcome (`Draw` on timeout is allowed).
- Refactored `tests/stress.rs` to load the hardest positions from the fixture.
  During verification `m22` was found to be solvable in release within 60
  seconds, so the hard-positions stress test now asserts that only `m20` and
  `m21` remain unproven in 60 seconds.
- Updated `README.md` and `AGENTS.md` to document the new `--suite`,
  `--first-outcome`, and `--name` options.

## Files changed

- `tests/fixtures/move_order_positions.txt` (new)
- `tests/test_move_order.rs` (new)
- `examples/benchmark.rs`
- `examples/common.rs`
- `examples/move_order_debug.rs`
- `examples/static_move_scores.rs`
- `tests/common/mod.rs`
- `tests/stress.rs`
- `README.md`
- `AGENTS.md`

## The 19-position fixture

The fixture is in `tests/fixtures/move_order_positions.txt`. Positions alternate
White-to-move (`Win`) and Black-to-move (`Loss`) through the endgame line, from
`m20` (hardest) to `m29` (easiest).

Example entries:

```text
m20_white;4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20;Win;hardest
m20_black;4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/1R4RK b - - 5 20;Loss;hardest
...
m29_white;5R2/3p4/3Bk1p1/6Pp/2p4P/p1N2P2/P1PP4/7K w - - 0 29;Win;easiest
```

The full list is in the fixture. Every FEN parses and has legal moves, as
checked by `tests/test_move_order.rs::move_order_fixture_fens_are_valid`.

## Benchmark harness changes

`examples/benchmark.rs` now supports:

```bash
cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 1
cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5 --runs 1
```

The printed table has new `status`, `expected`, and `note` columns when an
expected outcome is available. `status` is:

- `ok` — outcome matches expected.
- `timeout` — outcome is `Draw` (the solver timed out before proving the expected
  result).
- `wrong` — the solver returned a decisive outcome that does not match expected.
  This makes the benchmark exit with a non-zero status.

The default suite is unchanged, and `--suite all` concatenates the default and
move-order suites.

## Diagnostic examples

`examples/move_order_debug.rs` and `examples/static_move_scores.rs` now accept a
`--name <case>` argument to inspect any move-order benchmark position by name:

```bash
cargo run --release --example move_order_debug -- --name m25_white
cargo run --release --example static_move_scores -- --name m25_white
```

## Regression test

`tests/test_move_order.rs` contains two tests:

1. `move_order_fixture_fens_are_valid` (runs in debug) — every fixture FEN
   parses and non-terminal positions have legal moves.
2. `move_order_suite_no_misclassification` (release-only) — runs the full
   suite with a 5-second timeout and asserts the solver never returns a wrong
   decisive outcome. `Draw` on timeout is allowed because the hardest positions
   are not expected to solve in 5 seconds yet.

The release run passed in ~49 seconds, proving the suite does not produce false
wins or losses.

## Stress test adjustment

`tests/stress.rs` was updated to load the hardest move-order positions from the
fixture. The original hard-positions assumption was `m20`–`m22`, but
verification showed that `m22_white` (`4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`)
is solved in release within 60 seconds. The stress test now asserts that only
`m20` and `m21` remain unproven in 60 seconds; when a move-order improvement
makes one of them decisive, the test will fail and the position should be moved
to the regression suite.

## Verification results

```bash
cargo fmt                    # clean
cargo clippy --all-targets   # clean
cargo test --all-targets     # all active tests pass
cargo doc --no-deps          # clean
cargo test --release --test test_move_order    # passed in ~49 s
cargo test --release --test stress m19         # passed in 60 s
cargo test --release --test stress move_order_hard -- --test-threads=2  # passed in 240 s
```

Manual benchmark sanity checks:

```bash
cargo run --example benchmark -- --suite move-order --timeout 1 --runs 1
cargo run --example benchmark -- --suite move-order --first-outcome --timeout 1 --runs 1 --filter m29
cargo run --example move_order_debug -- --name m25_white
cargo run --example static_move_scores -- --name m25_white
```

## What was intentionally left out / follow-up

- **Baseline comparison:** The plan proposed `--save-baseline` / `--baseline`
  CSV flags for before/after move-order comparison. This was not implemented in
  this pass to keep `examples/benchmark.rs` under the 10 KiB soft limit (it is
  currently 9.9 KiB) and to avoid fragile committed baselines. A future pass
  can add baseline support once the suite is stable.
- **`test_plan6.rs` overlap:** Some `m24`–`m29` positions overlap with
  `tests/test_plan6.rs`. They were not removed because `test_plan6.rs` often
  asserts specific first moves, PV lengths, or CLI behavior that the new
  benchmark-backed test does not cover.
- **Harder positions:** `m20` and `m21` still timeout with the current move
  ordering. They are the primary targets for the changes in
  `docs/plans/move_order/ideas.md`.

## Next steps

1. Use `--suite move-order --first-outcome` to evaluate each move-order idea from
   `docs/plans/move_order/ideas.md`.
2. When an idea consistently reduces `nodes` / `child_evals` on `m24`–`m29`
   without misclassifying `m20`–`m21`, add a regression test or update the
   expected outcomes in `tests/test_move_order.rs`.
3. Once `m20`/`m21` become solvable, move them out of `tests/stress.rs` and into
   the active regression suite.
4. Consider adding baseline comparison to `examples/benchmark.rs` once the suite
   stabilizes.
