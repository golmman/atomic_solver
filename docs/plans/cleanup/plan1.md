# Plan 1: Housekeeping — DRY, dead code, visibility, and small cleanups

## Start

- Read `AGENTS.md` to confirm the project conventions and quality gates.
- Inspect the source tree that was created/refined by the previous review plans
  (especially `docs/plans/review/plan18.md` and `report18.md`).
- Focus on `src/position.rs`, `src/zobrist.rs`, `src/notation.rs`,
  `src/main.rs`, `src/search/**/*.rs`, `examples/*.rs`, and `tests/*.rs`.

## Goal

Tighten the codebase without changing any game-theoretic result:

- remove dead code and YAGNI data fields,
- eliminate duplicated constants and helper logic,
- tighten visibility so implementation details are not exposed in the public API,
- fix small mechanical inefficiencies and clippy pedantic warnings that are
  clearly safe to address.

## Background

The recent module split left the repository in good structural shape, but
several leftover artifacts remain:

- `src/notation.rs` exports `square_to_uci`, which is never called.
- `src/zobrist.rs` implements the SplitMix64 mixing round twice: once inside
  `SplitMix64::next` and once in the standalone `splitmix64` function.
- `src/search/dfpn/mod.rs` redefines a public `INF` constant that already lives
  in `zobrist`.
- `src/search/dfpn/children.rs` carries `vpn`/`vdn` fields that are always
  identical to `pn`/`dn` and ignored by the search.
- `Search` fields are `pub(crate)` although only the `dfpn` module tree uses
  them.
- `TtEntry` and `TwinEntry` expose all fields publicly although only code
  inside the crate accesses them.
- `Position` exposes `pub zobrist` even though `hash()` and `repetition_key()`
  are the intended accessors.
- `TranspositionTable::probe` uses `|&&e|`, which copies a large `TtEntry` on
  every lookup.
- `dfpn` checks `Instant::now() >= self.deadline` inline in `core.rs` while
  `Search::time_exceeded()` already exists.
- `Search::search_depth` and `Search::solve` duplicate the same run-start reset
  sequence.
- `validate_pv` re-applies the whole PV after `validate_pv_prefix` already did.
- The standard start FEN is hardcoded in `Position::new()` and `main.rs`;
  several examples repeat the same m19 regression FEN and parsing helpers.
- Pedantic clippy reports low-risk cleanups (`redundant_else`, `match_same_arms`,
  `unreadable_literal`, `unnested_or_patterns`, `uninlined_format_args`).

## Implementation tasks

### 1. Centralize constants and remove duplication

1.1 In `src/zobrist.rs`:
  - Extract the xor/shift/mul mixing round into a private `fn mix(z: u64) -> u64`.
  - Rewrite `SplitMix64::next` as `self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15); mix(self.0)`.
  - Rewrite `splitmix64(x)` as `let z = x.wrapping_add(0x9e37_79b9_7f4a_7c15); mix(z)`.
  - This removes the duplicated constants and operations while preserving the
    exact key stream.
  - Add underscores to the long hex literals for readability.
  - Fix the stale `PATH_MOVE_NB` reference in the `path_random` comment;
    describe `move_index` and `MAX_PATH_DEPTH` instead.

1.2 In `src/position.rs`:
  - Add a public constant `pub const STARTPOS_FEN: &str =
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";`.
  - Make `Position::new()` use `STARTPOS_FEN` instead of duplicating the string.
  - Merge the identical `Loss` and `Draw` arms in `Outcome::to_pn_dn`:
    `Outcome::Loss | Outcome::Draw => (zobrist::INF, 0)`.
  - Remove the redundant `else` block in `outcome_from_state`
    (lines 116–118).
  - Add a short comment to the `Clone` impl explaining that the clone starts
    with an empty `undo_stack` (it is a snapshot, not a replayable history).

1.3 In `src/search/dfpn/mod.rs`:
  - Replace `pub const INF: u64 = zobrist::INF;` with
    `pub use crate::zobrist::INF;`.
  - This keeps `dfpn::INF` in the public API but removes a second definition.

1.4 In `src/main.rs`:
  - Import `Position::STARTPOS_FEN` and use it for the default FEN.
  - Import `Outcome` from `atomic_solver::position` to remove the fully
    qualified names in the `match` and `matches!` blocks.

### 2. Remove dead and redundant code

2.1 In `src/notation.rs`:
  - Remove `pub fn square_to_uci` (it is unused).
  - Keep `move_to_uci` because it is used throughout the CLI, examples, and
    tests as a stable helper.

