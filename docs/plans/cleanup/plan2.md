# Plan 2: Housekeeping — DRY, YAGNI, consistency, and module sizing

## Start

- Read `AGENTS.md`, `docs/plans/cleanup/plan1.md`, and
  `docs/plans/cleanup/report1.md` to avoid repeating work and to follow the
  established conventions.
- Run `cargo clippy --all-targets` and `cargo test --all-targets` to confirm the
  baseline is green.
- Run `cargo clippy --all-targets -- -W clippy::pedantic` and capture the
  mechanical, safe-to-fix warnings that are not domain-specific noise.
- Inspect source files over the 10 KB soft limit and the `examples/` helpers.

## Goal

Tighten the codebase after Plan 1 without changing any game-theoretic result:
remove remaining duplication and YAGNI surface, standardize small conventions,
and bring module sizing/headers into line with `AGENTS.md`.

## Background

Plan 1 fixed the first layer of dead code, visibility, and small mechanical
lints. Several follow-up items and new inconsistencies remain:

- `Outcome` formatting and parsing is hand-rolled in four places
  (`main.rs`, `examples/benchmark.rs`, `examples/chunk_growth.rs`,
  `examples/verify_ppv.rs`, `tests/test_cli.rs`, and `tests/test_corpus.rs`).
- `examples/chunk_growth.rs` re-implements the f64-to-reduced-fraction conversion
  that already lives in `Search`. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/mod.rs" lines="56-82" />
- `examples/common.rs::parse_uci` duplicates `notation::uci_to_move`, and
  `parse_move` manually re-implements promotion matching.
- `Search` exposes two public entry points (`set_log_writer`,
  `search_with_settings`) and `Position` still exposes the `board` field, all
  without consumers.
- Several source files exceed 10 KB with no size justification in the module
  header; `src/proof_tree/mod.rs` exceeds 20 KB.
- A few pedantic lints are safe and mechanical to fix (`unreadable_literal`,
  non-domain `similar_names`, and `must_use_candidate`).

## Implementation tasks

### 1. DRY `Outcome` formatting and parsing

1.1 In `src/position.rs`:
  - Add `Outcome::as_str(&self) -> &'static str` returning `"win"`, `"loss"`,
    or `"draw"`.
  - Implement `std::str::FromStr for Outcome` (case-insensitive, error type
    `String`).
  - Make the existing `Display` impl for `Outcome` delegate to `as_str()`.
  - Add `#[must_use]` to `Outcome::as_str`.

1.2 Remove the four duplicated `outcome_str` helpers:
  - `src/main.rs` lines 83–89.
  - `examples/benchmark.rs` lines 196–201.
  - `examples/chunk_growth.rs` lines 203–209.
  - `examples/verify_ppv.rs` lines 34–40.
  - Replace every call site with `outcome.as_str()`.

1.3 Replace the hand-rolled parsers:
  - `tests/test_cli.rs::parse_outcome` becomes
    `s.parse::<Outcome>().ok()`.
  - `tests/test_corpus.rs::parse_expected` becomes
    `token.parse::<Outcome>().ok()`, removing the `to_lowercase()` dance.

### 2. DRY f64-to-fraction conversion

2.1 In `src/search/dfpn/mod.rs`:
  - Rename the private `epsilon_fraction(v: f64)` helper to `fraction_from_f64`
    (it is not specific to epsilon).
  - Add a public `Search::set_chunk_multiplier_from_factor(&mut self, factor: f64) -> (u64, u64)` that:
    - asserts `factor >= 1.0`,
    - calls `fraction_from_f64(factor)`,
    - sets `chunk_multiplier_num` / `chunk_multiplier_den` (or calls the existing
      `set_chunk_multiplier` if it is kept),
    - returns the reduced `(num, den)` for display.
  - `set_epsilon` should now call `fraction_from_f64(1.0 + epsilon)`.

2.2 In `examples/chunk_growth.rs`:
  - Remove the duplicated `factor_fraction` and `gcd` functions.
  - Rename the `Result` struct to `ChunkResult` to stop shadowing
    `std::result::Result`.
  - Update `run_once` and `bench_mode` to take `factor: Option<f64>` and use
    `search.set_chunk_multiplier_from_factor(factor)` for geometric growth.
  - If `Search::set_chunk_multiplier` has no callers after the update, remove it
    as YAGNI.

### 3. DRY example helpers

