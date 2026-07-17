# Plan 3: Harden GHI twin simulation against the current search path

## Start

- Read `docs/plans/review/report2.md` to confirm the path-code encoding is in
  place and to note any blockers before changing the simulation logic.

## Goal

Make `simulate` verify a twin's proof/disproof under the current search prefix
and current path code, so a twin from another path is not reused unless it is
also valid along the current path. Also harden the `Outcome::Loss` branch so an
empty move list is not accepted as a valid loss.

## Background

`try_use_tt` calls `simulate` with `twin.path_code` and a fresh `sim_path` set.
The simulation recomputes child path codes from the twin's path and does not
include `self.path`, so repetitions that existed in the twin's path may not exist
in the current path.

## Implementation tasks

1. Change `simulate`'s signature so it accepts the current prefix:
   - `sim_path: &HashSet<u64>` initial state (clone of `self.path`)
   - `sim_stack: &Vec<u64>` initial state (clone of `self.path_stack`)
   - `path_code: u64` starting at the current node's `self.path_code`
2. In `try_use_tt`, before calling `simulate`, create `sim_path` as a clone of
   `self.path` and `sim_stack` as a clone of `self.path_stack`. Pass
   `self.path_code` (the path code of the current node in the current search)
   instead of `twin.path_code`.
3. Inside `simulate`, compute child path codes as
   `path_code ^ zobrist::path_random(mv, sim_stack.len())`. Use the existing
   depth-key logic.
4. When probing the TT inside `simulate`, look for results matching the new child
   path code (current path + continuation). A child position already in
   `sim_path` is a repetition in the current prefix and should be treated as
   `Outcome::Draw` for that branch.
5. In the `Outcome::Loss` branch of `simulate`, after generating legal moves, if
   `moves.is_empty()` return `pos.outcome() == Some(expected)` (which will be
   `true` only if the position is genuinely terminal `Loss`, e.g. no commoners).
6. Add unit/integration tests that stress twin reuse across different paths and
   ensure a twin whose proof relies on a repetition not present in the current
   path is rejected.
7. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
8. Final task: write `docs/plans/review/report3.md` documenting the simulation
   changes and test results.

## File changes

- `src/search/dfpn.rs`

## Risks

- Passing full path clones adds per-twin-verification memory cost. Monitor with a
  few repetition-heavy positions.
- The simulation node budget (`SIM_MAX_NODES`) may need adjustment if
  verification now fails more often and falls back to re-search.

## Verification

- Existing tests still pass.
- New tests demonstrate that a twin from an incompatible path is not reused.
