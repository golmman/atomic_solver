# Plan 18 Implementation Report

## Summary

Implemented the file-size convention, split `src/search/dfpn.rs` and
`src/search/tt.rs`, removed dead code related to `print_pv_update` and the
`refine_shortest` guard, and made the PV extraction cap configurable with a
clear truncation warning.

## AGENTS.md updates

- Added a file-size convention under **Conventions**:

  > Keep source files under ~10 KB. Files larger than 10 KB must include a short
  > documented justification in the file header or in `AGENTS.md`. Files larger
  > than ~20 KB should normally be split into submodules.

- Updated the architecture bullets to point to `src/search/dfpn/` and
  `src/search/tt/` instead of the old monolithic files.

## `dfpn.rs` split

The old `src/search/dfpn.rs` (~55 KB) was replaced by `src/search/dfpn/`:

| File | Role | Size |
|------|------|------|
| `mod.rs` | `Search` struct and public API (`new`, `solve`, `search_depth`, `set_timeout`, `set_epsilon`, `set_max_ply`, etc.) | ~8.8 KB |
| `core.rs` | `dfpn` recursive routine, `epsilon_ceil`, `Resolved`, `outcome_from_pn_dn` | ~9.4 KB |
| `children.rs` | `ChildInfo`, `ChildSelection`, `select_children`, `evaluate_child` | ~6.1 KB |
| `selection.rs` | `is_solved_by_children`, `best_and_second_unsolved` and their unit tests | ~7.6 KB |
| `pv.rs` | `extract_pv`, `extract_pv_checked`, `validate_pv`, and PV-related tests | ~7.6 KB |
| `simulate.rs` | `try_use_tt` and Kawano-style `simulate` | ~8.4 KB |
| `history.rs` | `sort_moves`, `update_history`, `update_killers`, `maybe_age_history` | ~2.9 KB |
| `tests.rs` | Cross-module DF-PN tests (simulation, twin lookup) | ~5.2 KB |

The public API is unchanged: `Search`, `INF`, and `outcome_from_pn_dn` are still
available through `atomic_solver::search::dfpn`.

## `tt.rs` split

The old `src/search/tt.rs` (~16 KB) was replaced by `src/search/tt/`:

| File | Role | Size |
|------|------|------|
| `mod.rs` | Re-exports `TranspositionTable`, `TtEntry`, `TwinEntry`, `EntryResult` | <1 KB |
| `entry.rs` | `TwinEntry`, `TtEntry`, `EntryResult`, `TwinAction`, `MAX_TWINS` | ~5.0 KB |
| `table.rs` | `TranspositionTable` and its `impl` | ~7.7 KB |
| `tests.rs` | Unit tests for the transposition table | ~3.3 KB |

All source files in the repository are now under 10 KB without needing
justification.

## Dead code removed

- Deleted `fn print_pv_update` (which also wrote to `last_pv`).
- Deleted `fn should_print_update` (only used by the removed print path).
- Removed the `last_printed` variable from the `dfpn` loop.
- Removed the `refine_shortest` guard that printed intermediate PV updates when
  `self.path_stack.len() == 1`.
- Removed `refine_shortest` from the threshold-break logic inside `dfpn`;
  refinement is now handled entirely by `solve_refined`.
- Kept `Search::refine_shortest` as the public switch and kept `last_pv`
  because `solve_refined` sets it directly.

## Configurable PV cap with warnings

- Added `pub(crate) const DEFAULT_MAX_PV_PLIES: usize = 1000` in `dfpn/mod.rs`.
- Added a `max_ply: usize` field to `Search`, defaulting to
  `DEFAULT_MAX_PV_PLIES`.
- Added `pub fn set_max_ply(&mut self, max_ply: usize)` (clamped to at least 1).
- `extract_pv` now iterates `for _ in 0..self.max_ply` instead of a hard-coded
  `1000`.
- `extract_pv_checked` detects a truncated PV, validates that every move in the
  partial line is legal, and returns the partial PV with:

  ```text
  eprintln!("warning: PV truncated after {} plies", self.max_ply);
  ```

- A truncated PV is still returned with the correct `Outcome`; the cap is a
  display safety, not a proof cutoff.
- `try_use_tt` now seeds simulations with `self.max_ply.max(SIM_MAX_DEPTH)` so
  that a larger PV cap does not cause cross-path twins to be rejected because
  the simulation depth cap is too small. `SIM_MAX_NODES` remains the hard
  backstop.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo test --release
$ cargo doc --no-deps
```

All passed.

CLI sanity checks:

```text
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
outcome: win
pv: f1f7 e8d8 g1g8

$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
outcome: win
pv: a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6
```

A new unit test, `pv::tests::pv_truncation_warns_and_keeps_outcome`, solves the
first position with `max_ply = 2`, asserts the outcome is still `Win`, and that
the returned PV is exactly two plies long.

## Follow-up items

- The `SIM_MAX_DEPTH` constant is now respected relative to the configured PV cap
  (`self.max_ply.max(SIM_MAX_DEPTH)`). If longer or shorter defaults become
  desirable, they can be tuned independently without changing the solver core.
