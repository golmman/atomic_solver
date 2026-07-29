# Implementation Plan: Phase 1 — `--outcome-only` flag and simple pre-exit log

## Goal

Introduce the pluggable `pre_exit_hook` machinery and the `--outcome-only`
CLI flag. The default hook logs a one-line summary when the search ends for any
reason (timeout, `q` + Enter, or completion). `--outcome-only` disables the
hook entirely, restoring the pre-feature CLI behavior.

## Changes

1. **`src/main.rs`**
   * Parse the new `--outcome-only` flag. Keep unknown-option handling strict
     as required by `AGENTS.md`.
   * Parse `--timeout` (seconds) with the same default used in the DFPN solver.
   * Track the stop `reason` through the search run.
   * Define an enum `ExitReason { Timeout, Quit, Complete }`.
   * Add a type `PreExitHook = Box<dyn FnOnce(ExitReason, Outcome, u64) + Send>`.
   * When `--outcome-only` is present, set the hook to `None` and do not spawn
     the input reader.
   * Otherwise install a default hook that prints:
     `pre_exit: reason=<Reason> outcome=<Outcome> nodes=<nodes>`.

2. **`src/search/dfpn/mod.rs` (or appropriate file in `src/search/dfpn/`)**
   * Make `search` / `search_with_settings` accept an optional `Arc<AtomicBool>`
     stop flag and a `&mut ExitReason` output.
   * Check the stop flag alongside `time_exceeded` so that `q` can stop the
     search.
   * Return the `nodes` count so the hook can log it.

3. **Input handling**
   * Spawn a line-reading thread in `main` when the hook is enabled.
   * On `q` + Enter, set `Arc<AtomicBool>` to true and store the reason.

4. **`src/outcome.rs` or wherever `Outcome` lives**
   * Ensure `Outcome` implements `Display` so the hook can log it.

## Test plan

* Run `cargo run -- --fen <FEN>` with a short `--timeout` and observe the
  `pre_exit` log line.
* Run the same command and press `q` + Enter; observe `reason=Quit`.
* Run with `--outcome-only` and confirm the `pre_exit` log is absent and no
  stdin reader is spawned.
* Run `cargo clippy`, `cargo fmt`, and `cargo test`.

## Final task

After implementation, create `docs/plans/storage/report1.md` summarizing the
additional tools/examples used, any problems encountered, open ends, and next
steps.
