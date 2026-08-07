# Plan 1: Strengthen the test suite

## Start

1. Read `AGENTS.md` and this plan.
2. Run the baseline suites and capture the current state of ignored tests:
   ```bash
   cargo test
   cargo test --release -- --ignored
   ```
   The second run is the source of truth for which `#[ignore]`d tests are
   genuinely too slow/broken and which ones are simply stale.

## Goal

Make regressions harder to introduce by:

* improving the quality and coverage of existing tests,
* auditing and fixing `#[ignore]`d tests,
* removing stale or duplicate tests while keeping legitimately ignored ones,
* adding unit and integration tests where the source code is currently
  untested, and
* adding small diagnostic/regression examples that make it easier to debug
  solver behaviour.

## Background

* The project currently has **47 unit tests** in `src/` and **~75
  integration/regression tests** under `tests/`. `cargo test` passes in debug.
* **27 integration tests are ignored** (`test_ghi.rs` 2, `test_epsilon.rs` 2,
  `test_review.rs` 2 via `#[cfg_attr(debug_assertions, ignore = ...)]`, and
  `test_plan6.rs` 18 unconditional `#[ignore]`s).
* `cargo test --release -- --ignored` shows that many of those ignores are
  stale or wrong:
  * `test_ghi.rs` cyclic-GHI tests pass in release in ~5 s.
  * Several `test_plan6.rs` ignored positions (`m23`, `m24-white`, `m25b-black`,
    `m26`, `m27-white/black`, `m29-black`) are proven within the 5 s release
    budget and should not be ignored.
  * `m24-black`, `m25a-white` and `m25a-black` contain invalid FENs
    (`p6Pp` on rank 4 is nine squares).
  * `m27_kh7_fast_win` parses and passes in release despite being ignored as
    "invalid FEN"; the `c`/`C` pieces need to be audited.
  * `m19`–`m22` return `Draw` within the 5 s release timeout and need to be
    classified as either "needs longer timeout" or "stress tests".
* Several files duplicate coverage:
  * `test_plan4.rs` and `test_plan5.rs` test the same m28-white position.
  * `test_plan5.rs` `black_root_report4_fen` and `test_plan6.rs`
    `black_root_report6_fen` are identical.
  * `test_plan6.rs` `m27_black_loses` and `m28_white_wins` duplicate
    `test_plan5.rs`.
  * `test_plan1.rs` duplicates `src/position.rs` unit tests except for one
    `outcome_from_pn_dn` test.
  * `test_terminal_ordering.rs` duplicates `src/position.rs` terminal tests.
* A number of source modules have **no unit tests at all**:
  `src/notation.rs`, `src/zobrist.rs`, `src/search/ordering.rs`,
  `src/search/dfpn/history.rs`, and `src/search/tt/table.rs`.
* `README.md` lists `twin_stats` as an example, but the file does not exist;
  `inspect_pt.rs` and `replay.rs` exist but are not documented.

## Implementation tasks

### 1. Audit and fix `#[ignore]`d tests

1.1 Convert `tests/test_ghi.rs` ignored cyclic tests to
`#[cfg_attr(debug_assertions, ignore = "slow cyclic GHI regression")]` so they
run in release CI but not in slow debug builds. They already pass in release.

1.2 Fix the `test_plan6.rs` FENs that currently fail with
`InvalidPlacement("piece 'p' at rank 5 is placed past column 8")`.
The problem is `p6Pp` on rank 4 in:

* `m24_black_loses` <ref_snippet file="/workspace/atomic_solver/tests/test_plan6.rs" lines="109-116" />
* `m25a_white_wins` <ref_snippet file="/workspace/atomic_solver/tests/test_plan6.rs" lines="118-125" />
* `m25a_black_loses` <ref_snippet file="/workspace/atomic_solver/tests/test_plan6.rs" lines="127-134" />

Change `p6Pp` to `p5Pp` and verify that the resulting position is the intended
one by replaying the expected line.

