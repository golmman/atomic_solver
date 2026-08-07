# Report: Plan 2 — Housekeeping cleanup

This report documents the cleanups applied from `docs/plans/cleanup/plan2.md`,
items intentionally skipped or deferred, and verification results.

## Applied cleanups

### DRY `Outcome` formatting and parsing

- `src/position.rs`
  - Added `Outcome::as_str(&self) -> &'static str` returning `"win"`, `"loss"`,
    or `"draw"`.
  - Implemented `std::str::FromStr for Outcome` (case-insensitive, error type
    `String`).
  - Made the existing `Display` impl for `Outcome` delegate to `as_str()`.
  - Added `#[must_use]` to `Outcome::as_str`.

- `src/main.rs`, `examples/benchmark.rs`, `examples/chunk_growth.rs`,
  `examples/verify_ppv.rs`
  - Removed the duplicated `outcome_str` helpers and replaced all call sites
    with `outcome.as_str()`.

- `tests/test_cli.rs`, `tests/test_corpus.rs`
  - Replaced the hand-rolled parsers with `s.parse::<Outcome>().ok()` and
    `token.parse::<Outcome>().ok()`.

### DRY f64-to-fraction conversion

- `src/search/dfpn/mod.rs`
  - Renamed the private `epsilon_fraction(v: f64)` helper to
    `fraction_from_f64`.
  - Added `Search::set_chunk_multiplier_from_factor(factor: f64) -> (u64, u64)`
    which asserts `factor >= 1.0`, converts the factor, and returns the
    reduced `(num, den)`.
  - Updated `set_epsilon` to call `fraction_from_f64(1.0 + epsilon)`.
  - Removed `Search::set_chunk_multiplier(num, den)` once it had no callers.

- `examples/chunk_growth.rs`
  - Removed the duplicated `factor_fraction` and `gcd` functions.
  - Renamed `Result` to `ChunkResult` to avoid shadowing `std::result::Result`.
  - `run_once` and `bench_mode` now take `factor: Option<f64>` and call
    `search.set_chunk_multiplier_from_factor(factor)` for geometric growth.

### DRY example helpers

- `examples/common.rs`
  - Removed `parse_uci`; it was identical to `notation::uci_to_move`.
  - Rewrote `parse_move` to build a candidate UCI move and delegate to
    `notation::uci_to_move`, removing the manual `MoveList` scan and
    `MoveType`/`PieceType` imports.

- `examples/verify_ppv.rs`
  - Replaced `common::parse_uci` with `atomic_solver::notation::uci_to_move`.

- `examples/twin_stats.rs`
  - Replaced `pv.iter().map(|m| m.to_uci())` with
    `pv.iter().copied().map(move_to_uci)`.

### YAGNI and visibility cleanup

- `src/search/dfpn/mod.rs`
  - Removed `Search::set_log_writer`, the `log_writer` field, and the
    `std::io::Write` import. `log_chunk` now writes directly to stderr.
  - Removed `Search::search_with_settings`.

- `src/position.rs`
  - The `board` field was already private; external call sites already used the
    `board()` accessor.
  - Restricted `try_do_move` to `#[cfg(test)] pub(crate)` and removed the
    stale `#[allow(dead_code)]` guard.

### Structural DRY in search and position

- `src/search/dfpn/mod.rs`
  - Refactored `begin_run` to call `reset_search_state`, then set only the
    timing and counter fields.

- `src/search/dfpn/core.rs`
  - Added a private `Search::with_child_path(mv, f)` helper that updates
    `proof_path` and `move_stack` only when a proof-tree sender is configured
    and then executes `f(&mut self)`. The recursive `dfpn` call is now written
    once instead of duplicated across `emit`/`!emit` branches.

- `src/position.rs`
  - Extracted a private `fn refresh_zobrist(&mut self)` and called it from both
    `do_move` and `undo_move`.
  - Made `legal_moves_with_state` call `self.populate_state(state)` instead of
    duplicating `self.board.populate_state(state)`.

### Constants and literal duplication

- Documented intentional cross-boundary duplication:
  - `examples/common.rs` and `tests/common/mod.rs` now note that `M19_FEN` is
    duplicated because examples and integration tests cannot share modules.
  - `src/cli.rs` now notes that its `STARTPOS_FEN` mirrors
    `Position::STARTPOS_FEN` so the parser stays `std`-only.

- Replaced remaining hardcoded start-FEN literals with
  `Position::STARTPOS_FEN` where the file already had access to `Position`:
  - `src/notation.rs` unit tests.
  - `src/zobrist.rs` unit tests.
  - `src/search/dfpn/history.rs` `start_position` test helper.
  - `src/proof_tree/mod.rs` `to_bin_round_trips_small_tree` test.
  - `src/position.rs` `try_do_move` test.
  - `src/main.rs` module doc comment now references `Position::STARTPOS_FEN`
    instead of repeating the full FEN string.