2.2 In `src/search/dfpn/children.rs`:
  - Remove the `vpn` and `vdn` fields from `ChildInfo` and `ChildSelection`.
  - In `best_and_second_unsolved`, sort by `pn`/`dn` directly.
  - Change `best_child` from a 5-tuple to `(Move, u64, u64)`.
  - Update `core.rs` destructuring of `selection.best_child` accordingly.
  - Update the `ChildInfo` construction sites in `evaluate_child`.
  - These fields have always equaled `pn`/`dn` and are ignored by the engine, so
    removing them is purely a simplification.

2.3 In `src/position.rs`:
  - If `PartialOrd`/`Ord` on `Outcome` are unused, remove those derives.
    If any test or source code relies on ordering, leave them in place and add
    a comment explaining that the derived order is `Loss < Draw < Win`.

### 3. Visibility and encapsulation

3.1 In `src/search/dfpn/mod.rs`:
  - Change all `pub(crate)` fields on `Search` to private.
  - No external code accesses these fields (examples and tests use the public
    methods), and the `dfpn` submodules are in the same module tree, so
    private visibility is sufficient.

3.2 In `src/search/tt/entry.rs`:
  - Change `TtEntry` and `TwinEntry` fields from `pub` to `pub(crate)`.
  - Keep the public methods (`find_result_for_path`, `best_result_for_path`)
    and the re-exports in `search::tt` unchanged.

3.3 In `src/position.rs`:
  - Make `pub zobrist` private. The public API should use `hash()` and
    `repetition_key()`.
  - Keep `pub board` for now (examples and the scorer use direct access);
    consider adding `Position::board(&self) -> &Board` and hiding the field in a
    follow-up plan.

### 4. Small correctness-safe refactorings

4.1 In `src/search/tt/table.rs`:
  - Change `probe` from `.find(|&&e| e.valid && e.key == key)` to
    `.find(|e| e.valid && e.key == key)`.
  - The old closure copies a `TtEntry` on every bucket scan; the new closure
    borrows.

4.2 In `src/search/dfpn/core.rs`:
  - Replace the two inline `Instant::now() >= self.deadline` checks with
    `self.time_exceeded()`.
  - Nest the or-pattern in the `store_remaining_depth` match:
    `Some(Outcome::Win | Outcome::Loss) => u32::MAX`.

4.3 In `src/search/dfpn/mod.rs`:
  - Extract the duplicated start-of-run reset sequence into a private helper:
    ```rust
    fn begin_run(&mut self) {
        self.nodes = 0;
        self.start = Instant::now();
        self.deadline = self.start + self.timeout;
        self.path.clear();
        self.path_stack.clear();
        self.path_code = 0;
        self.last_pv.clear();
    }
    ```
  - Use it in `search_depth` and `solve`.

4.4 In `src/search/dfpn/pv.rs`:
  - Change `validate_pv_prefix` to return `Option<Position>`: the position after
    playing the verified prefix, or `None` if a move was illegal.
  - Have `validate_pv` use that returned position for the final terminal check
    instead of re-cloning the root and re-applying every PV move.
  - Keep `extract_pv_checked` warning behavior unchanged.

4.5 In `src/search/dfpn/selection.rs` (optional, while editing the file):
  - Rename `win_idx`/`loss_idx` in `is_solved_by_children` to
    `parent_win_child_idx` / `parent_loss_child_idx` (or add comments) to make
    the inverted semantics (a child `Loss` means a parent `Win`) explicit.

### 5. Example and test DRY

5.1 Examples:
  - Create `examples/common.rs` containing:
    - `pub const M19_FEN: &str = "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19";`
    - A small `pub fn parse_move(pos: &Position, from: &str, to: &str, promo: Option<&str>) -> Option<Move>` helper.
  - Update `find_winning_child.rs`, `play_and_solve.rs`, `solve_depth_limited.rs`,
    and `static_move_scores.rs` to use `M19_FEN` and the shared `parse_move`
    helper where appropriate.

5.2 Tests (optional but recommended):
  - Create `tests/common/mod.rs` with shared helpers such as:
    ```rust
    pub fn solve(fen: &str) -> Outcome;
    pub fn solve_with_timeout(fen: &str, secs: u64) -> Outcome;
    pub fn solve_refined(fen: &str) -> (Outcome, Vec<String>, u64);
    pub fn pv_strings(pv: &[Move]) -> Vec<String>;
    pub fn cli_bin() -> String;
    ```
  - Deduplicate the `solve`/`solve_refined`/`cli_bin` helpers from
    `test_inf.rs`, `test_plan1.rs`, `test_plan2.rs`, `test_plan4.rs`,
    `test_plan6.rs`, `test_epsilon.rs`, `test_review.rs`,
    `test_terminal_ordering.rs`, and `test_repetition.rs`.
  - If `tests/common/mod.rs` causes Cargo to create an unwanted `common`
    integration-test target, switch to a single `tests/common.rs` included via
    `mod common;` and verify it is not run as a test binary.