1.3 Investigate `m27_kh7_fast_win` <ref_snippet file="/workspace/atomic_solver/tests/test_plan6.rs" lines="189-203" />.
It parses and passes in release, so the ignore reason is stale. Decide:

* If `c`/`C` are intentional custom commoner pieces, rename the test to reflect
  that and remove the `#[ignore]`.
* If `c`/`C` are typos for `k`/`K`, fix the FEN.
* If the position is not meaningful for the supported rules, remove the test.

1.4 For `m19`–`m22` <ref_snippet file="/workspace/atomic_solver/tests/test_plan6.rs" lines="19-116" />,
run each in release with `--timeout 60`. Positions that become decisive can be
enabled with `#[cfg_attr(debug_assertions, ignore = "...")]` and a 60 s timeout;
positions that still cannot be proven belong in a separate stress suite (see
Section 4).

1.5 `m24_solve_with_pv` <ref_snippet file="/workspace/atomic_solver/tests/test_plan6.rs" lines="246-269" />
should use `#[cfg_attr(debug_assertions, ignore = "60 second stress test")]`
instead of an unconditional `#[ignore]`, because it passes in release.

1.6 Establish a policy: every remaining `#[ignore]` must have an attribute
*and* an inline comment explaining why, plus the command needed to run it.

### 2. Remove stale and duplicate tests

2.1 Merge `tests/test_plan4.rs` and `tests/test_plan5.rs`. They test the same
m28-white FEN:

* <ref_snippet file="/workspace/atomic_solver/tests/test_plan4.rs" lines="6-22" />
* <ref_snippet file="/workspace/atomic_solver/tests/test_plan5.rs" lines="23-37" />

Keep the stronger assertions (first-move and PV length) in `test_plan5.rs` and
delete `test_plan4.rs`.

2.2 Remove exact duplicates in `tests/test_plan6.rs`:

* `black_root_report6_fen` duplicates `test_plan5.rs` `black_root_report4_fen`.
* `m27_black_loses` is the same FEN as the black-root test above.
* `m28_white_wins` duplicates the merged m28-white test.

2.3 Reduce `tests/test_plan1.rs` to the single test that is not already
unit-tested in `src/position.rs` (`outcome_from_pn_dn_only_recognizes_win`) and
move it into `src/search/dfpn/core.rs` as a unit test. Delete
`tests/test_plan1.rs`.

2.4 Merge `tests/test_terminal_ordering.rs` into `tests/test_position.rs` or
`src/position.rs` unit tests. The three cases are already covered by
`position::tests`, but exercising them through `Search::solve` is useful; do it
once, not twice.

2.5 Keep `tests/test_ghi.rs` active `promotion_transposition_outcome_is_consistent`
test; move the ignored cyclic-shuffle test into `tests/test_repetition.rs` with a
release-only `#[cfg_attr]` ignore.

### 3. Improve existing tests

3.1 Replace `matches!(solve(...), Outcome::Win)` assertions in `tests/test_inf.rs`
and `tests/test_plan2.rs` with `assert_eq!(..., Outcome::Win)` so failures print
both the actual and expected outcome.

3.2 For every solver test that returns a decisive `Outcome`, also assert that:

* the PV is non-empty (unless the result is `Draw`),
* `Search::validate_pv(&pv, &pos, expected_outcome, None)` is true,
* the first PV move is legal in the starting position.

Add helpers for this in `tests/common/mod.rs`.

3.3 Clean up `tests/common/mod.rs` <ref_file file="/workspace/atomic_solver/tests/common/mod.rs" />:

* Remove the `solve_refined` alias (it is identical to `solve_with_pv`).
* Add `assert_solves_to(fen, expected, max_pv_len)` and
  `assert_solves_with_first_move(fen, expected, first_uci)` helpers.
* Add `assert_pv_valid(fen, expected, pv)` helper.
* Ensure all helpers validate FENs and panic with useful messages.

3.4 Add context messages to all remaining bare `assert!(...)` and
`assert_eq!(...)` statements.