- Fixed remaining `unreadable_literal` warnings:
  - `src/search/dfpn/mod.rs` mantissa mask is now `0x000f_ffff_ffff_ffff`.
  - LCG constants in `src/zobrist.rs` and `tests/test_position.rs` are now
    `6_364_136_223_846_793_005` and `1_442_695_040_888_963_407`.

### Module-size documentation and organization

- Added module-level size-justification headers to files over 10 KiB that lacked
  one:
  - `src/search/dfpn/children.rs`
  - `src/search/dfpn/core.rs`
  - `src/search/dfpn/history.rs`
  - `src/search/dfpn/selection.rs` (also notes it is close to the 20 KiB soft
    limit)
  - `src/position.rs`
  - `src/cli.rs`
  - `src/main.rs`
  - `src/proof_tree/binary.rs`
  - `src/search/ordering.rs`
  - `src/proof_tree/worker/tests.rs`

- `src/proof_tree/mod.rs` was the only file over the 20 KiB soft limit. It was
  split into:
  - `src/proof_tree/mod.rs` — `ProofNode`, `ProofTree`, `NodeProven`, and the
    public data-model API (`to_bin`/`from_bin`, `extract_ppv`,
    `validate_ppv`).
  - `src/proof_tree/worker.rs` — `ProofTreeWorker`, `ProofMessage`,
    `ProofResponse`, `ProofStats`, and the worker logic.
  - `src/proof_tree/worker/tests.rs` — worker-specific unit tests, keeping
    `worker.rs` under the 20 KiB soft limit.
  - `mod.rs` re-exports `ProofTreeWorker`, `ProofMessage`, `ProofResponse`,
    and `ProofStats` so the public API is unchanged.

- `src/search/tt/entry.rs`
  - Reordered the file so `impl TtEntry` and `EntryResult` precede the
    `#[cfg(test)]` module.

### Mechanical clippy and API consistency

- `src/search/ordering.rs`
  - Renamed the `f3e5` / `f3d4` test variables to `capture_move` / `quiet_move`
    to avoid the trivial `similar_names` pedantic warning.

- Added `#[must_use]` to the public pure query functions listed in plan2
  (section 8.3):
  - `Position`: `new`, `board`, `legal_moves_vec`, `side_to_move`,
    `commoners`, `outcome`, `outcome_from_state`, `fen`.
  - `Search`: `new`, `nodes`, `child_evaluations`, `tt_stats`,
    `tt_best_child_counts`, `exit_reason`, `time_exceeded`,
    `move_order_breakdown`, `validate_pv`.
  - `TranspositionTable`: `probe_summary`, `probe_best_move`,
    `best_child_counts`, `stats`.
  - `TtEntry`: `result_for`, `result_for_depth`, `best_result`.
  - `ProofTree`: `extract_ppv`, `validate_ppv`, `is_terminal`, `from_bin`.
  - `notation`: `move_to_uci`, `uci_to_move`.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets       # zero warnings
$ cargo test --all-targets         # all tests pass
$ cargo test --release --all-targets  # all tests pass
$ cargo doc --no-deps              # succeeds, no broken links
```

Manual example checks:

```text
$ cargo run -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
outcome: win length: 3
pv: f1f7 e8d8 g1g8
...

$ cargo run --example chunk_growth -- --factor 1.5 --runs 2 --timeout 5
| growth     | outcome | ... |
| factor 3/2 | draw    | ... |

$ cargo run --example verify_ppv -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --moves "f1f7 e8d8 g1g8" --timeout 10
is_ppv: true

$ cargo run --example benchmark -- --runs 1 --timeout 5
... table produced as expected ...

$ cargo run --example twin_stats
... runs to completion ...
```

`cargo test --all-targets` and `cargo test --release --all-targets` both
passed with no failures.

## Skipped / deferred items

- `Position::board` was already private; no change was required.
- `cargo clippy --all-targets -- -W clippy::pedantic` still reports a number of
  pedantic warnings. We fixed the specific mechanical ones called out in
  plan2 (`unreadable_literal`, non-domain `similar_names`, and added the listed
  `#[must_use]` attributes). The remaining pedantic warnings are mostly
  documentation lints (`doc_markdown`, `missing_errors_doc`,
  `missing_panics_doc`), cast-truncation warnings in performance-sensitive
  paths, and a few `too_many_lines` / `redundant_closure` lints. These were
  considered domain-specific or outside the scope of this housekeeping pass and
  left for future plans.
- `examples/play_and_solve.rs` and other examples not explicitly listed in
  plan2 were not modified except where they depended on `examples/common.rs`.

## Follow-up items

- `src/search/dfpn/selection.rs` is at ~19.4 KiB, just under the 20 KiB soft
  limit. If it grows, its test module should be split into a separate file.
- The remaining pedantic clippy warnings should be triaged in a future cleanup
  plan; documentation lints are the largest category.
- Consider whether the `M19_FEN` and `STARTPOS_FEN` cross-boundary duplicates
  could be shared through a `dev` dependency or build script if they continue
  to drift.
