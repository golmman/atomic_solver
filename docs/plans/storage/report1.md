# Implementation Report: Storage Phase 1

## Summary

Implemented the pluggable `pre_exit_hook` machinery, the `--outcome-only` CLI
flag, and the `q` + Enter stop signal described in
`docs/plans/storage/plan1.md`.

## Changes made

- `src/search/dfpn/mod.rs`
  - Added `ExitReason { Timeout, Quit, Complete }` with `Display`.
  - `Search` now carries an optional `Arc<AtomicBool>` stop flag.
  - Added `Search::set_stop_flag`, `Search::exit_reason`, and
    `Search::search_with_settings(stop_flag, exit_reason)`.
  - `time_exceeded` checks the stop flag alongside the deadline.
- `src/position.rs`
  - `Outcome` now implements `Display` (lower-case `win`/`loss`/`draw`).
- `src/main.rs`
  - Parses `--outcome-only`.
  - Spawns a stdin reader thread that sets the stop flag on `q` + Enter.
  - Installs a default `PreExitHook` that prints
    `pre_exit: reason=<Reason> outcome=<Outcome> nodes=<nodes>`.
  - `--outcome-only` disables the hook and the stdin reader.
- `AGENTS.md`
  - Updated the `src/main.rs` option list to include `--outcome-only`,
    `--timeout`, and `--tt-size`.
- `tests/test_plan6.rs`
  - Updated `m27_ppv_only` to expect the new `pre_exit` summary line so that
    it fails on the pre-existing PPV content assertion instead of the line
    count.

## Verification

- `cargo fmt --check` passed.
- `cargo clippy --all-targets` passed with zero warnings.
- `cargo doc --no-deps` built cleanly.
- Manual CLI checks:
  - Default run prints `pre_exit: reason=Complete ...`.
  - Short `--timeout` prints `pre_exit: reason=Timeout ...`.
  - `q` + Enter prints `quit` and `pre_exit: reason=Quit ...`.
  - `--outcome-only` suppresses the `pre_exit` line and the stdin reader.
- `cargo test --release`
  - 10 passed / 2 failed / 17 ignored in `test_plan6`.
  - The two failing tests (`m27_ppv_only`, `m27_streaming_output`) are
    pre-existing PPV-extraction issues documented in
    `docs/plans/ultimattt/report5.md` (the solver returns an 11-plies PPV
    instead of the expected 7-plies line). They are unrelated to the Phase 1
    pre-exit hook.
- `cargo test` (debug)
  - Same two pre-existing failures as release, plus `m27_shortest_pv` timing
    out in debug (release passes). This is consistent with the debug/release
    performance gap and is not caused by the Phase 1 changes.

## Problems encountered

- `Search::time_exceeded` was initially made `&mut self` so it could update an
  internal `exit_reason` field. This added overhead and borrow complexity.
  Replaced the stored `exit_reason` with an on-demand `exit_reason()` method
  and kept `time_exceeded` `&self`.
- `tests/test_plan6.rs::m27_ppv_only` required a line-count update because the
  default pre-exit hook adds a third output line. The test's PPV content
  assertion (`b1b8 g8f7`) remains a pre-existing failure.

## Open ends / next steps

- Phase 2: implement the `ProofTree -> .sql` serializer independently of the
  search and have the pre-exit hook write a small test dump to `--dump-path`.
- The pre-existing `find_ppv` PPV-length issue exposed by `m27_ppv_only` and
  `m27_streaming_output` may need to be addressed before the real proof-tree
  dump can reliably extract a correct PPV.
- Phase 3 will add the worker thread, `mpsc` event queue, and `--pt-size`
  memory cap.
