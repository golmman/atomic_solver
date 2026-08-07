# Report 2: Improve testability

This report documents the work done for `docs/plans/testability/plan2.md`:
refactoring the CLI, encapsulating the `Position` board, moving core DF-PN
selection/PV helpers to free functions, making `Search` output inspectable,
adding test constructors, and extending the shared test helpers.

## Summary

* Created a standalone, testable `src/cli.rs` module and rewrote `src/main.rs`
  to use it.
* Made `Position::board` private and provided `board()`/`populate_state()`/
  `legal_moves_vec()`/`try_do_move()` helpers so external callers do not need
  to mutate the raw `Board`.
* Moved `is_solved_by_children`, `select_child_with_early_exit`,
  `best_and_second_unsolved`, `second_best_unsolved_excluding`,
  `selection_for_child`, and `select_from_children` from `Search` methods to
  module-level free functions in `src/search/dfpn/selection.rs`.
* Extracted the history/killer logic in `src/search/dfpn/history.rs` into
  small, testable helper functions with focused unit tests.
* Moved `validate_pv` and `validate_pv_prefix` to module-level free functions
  in `src/search/dfpn/pv.rs`; kept `Search::validate_pv` as a thin wrapper.
* Made `dfpn` chunk logging inspectable via `Search::set_log_writer` and
  documented the public status accessors (`nodes`, `child_evaluations`,
  `exit_reason`).
* Added `TranspositionTable::with_capacity` and kept `bucket_count` exposed
  for unit tests.
* Split `ProofTreeWorker::spawn` into `ProofTreeWorker::new` and
  `ProofTreeWorker::handle_message`, allowing synchronous unit tests without
  spawning threads.
* Extended `tests/common/mod.rs` with `pv_from_uci`,
  `assert_position_invariants`, `assert_solves_with_first_move`,
  `assert_pv_valid`, and other helper utilities.
* Updated `examples/static_move_scores.rs` and `tests/test_position.rs` to
  use the new `Position` API.

All verification commands pass (see below).

## CLI refactor

`src/cli.rs` now owns all command-line parsing. It exposes:

* `pub struct CliOptions` — strongly typed option bag.
* `pub enum ParseResult` — either `Options(CliOptions)` or `Help`.
* `pub fn parse_args(args: &[String]) -> Result<ParseResult, String>` —
  pure parser that can be unit-tested without spawning the binary.
* `pub const STARTPOS_FEN` and `pub fn help_message()`.

`src/main.rs` was reduced to the orchestration layer: parse, build `Search`,
set timeout/TT/proof-tree options, solve, and print the result. The CLI module
is declared in both `src/main.rs` and `src/lib.rs` (`#[cfg(test)] mod cli;`)
so unit tests can exercise it without the binary being built.

`CliOptions` contains an `f64` (`epsilon`), so it derives `PartialEq` but not
`Eq`.

### New `cli` unit tests

| Test | Purpose |
|------|---------|
| `defaults_match_documentation` | All defaults match the CLI help text. |
| `help_short_and_long` | `-h` and `--help` return `ParseResult::Help`. |
| `fen_is_parsed`, `tt_size_is_parsed`, `epsilon_is_parsed`, `timeout_is_parsed`, `pt_size_is_parsed`, `dump_path_is_parsed`, `first_outcome_is_parsed`, `outcome_only_is_parsed` | Each option is parsed correctly. |
| `missing_value_returns_err` | Options that require a value error when omitted. |
| `unknown_option_returns_err` | Unknown flags produce a clear error. |
| `non_positive_tt_size_rejected` | `0` and `-1` for `--tt-size` are rejected. |
| `non_positive_timeout_rejected` | `0` and `-1` for `--timeout` are rejected. |
| `out_of_range_epsilon_rejected` | `--epsilon` outside `[0, 1]` is rejected. |

## `Position` encapsulation

The `board` field is no longer `pub`. Callers receive a read-only reference via
`Position::board()`. New helpers:

* `pub fn board(&self) -> &Board` — read-only access for scoring and diagnostics.
* `pub fn populate_state(&self, state: &mut StateInfo)` — wraps
  `Board::populate_state`.
* `pub fn legal_moves_vec(&self) -> Vec<Move>` — convenience helper for tests.
* `pub(crate) fn try_do_move(&mut self, m: Move) -> bool` — plays `m` only if it
  is legal, intended for fuzz/property tests.