3.1 In `examples/common.rs`:
  - Remove `parse_uci` (it is identical to `atomic_solver::notation::uci_to_move`).
  - Simplify `parse_move` to build a UCI string (`{from}{to}{promo?}`) and
    delegate to `notation::uci_to_move`, removing `parse_promotion`, the
    `MoveList` scan, and the `MoveType`/`PieceType` imports.

3.2 In `examples/verify_ppv.rs`:
  - Replace `common::parse_uci` with `atomic_solver::notation::uci_to_move`.

3.3 In `examples/twin_stats.rs`:
  - Replace `pv.iter().map(|m| m.to_uci())` with
    `pv.iter().copied().map(move_to_uci)` (or equivalent) so the codebase
    consistently uses `notation::move_to_uci`.

### 4. YAGNI and visibility cleanup

4.1 In `src/search/dfpn/mod.rs`:
  - Remove `Search::set_log_writer` and the `log_writer` field; `log_chunk`
    should write directly to stderr. Remove the `std::io::Write` import.
  - Remove `Search::search_with_settings`; it has no callers.

4.2 In `src/position.rs`:
  - Make the `board` field private. All external call sites already use the
    `board()` accessor. <ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="55-84" />
  - Restrict `try_do_move` to `#[cfg(test)] pub(crate)` and remove the
    `#[allow(dead_code)]` guard; it is only used in `position.rs` unit tests.

### 5. Structural DRY in search and position

5.1 In `src/search/dfpn/mod.rs`:
  - Refactor `begin_run` to call `reset_search_state`, then set the counters and
    timing fields. The duplicated path-stack/move-stack/proof-path clearing
    should live in only one place.

5.2 In `src/search/dfpn/core.rs`:
  - Add a private `Search::with_child_path(mv, f)` helper that updates
    `proof_path` and `move_stack` only when a proof-tree sender is configured
    and then executes `f(&mut self)`. Use it to write the recursive `dfpn` call
    once instead of duplicating it in the `emit` / `!emit` branches.
    Benchmark on `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` to ensure no regression.

5.3 In `src/position.rs`:
  - Extract a private `fn refresh_zobrist(&mut self)` and call it from both
    `do_move` and `undo_move`.
  - Make `legal_moves_with_state` call `self.populate_state(state)` instead of
    duplicating the `board.populate_state` call.

### 6. Constants and literal duplication

6.1 Document intentional cross-boundary duplication:
  - Add a comment in `examples/common.rs` and `tests/common/mod.rs` noting that
    `M19_FEN` is duplicated because examples and integration tests cannot share
    modules.
  - Add a comment in `src/cli.rs` explaining that its `STARTPOS_FEN` mirrors
    `Position::STARTPOS_FEN` so the parser stays std-only.

6.2 Replace remaining hardcoded start-FEN literals with
  `Position::STARTPOS_FEN` where the file already has access to `Position`:
  - `src/notation.rs` unit tests.
  - `src/zobrist.rs` unit tests.
  - `src/search/dfpn/history.rs` `start_position` test helper.
  - `src/proof_tree/mod.rs` `to_bin_round_trips_small_tree` test.
  - `src/position.rs` `try_do_move` test.
  - Update the `main.rs` module doc comment to reference `Position::STARTPOS_FEN`
    instead of repeating the full FEN string.

6.3 Fix remaining `unreadable_literal` warnings:
  - `src/search/dfpn/mod.rs` `0xfffffffffffff` becomes
    `0x000f_ffff_ffff_ffff` (or disappears when `chunk_growth.rs` stops
    duplicating the conversion).
  - LCG constants in `src/zobrist.rs` and `tests/test_position.rs` become
    `6_364_136_223_846_793_005` and `1_442_695_040_888_963_407`.
    Optionally extract them to a shared `#[cfg(test)]` helper if both test
    sites should use the same names.

### 7. Module-size documentation and organization

7.1 Add module-level header comments justifying files over 10 KB that lack one:
  - `src/search/dfpn/children.rs`
  - `src/search/dfpn/core.rs`
  - `src/search/dfpn/history.rs`
  - `src/search/dfpn/selection.rs` (also note it is close to the 20 KB soft
    limit and may need test splitting if it grows)
  - `src/position.rs`
  - `src/cli.rs`
  - `src/main.rs`

7.2 `src/proof_tree/mod.rs` is the only file over the 20 KB soft limit. Split it:
  - Move `ProofTreeWorker`, `ProofMessage`, `ProofResponse`, and `ProofStats` into
    a new `src/proof_tree/worker.rs`.
  - Keep `ProofNode` and `ProofTree` (data model, `to_bin`/`from_bin`,
    `extract_ppv`, `validate_ppv`) in `src/proof_tree/mod.rs`.
  - Re-export `ProofTreeWorker`, `ProofMessage`, `ProofResponse`, and
    `ProofStats` from `src/proof_tree/mod.rs` so the public API is unchanged.
  - Move worker-specific unit tests into `worker.rs`.

