# Report: Plan 1 — Housekeeping cleanup

This report documents the cleanups applied from `docs/plans/cleanup/plan1.md`, items intentionally skipped, and verification results.

## Applied cleanups

### Constants and duplication

- `src/position.rs`
  - Added `Position::STARTPOS_FEN` and used it from `Position::new()`.
  - Merged the identical `Outcome::Loss` and `Outcome::Draw` arms in `Outcome::to_pn_dn`.
  - Removed the redundant `else` after the early `return` in `outcome_from_state`.
  - Added a `Clone` impl comment noting that clones start with an empty `undo_stack`.
  - Removed unused `PartialOrd`/`Ord` derives from `Outcome`.
  - Added `#[must_use]` to `Outcome::to_pn_dn`, `Outcome::pn_dn_for`, `Outcome::flip`, `Position::hash`, and `Position::repetition_key`.

- `src/zobrist.rs`
  - Extracted the xor/shift/mul mixing round into a private `fn mix(z: u64) -> u64`.
  - Rewrote `SplitMix64::next` and `splitmix64` to call `mix` and share the constants.
  - Added underscores to the long hex literals for readability.
  - Updated the `path_random` comment to describe `move_index` and `MAX_PATH_DEPTH` instead of the stale `PATH_MOVE_NB`.

- `src/main.rs`
  - Replaced the duplicated start-FEN literal with `Position::STARTPOS_FEN`.
  - Imported `Outcome` and used `Outcome::Win`/`Loss`/`Draw` in the match and `matches!` blocks.

- `src/search/dfpn/mod.rs`
  - Replaced the duplicated `pub const INF: u64 = zobrist::INF;` with `pub use crate::zobrist::INF;`.
  - Extracted the duplicated start-of-run reset sequence into a private `begin_run` helper used by `search_depth` and `solve`.
  - Made all `Search` fields private.

### Dead and redundant code

- `src/notation.rs`
  - Removed unused `square_to_uci`.

- `src/search/dfpn/children.rs`
  - Removed the `vpn`/`vdn` fields from `ChildInfo` and `ChildSelection`.
  - `best_child` is now a `(Move, u64, u64)` 3-tuple.
  - Updated `evaluate_child` construction sites and `core.rs` destructuring.
  - `best_and_second_unsolved` now sorts by `pn`/`dn` directly.
  - Replaced `map(...).unwrap_or(...)` calls with `map_or` / `is_some_and`.

### Visibility and encapsulation

- `src/search/tt/entry.rs`
  - Changed `TtEntry` and `TwinEntry` fields from `pub` to `pub(crate)`.
  - Kept the structs and public methods (`find_result_for_path`, `best_result_for_path`) unchanged.

- `src/position.rs`
  - Made `zobrist` private; public callers should use `hash()` and `repetition_key()`.
  - Kept `board` public for now because examples and the scorer access it directly.

### Small correctness-safe refactorings

- `src/search/tt/table.rs`
  - Changed `probe` from `.find(|&&e| ...)` to `.find(|e| ...)` so the closure borrows instead of copying a `TtEntry` on every bucket scan.

- `src/search/dfpn/core.rs`
  - Replaced the two inline `Instant::now() >= self.deadline` checks with `self.time_exceeded()`.
  - Nested the `Outcome::Win | Outcome::Loss` or-pattern in the `store_remaining_depth` match.
  - Updated `best_child` destructuring to the new 3-tuple shape.

- `src/search/dfpn/pv.rs`
  - Changed `validate_pv_prefix` to return `Option<Position>`.
  - `validate_pv` now uses that returned position for the final terminal check instead of re-cloning the root and re-applying every PV move.
  - `extract_pv_checked` uses `.is_some()` on the prefix result.

- `src/search/dfpn/selection.rs`
  - Renamed `win_idx`/`loss_idx` (and related depth variables) to `parent_win_child_idx` / `parent_loss_child_idx` to make the inverted parent/child semantics explicit.
  - Added a doc comment on `is_solved_by_children` explaining the naming.

### Example and test DRY

- Added `examples/common.rs` with `M19_FEN` and a shared `parse_move` helper.
- Updated `find_winning_child`, `play_and_solve`, `solve_depth_limited`, and `static_move_scores` to use the shared helpers and clearer error messages in `play_and_solve`.
- Added `tests/common/mod.rs` with shared helpers: `solve`, `solve_with_timeout`, `solve_with_pv`, `solve_refined`, `solve_refined_moves`, `pv_strings`, and `cli_bin`.
- Deduplicated the `solve`/`solve_refined`/`cli_bin` helpers in `test_inf.rs`, `test_plan1.rs`, `test_plan2.rs`, `test_plan4.rs`, `test_plan6.rs`, `test_epsilon.rs`, `test_review.rs`, `test_terminal_ordering.rs`, and `test_repetition.rs`.

### Mechanical clippy cleanups

- Added underscores to unreadable hex literals in `src/zobrist.rs` and `src/search/dfpn/tests.rs`.
- Inlined format arguments in `src/search/dfpn/pv.rs`, `src/search/tt/tests.rs`, and the touched test files.
- Added backticks around example names in example doc comments.
- Added module-level `#[allow(clippy::similar_names)]` in `src/search/dfpn/core.rs`, `children.rs`, and `selection.rs` for the paired `pn`/`dn` variable names.

## Skipped or intentionally deferred

- `Position::board` was left public. A follow-up can add `Position::board(&self) -> &Board` and make the field private.
- Additional pedantic clippy lints not listed in plan 6.1 (e.g. `missing_panics_doc`, `missing_errors_doc`, `must_use_candidate`, `cast_possible_truncation`) were not addressed to keep the change set focused.
- `solve_no_refinement.rs` and `twin_stats.rs` were not moved to the shared `M19_FEN` because they use different default positions.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo test --release
$ cargo doc --no-deps
$ cargo run -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --example play_and_solve
$ cargo run --example solve_depth_limited
$ cargo run --example solve_no_refinement
$ cargo run --example static_move_scores
```

Results:

- `cargo fmt` completed with no changes needed after the final pass.
- `cargo clippy --all-targets` reports zero warnings.
- `cargo test --all-targets` and `cargo test --release` pass all tests.
- `cargo doc --no-deps` builds cleanly.
- The CLI run for `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` still prints `outcome: win` and `pv: f1f7 e8d8 g1g8`.
- All source files remain under 10 KB; the largest is `src/search/dfpn/core.rs` at ~9.4 KB.

## Follow-up items

- Consider adding `Position::board(&self) -> &Board` and making `Position::board` private in a later plan.
- Re-evaluate whether remaining pedantic clippy warnings should be addressed after the public API stabilizes.