### 4. Add missing tests

4.1 `src/notation.rs` <ref_file file="/workspace/atomic_solver/src/notation.rs" />:
round-trip `uci_to_move(move_to_uci(m)) == m` for every legal move in a few
positions; `uci_to_move` returns `None` for illegal/non-existent UCI strings;
promotion, castling and en-passant UCI parsing.

4.2 `src/zobrist.rs` <ref_file file="/workspace/atomic_solver/src/zobrist.rs" />:
`rule50_key` values are distinct for `rule50` 0..100; `hash` changes with
`rule50` while `board_hash` ignores it; keys are deterministic across runs.

4.3 `src/search/ordering.rs` <ref_file file="/workspace/atomic_solver/src/search/ordering.rs" />:

* `nearest_commoner_map` returns correct Chebyshev distances and `i8::MAX` when
  the opponent has no commoner.
* `StaticAtomicScorer` ordering: winning capture > promotion > capture > threat
  > center/approach.
* `score` and `score_with_map` agree for the same position.

4.4 `src/search/dfpn/history.rs` <ref_file file="/workspace/atomic_solver/src/search/dfpn/history.rs" />:
history bonuses cap at `HISTORY_MAX`; killer slots shift correctly; aging
halves all entries; `killer_bonus` returns the configured score only for stored
killers.

4.5 `src/search/dfpn/children.rs` <ref_file file="/workspace/atomic_solver/src/search/dfpn/children.rs" />:
`evaluate_all_children` stops evaluating once a winning child is found;
`evaluate_child` produces the right `pn`/`dn`/`outcome` for terminal,
repetition, solved TT and unsolved TT children; the `explored` flag is set when
a bounded child cannot make progress.

4.6 `src/search/dfpn/pv.rs` <ref_file file="/workspace/atomic_solver/src/search/dfpn/pv.rs" />:
`extract_pv` follows the TT to the terminal position; `extract_pv_checked`
returns `None` on validation failure and warns correctly; `emit_proof_tree`
creates a tree whose `validate_ppv` accepts the returned PV.

4.7 `src/search/tt/table.rs` <ref_file file="/workspace/atomic_solver/src/search/tt/table.rs" /> and
`src/search/tt/entry.rs` <ref_file file="/workspace/atomic_solver/src/search/tt/entry.rs" />:
`probe`/`probe_summary`/`probe_best_move` for missing and stored keys;
unsolved `(INF, INF)` bounds are stored as `(1, 1)`; solved results overwrite
unsolved bounds and vice versa; `new_generation` makes old entries stale;
eviction prefers live, solved, high-work entries.

4.8 `src/proof_tree/mod.rs` <ref_file file="/workspace/atomic_solver/src/proof_tree/mod.rs" />:
`add_node` reconstructs paths; `extract_ppv` and `validate_ppv` handle win,
loss and draw subtrees; `estimate_memory` triggers the `memory_limited` flag at
small non-zero budgets; `process_pending` correctly attaches out-of-order events.

4.9 `src/search/dfpn/mod.rs` <ref_file file="/workspace/atomic_solver/src/search/dfpn/mod.rs" />:
`set_epsilon` rejects out-of-range values; `set_timeout(0)` causes an immediate
timeout; `first_outcome_only` skips refinement; `exit_reason` reports `Timeout`,
`Quit`, `MemoryLimit` and `Complete` correctly; `solve_with_progress` calls the
closure at least once for a decisive result.

4.10 Integration CLI tests in a new `tests/test_cli.rs`:
`--help` exits 0; unknown options exit 1 with a clear error; `--outcome-only`
does not hang on stdin and prints only outcome/PV; `--first-outcome` returns a
shorter or equal PV; `--dump-path` writes a valid `proof_tree.bin`; invalid
`--epsilon`/`--tt-size`/`--timeout` values produce clear errors.

### 5. Add diagnostic/regression examples

