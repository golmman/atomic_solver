# Plan 17 Implementation Report

## Summary

Completed the remaining follow-up work from `review3.md`:

1. Hardened `solve_refined` shortest-PV refinement on transposition-heavy wins.
2. Documented GHI simulation limitations and added regression/unit tests for
   cross-path twin verification.
3. Removed duplicate CLI `outcome:`/`pv:` output and confirmed the public API
   state.

## Shortest-PV refinement

### What changed

- `Search::solve` bootstrap (`src/search/dfpn.rs`) now temporarily disables
  `refine_shortest` while doubling the depth budget.  Bootstrapping only needs a
  decisive result, so running a full shortest-PV search at each doubled depth
  was unnecessary and caused large node blow-up on positions like the
  promotion transposition.
- `Search::solve_refined` now resets the search state, clears the transposition
  table, and resets the history and killer tables before every depth-bounded
  probe.  Stale heuristic data from the bootstrap or previous binary-search
  probes was sometimes misdirecting the depth-bounded searches and producing
  much larger search trees than a fresh search.
- `solve_refined` no longer prints intermediate PV updates.  It sets
  `self.last_pv` directly and lets `main.rs` print the final `outcome:`/`pv:`
  block, eliminating duplicate final output from the CLI.

### New regression tests

Added to `tests/test_review.rs`:

- `two_rook_shortest_pv_is_three_plies` for
  `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1`.
- `promotion_shortest_pv_is_seven_plies` for
  `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1`.
- `epsilon_mate_shortest_pv_is_five_plies` for the mate-in-two position from
  `tests/test_epsilon.rs`.

These call `solve` with `refine_shortest(true)` and assert the returned PV
lengths are 3, 7, and 5 plies respectively.  The promotion and epsilon tests
are slow in debug builds, so they are ignored under `#[cfg(debug_assertions)]`.

### Verification on the three positions

- Two-rook mate: found in 138 nodes, PV `f1f7 e8d8 g1g8` (3 plies).
- Promotion transposition: refined to the 7-ply win
  `a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6`.
- Epsilon mate: refined to the 5-ply win
  `h5d5 d7d6 d5f7 e8d7 f7e7`.

## GHI simulation

### What changed

- Added a synthetic unit test in `src/search/dfpn.rs`:
  `try_use_tt_rejects_cross_path_win_twin_without_child_proof`.  It stores a
  `Win` twin and a child `Win` twin for one path, then calls `try_use_tt` from a
  different path where the child twin is missing.  The test confirms the cross-
  path win twin is rejected because `simulate` cannot follow the stored proof
  tree.
- Documented the current GHI simulation limitations in
  `docs/plans/dfpn/research_ghi.md` (section 9) and `docs/plans/review/review3.md`
  (section 8).  The solver does not implement full Kawano cross-path ancestor-set
  tracking, and it does not fall back to a bounded fresh `dfpn` when simulation
  fails.
- Added an integration placeholder in `tests/test_ghi.rs` for a concrete atomic-
  chess cross-path repetition-dependent win.

### Remaining limitations

Cross-path wins that depend on a repetition right available only in the twin's
original path are still not handled by the current pragmatic simulation.  The
placeholder test in `tests/test_ghi.rs` is ignored until such a position is found.

## Cleanup and API polish

### What changed

- Removed the final `print_pv_update` calls from `solve_refined`; only the CLI
  (`src/main.rs`) prints the final `outcome:`/`pv:` block.  Intermediate
  progress output is no longer emitted.
- Added `cli_does_not_duplicate_final_output` in `tests/test_review.rs`, which
  runs the release binary on the two-rook mate FEN and asserts that stdout
  contains exactly one `outcome:` block and one `pv:` block.
- Confirmed `Position::outcome_from_state` is already public.  No example
  binaries precompute a `MoveList` + `StateInfo`, so no example updates were
  required.

## Verification results

```text
$ cargo fmt                    # passed
$ cargo clippy --all-targets   # passed
$ cargo doc --no-deps          # passed
$ cargo test --release         # passed
$ cargo test --all-targets     # passed
$ cargo test --release --test test_ghi -- --ignored   # passed
$ cargo test --release --test test_epsilon             # passed
```

CLI output for the two transposition-heavy positions now shows a single
`outcome:`/`pv:` block each:

```text
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
outcome: win
pv: f1f7 e8d8 g1g8

$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
outcome: win
pv: a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6
```

## Remaining recommendations

- Implement a bounded fresh `dfpn` fallback in `try_use_tt` for cross-path twins
  that cannot be verified by simulation (Option A from `plan17.md`).  This would
  strengthen GHI correctness without the full ancestor-set bookkeeping of the
  paper.
- Continue searching for a concrete atomic-chess cross-path repetition-dependent
  win to replace the ignored placeholder in `tests/test_ghi.rs`.
- The shortest-PV refinement is now correct on the tested transposition-heavy
  wins, but hard positions can still consume most of the default 5-second budget
  during the depth-bounded binary search; keep an eye on node counts if the
  timeout is reduced further.