7.3 In `src/search/tt/entry.rs`, reorder so `impl TtEntry` and `EntryResult`
    precede the `#[cfg(test)]` module.

### 8. Mechanical clippy and API consistency

8.1 In `src/search/ordering.rs`, rename the `f3e5` / `f3d4` test variables to
    something like `capture_move` / `quiet_move` to avoid the trivial
    `similar_names` pedantic warning.

8.2 For the `reply_tx2` / `reply_rx2` channel pairs in `src/proof_tree/mod.rs`,
    prefer a targeted `#[allow(clippy::similar_names)]` if renaming would hurt
    clarity; otherwise use longer distinct names.

8.3 Add `#[must_use]` to the remaining public pure query functions where
    ignoring the result is a real bug:
  - `Position`: `new`, `board`, `legal_moves_vec`, `side_to_move`, `commoners`,
    `outcome`, `outcome_from_state`, `fen`.
  - `Search`: `new`, `nodes`, `child_evaluations`, `tt_stats`,
    `tt_best_child_counts`, `exit_reason`, `time_exceeded`, `move_order_breakdown`,
    `validate_pv`.
  - `TranspositionTable`: `probe_summary`, `probe_best_move`, `best_child_counts`,
    `stats`.
  - `TtEntry`: `result_for`, `result_for_depth`, `best_result`.
  - `ProofTree`: `extract_ppv`, `validate_ppv`, `is_terminal`, `from_bin`.
  - `notation`: `move_to_uci`, `uci_to_move`.
  - Do **not** add `#[must_use]` to setters or methods with side effects such as
    `do_move`, `undo_move`, `solve`, `store`, or `clear`.

## File changes

- `src/position.rs`
- `src/notation.rs`
- `src/search/dfpn/mod.rs`
- `src/search/dfpn/core.rs`
- `src/search/ordering.rs`
- `src/search/tt/entry.rs`
- `src/proof_tree/mod.rs` (split; new `src/proof_tree/worker.rs`)
- `src/cli.rs` (comment only)
- `src/main.rs`
- `examples/benchmark.rs`
- `examples/chunk_growth.rs`
- `examples/common.rs`
- `examples/twin_stats.rs`
- `examples/verify_ppv.rs`
- `tests/test_cli.rs`
- `tests/test_corpus.rs`
- `tests/test_position.rs`

## Risks

- Removing public methods (`set_log_writer`, `search_with_settings`, and
  possibly `set_chunk_multiplier`) and making `Position::board` private are API
  changes. Verify with `cargo check --all-targets` that there are no callers.
- `Outcome::FromStr` and `Outcome::as_str` are new public API. Keep their
  behavior identical to the existing `Display` impl and the removed `outcome_str`
  helpers.
- `Search::with_child_path` touches the recursive search hot path. Run the
  benchmark examples and the regression positions to confirm no measurable slow-
  down.
- Splitting `proof_tree/mod.rs` is structural; ensure `pub use` re-exports and
  `crate::proof_tree` consumers (search and `inspect_pt`) keep compiling.
- `#[must_use]` additions are safe but may surface ignored-return warnings in
  tests or examples if any call site discards a result. Fix those call sites,
  do not remove the attribute.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo test --release
$ cargo doc --no-deps
$ cargo run -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --example benchmark
$ cargo run --example chunk_growth -- --factor 1.5 --runs 2 --timeout 5
$ cargo run --example verify_ppv -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --moves "f1f7 e8d8 g1g8" --timeout 10
$ cargo run --example twin_stats
```

Additional checks:

- `cargo clippy --all-targets` reports zero warnings.
- The decisive position still prints `outcome: win` and `pv: f1f7 e8d8 g1g8`.
- `chunk_growth` still produces the expected geometric-chunk table.
- `verify_ppv` still reports `is_ppv: true` for the known two-rook mate.
- All files that remain over 10 KB have a documented justification in their
  module header.
- No source file exceeds 20 KB without an explicit plan to split or an
  `AGENTS.md` note.

## Final task

Write `docs/plans/cleanup/report2.md` documenting which cleanups were applied,
which were intentionally skipped or deferred, the `cargo test` and
`cargo clippy` results, and any follow-up items.