`examples/static_move_scores.rs` and `tests/test_position.rs` were updated to
use `pos.board()` and `pos.populate_state()` instead of accessing the raw
field.

### New `Position` unit test

* `try_do_move_accepts_legal_and_rejects_illegal` — confirms that legal moves
  are applied and illegal moves leave the position unchanged.

## DF-PN selection helpers

The selection logic was split out of `src/search/dfpn/children.rs` and
`src/search/dfpn/core.rs` into a dedicated `src/search/dfpn/selection.rs`
module. The following free functions are now available for unit tests without
constructing a `Search`:

* `is_solved_by_children`
* `select_child_with_early_exit`
* `best_and_second_unsolved`
* `second_best_unsolved_excluding`
* `selection_for_child`
* `select_from_children`

`children.rs` now only contains `ChildInfo`/`ChildSelection` and child
evaluation; `core.rs` calls `selection::select_from_children`.

### New `selection` unit tests

| Test | Purpose |
|------|---------|
| `win_picks_shortest_loss_child` | A winning parent chooses the shortest decisive child. |
| `and_node_win_picks_longest_loss_for_attacker` | AND-node loss delays as long as possible. |
| `and_node_defender_win_picks_shortest_loss` | AND-node win is the shortest child Loss. |
| `draw_picks_longest_draw_child` | Draws choose the longest draw line. |
| `loss_picks_longest_win_child` | Losing parents choose the most resistant child. |
| `unsolved_returns_none` | Mixed solved/unsolved returns `None`. |
| `win_with_unsolved_returns_not_all_solved` | A single Win can be returned even if siblings are unsolved. |
| `mixed_win_and_draw_children_is_draw` | All solved with at least one Draw → Draw. |
| `mixed_win_depths_returns_longest_loss` | All-Loss children pick the longest. |
| `early_exit_allows_win_when_not_all_solved` | `select_child_with_early_exit` returns a Win without waiting for all children. |
| `select_from_children_can_be_called_without_search_instance` | Free function is directly callable. |
| `best_and_second_unsolved_orders_by_or_pn` | Best/second unsolved indices are chosen by `pn` at OR nodes. |
| `solved_children_are_skipped_by_best_and_second` | Solved children are excluded from unsolved ordering. |

## History / killer helpers

`src/search/dfpn/history.rs` was refactored so the history-table and
killer-slot logic is in small, named helper functions:

* `update_history_entry`
* `age_history`
* `update_killer_slots`
* `killer_bonus`

`Search::sort_moves`, `Search::update_history`, `Search::update_killers`, and
`Search::maybe_age_history` delegate to these helpers. A new public method,
`Search::move_order_breakdown`, returns a diagnostic `(Move, static, history,
killer, total)` tuple for each legal move; this is used by the
`move_order_debug` example.

### New `history` unit tests

| Test | Purpose |
|------|---------|
| `sort_orders_empty_list_without_panic` | Empty move lists sort safely. |
| `sort_is_deterministic` | Repeated sorting of the same position yields the same order. |
| `history_bonus_raises_move_score` | A history-bonus move moves up the sorted list. |
| `history_caps_at_maximum` | Repeated updates cap at `HISTORY_MAX`. |
| `update_history_entry_helper_caps_at_max` | Unit helper caps correctly. |
| `age_history_halves_scores` | Aging halves a search's history scores. |
| `age_history_helper_halves_all_entries` | The free `age_history` helper halves every entry. |
| `update_killers_shifts_previous_killer_to_second_slot` | Killer slots shift and deduplicate. |
| `update_killer_slots_helper_shifts_and_deduplicates` | Free helper behaves the same as the method. |
| `killer_moves_get_sort_bonus` | A killer move is sorted first. |
| `killer_bonus_helper_matches_method` | Free `killer_bonus` matches `Search::killer_bonus`. |

## PV validation helpers

`validate_pv` and `validate_pv_prefix` are now free functions at module scope in
`src/search/dfpn/pv.rs`. `impl Search` still provides `pub fn validate_pv(...)`
so existing callers (`tests/common/mod.rs`, `examples/verify_ppv.rs`) do not
need to change.

Existing PV unit tests were updated to call the free functions, and a new
unit test (`validate_pv_accepts_three_ply_mate`) was already present.

## `Search` testability

`src/search/dfpn/mod.rs` changes:

* Added `log_writer: Option<Box<dyn Write + Send>>` and
  `Search::set_log_writer(writer)` so unit tests can capture `log_chunk`
  output instead of `eprintln!`.
* `log_chunk` now writes to the configured writer, falling back to `eprint!`.
* Added doc comments to `Search::nodes`, `Search::child_evaluations`, and
  `Search::exit_reason`.

These changes keep the hot `dfpn` loop untouched and avoid trait objects in
performance-sensitive code; `set_log_writer` is only called from tests or
diagnostic harnesses.

## Transposition-table test constructor

`src/search/tt/table.rs` now exposes:

* `pub(crate) fn with_capacity(buckets: usize) -> Self` — builds a table with an
  exact (power-of-two rounded) bucket count, letting unit tests force
  collisions and eviction.
* `pub(crate) fn bucket_count(&self) -> usize` — returns the allocated bucket
  count for assertions.

### New `tt` unit tests

| Test | Purpose |
|------|---------|
| `with_capacity_rounds_to_power_of_two` | `0`, `1`, and `3` buckets round to `1` and `4`. |
| `with_capacity_forces_deterministic_eviction` | A one-bucket, two-slot table evicts exactly one old entry when a third key is stored. |

## Proof-tree worker refactor

`src/proof_tree/mod.rs` changes:

* Added `ProofTreeWorker::new(root_fen, budget, memory_limited)` — synchronous
  constructor that does not spawn a thread.
* Added `ProofTreeWorker::handle_message(msg) -> Option<ProofResponse>` —
  processes a single `ProofMessage` and returns any response.
* `ProofTreeWorker::spawn` now builds a worker with `new` and runs the message
  loop in a thread.

### New `proof_tree` unit tests

| Test | Purpose |
|------|---------|
| `worker_new_does_not_spawn_thread` | A worker can be created and queried synchronously. |
| `handle_message_processes_out_of_order_events` | Pending child events are attached once their parent arrives. |
| `handle_message_clears_tree` | `ProofMessage::Clear` resets the tree. |
| `memory_limited_flag_triggers_at_small_budget` | A zero-byte budget sets the `memory_limited` flag on the first node. |

## Shared test helpers

`tests/common/mod.rs` was extended with:

* `pv_from_uci(start, uci) -> Vec<Move>` — replay UCI moves and produce a
  validated internal `Move` list.
* `assert_position_invariants(pos)` — hash invariant and terminal/move-list
  consistency.
* `assert_solves_with_first_move(fen, expected, first)` — solve and assert the
  first move.
* `assert_pv_valid(fen, expected, pv)` — validate a supplied PV with
  `Search::validate_pv`.

`assert_solves_to` retains its optional `max_pv_len` argument for backwards
compatibility with existing tests; the parameter is documented as no longer
enforced.

## Verification

All verification commands from the plan were run:

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo test --release -- --ignored
cargo test --all-targets
cargo doc --no-deps
```

Results:

* `cargo fmt` — clean.
* `cargo clippy --all-targets` — clean.
* `cargo test` (debug) — all active tests pass; slow tests are ignored as
  designed.
* `cargo test --all-targets` — passes (includes example unit tests and
  integration tests).
* `cargo test --release -- --ignored` — passes; the unconditional
  `#[ignore]` proof-tree tests run and pass in release.
* `cargo doc --no-deps` — builds without warnings.

## Design notes / deviations from the plan

* `Position::try_do_move` is `pub(crate)` rather than `pub` because it is
  intended for in-crate tests and fuzz harnesses, not the public API.
* `TranspositionTable::with_capacity` and `bucket_count` are `pub(crate)`
  (compiled only under `#[cfg(test)]` for `bucket_count`) to keep the test
  surface small.
* `assert_solves_to` keeps its third `max_pv_len: Option<usize>` argument from
  Plan 1; the new helpers `assert_solves_with_first_move` and `assert_pv_valid`
  provide the additional assertions requested by Plan 2 without breaking
  existing call sites.
* `src/lib.rs` declares `#[cfg(test)] mod cli;` so the CLI unit tests compile
  without introducing dead-code warnings in non-test library builds.

## Remaining opportunities

* Add property-based tests that exercise `Position::try_do_move` through random
  legal move sequences.
* Use `Search::set_log_writer` in a `dfpn` unit test to verify chunk logging
  output.
* Add more `with_capacity` eviction corner cases (same-key overwrites,
  generation replacement).