5.3 In `examples/play_and_solve.rs`:
  - Replace `parse_sq(...).unwrap()` and `find_move(...).unwrap()` with
    `.expect("...")` or `unwrap_or_else` to give clearer error messages.

### 6. Polishing pass for mechanical clippy warnings

6.1 While touching the files above, also fix the clearly mechanical pedantic
    warnings (do not change semantics):
  - `redundant_else` in `src/position.rs`.
  - `match_same_arms` and `unnested_or_patterns` in `src/position.rs` and
    `src/search/dfpn/core.rs`.
  - `unreadable_literal` in `src/zobrist.rs` and `src/search/dfpn/tests.rs`.
  - `uninlined_format_args` in examples and tests.
  - `doc_markdown` missing backticks around example names in example doc comments.

6.2 For lints that are noisy rather than helpful (e.g. `similar_names` on the
    paired `pn`/`dn` variables), add `#[allow(clippy::similar_names)]` at the
    function or module level rather than renaming well-understood pairs.

6.3 Add `#[must_use]` to pure query functions and methods where ignoring the
    result is a real bug (e.g. `Outcome::to_pn_dn`, `Outcome::pn_dn_for`,
    `Outcome::flip`, `Position::hash`, `Position::repetition_key`).
    Do not add `#[must_use]` to methods with side effects (`do_move`,
    `undo_move`, `legal_moves`).

## File changes

- `src/position.rs`
- `src/zobrist.rs`
- `src/notation.rs`
- `src/main.rs`
- `src/search/dfpn/mod.rs`
- `src/search/dfpn/core.rs`
- `src/search/dfpn/children.rs`
- `src/search/dfpn/selection.rs`
- `src/search/dfpn/pv.rs`
- `src/search/tt/entry.rs`
- `src/search/tt/table.rs`
- `examples/common.rs` (new)
- `examples/find_winning_child.rs`
- `examples/play_and_solve.rs`
- `examples/solve_depth_limited.rs`
- `examples/static_move_scores.rs`
- `tests/common/mod.rs` or `tests/common.rs` (new, optional)
- `tests/test_inf.rs`
- `tests/test_plan1.rs`
- `tests/test_plan2.rs`
- `tests/test_plan4.rs`
- `tests/test_plan6.rs`
- `tests/test_epsilon.rs`
- `tests/test_review.rs`
- `tests/test_terminal_ordering.rs`
- `tests/test_repetition.rs`

## Risks

- Visibility changes could break examples or integration tests if any code
  accesses `Search` fields, `TtEntry` fields, or `Position::zobrist` directly.
  The current examples/tests only use public methods, but verify with
  `cargo test --all-targets` and `cargo run --example <name>`.
- Removing `vpn`/`vdn` changes the `ChildInfo`/`ChildSelection` public shape
  within `src/search/dfpn` (even though the fields are module-private). Update
  every tuple destructuring and construction site.
- `TranspositionTable::probe` closure change is behavior-preserving, but verify
  the `find` still returns `Option<&TtEntry>` and that the entry is still `Copy`
  where explicit `.copied()` calls expect it.
- `validate_pv_prefix` returning `Option<Position>` changes its internal
  signature only; ensure `extract_pv_checked` still emits the same warnings.
- Adding `tests/common` helpers requires care so Cargo does not treat the shared
  file as an integration test target.
- Pedantic clippy fixes are mechanical but numerous. Apply them incrementally
  and run tests after each file to avoid mixing style changes with logic changes.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo test --release
$ cargo doc --no-deps
$ cargo run -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --example find_winning_child
$ cargo run --example play_and_solve
$ cargo run --example solve_depth_limited
$ cargo run --example solve_no_refinement
$ cargo run --example static_move_scores
$ cargo run --example twin_stats
```

Additional checks:

- All source files remain under ~10 KB.
- `cargo clippy --all-targets` has no warnings.
- `outcome:`/`pv:` output for decisive positions is unchanged.
- No `#[allow(clippy::...)]` is added to suppress correctness-related lints.

## Final task

Write `docs/plans/cleanup/report1.md` documenting which cleanups were applied,
which were intentionally skipped, the `cargo test` / `cargo clippy` results,
and any follow-up items (e.g. making `Position::board` fully private behind an
accessor).