5.1 Create `examples/twin_stats.rs` (the README <ref_snippet file="/workspace/atomic_solver/README.md" lines="119-130" /> lists it but it does not exist).
It should load a FEN, run a solve, and print transposition-table statistics:
number of buckets, live entries, solved entries, unsolved bounds, generation,
and path-code distribution. This is invaluable for GHI debugging.

5.2 Create `examples/move_order_debug.rs` that prints, for each legal move in a
position, the static score, the history contribution, the killer contribution,
and the final sorted order. This helps debug why the solver chooses one first
move over another.

5.3 Create `examples/list_legal.rs` that simply prints all legal UCI moves and
the terminal outcome for a FEN. Useful for quickly validating that a position is
parsed as expected.

5.4 Update `README.md` so the example list matches the actual `examples/`
directory, including `inspect_pt.rs` <ref_file file="/workspace/atomic_solver/examples/inspect_pt.rs" />,
`replay.rs` <ref_file file="/workspace/atomic_solver/examples/replay.rs" />, and
`chunk_growth.rs` <ref_file file="/workspace/atomic_solver/examples/chunk_growth.rs" />.

### 6. Reorganise long-running regression tests

6.1 Move release-only or CI-stress tests that take more than 5 s in debug into a
dedicated `tests/stress.rs` file. Use `#[cfg_attr(debug_assertions, ignore = "slow stress test")]`
so they still run in release builds but do not slow down normal `cargo test`.

6.2 Keep the fast `test_plan6.rs` positions in place. After the deduplication and
FEN fixes, the remaining slow positions should be clearly documented.

6.3 Add a `tests/fixtures/positions.txt` corpus with lines like:

```text
fen;expected;max_pv_plies
4k3/8/8/8/8/8/8/4R1K1 w - - 0 1;Win;3
```

Add a single `tests/test_corpus.rs` that reads the file at compile time with
`include_str!` and asserts each entry. This makes adding new regression FENs a
one-line change.

## File changes

* `tests/test_ghi.rs`
* `tests/test_plan6.rs`
* `tests/test_plan5.rs`
* `tests/test_plan4.rs` (delete)
* `tests/test_plan1.rs` (delete)
* `tests/test_terminal_ordering.rs` (delete or merge)
* `tests/test_position.rs`
* `tests/common/mod.rs`
* `src/notation.rs`
* `src/zobrist.rs`
* `src/search/ordering.rs`
* `src/search/dfpn/history.rs`
* `src/search/dfpn/children.rs`
* `src/search/dfpn/pv.rs`
* `src/search/dfpn/mod.rs`
* `src/search/tt/table.rs`
* `src/search/tt/entry.rs`
* `src/proof_tree/mod.rs`
* `tests/test_cli.rs` (new)
* `tests/stress.rs` (new)
* `tests/test_corpus.rs` (new)
* `tests/fixtures/positions.txt` (new)
* `examples/twin_stats.rs` (new)
* `examples/move_order_debug.rs` (new)
* `examples/list_legal.rs` (new)
* `README.md`

## Risks

* Changing `#[ignore]`d tests may expose real solver bugs or broken FENs. Treat
  failures as data, not blockers.
* Removing duplicate tests could accidentally drop slightly different
  assertions; verify each removed test is fully covered elsewhere.
* Adding many new tests increases CI time; mitigate by keeping slow tests
  release-only or ignored.
* New corpus/fixtures add a data dependency; use `include_str!` so tests still
  work offline.

## Verification

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo test --release -- --ignored
cargo test --all-targets
```

Also run:

```bash
cargo run --example list_legal -- --fen startpos
cargo run --example twin_stats -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
cargo run --example move_order_debug -- --fen "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19"
```

Ensure no `#[ignore]` remains without a documented reason.

## Final task

Write `docs/plans/testability/report1.md` documenting:

* which `#[ignore]`d tests were enabled, fixed, moved or removed and why,
* which duplicate tests were merged,
* the new unit/integration test coverage map,
* the new examples and their usage,
* any positions that remain unsolved and the plan to re-enable them.
