# Plan 3 Implementation Report

This report documents the implementation of `docs/plans/review/plan3.md`, which
hardens the GHI twin-simulation logic in `src/search/dfpn.rs` to verify twins
under the current search prefix and path code.

## Changes made

### `src/search/dfpn.rs`

- `try_use_tt` now seeds simulation with the current search prefix instead of an
  empty set:
  - `sim_path` is initialized as a clone of `self.path`.
  - `sim_stack` is initialized as a clone of `self.path_stack`.
  - The starting `path_code` passed to `simulate` is `self.path_code` (the current
    node's path code) rather than `twin.path_code`.
- `simulate` now treats a repeated position as a draw:
  - If `sim_path` already contains the position being entered, `simulate` returns
    `expected == Outcome::Draw`.  This is correct because a position that repeats
    an ancestor in the real search tree is a draw by repetition.
- The `Outcome::Loss` branch of `simulate` no longer accepts an empty move list
  as a valid loss.  When `moves.is_empty()` it returns `pos.outcome() == Some(expected)`,
  which is `true` only when `Position::outcome` reports a terminal `Loss` (e.g.
  the side to move has no commoners).  Stalemate positions no longer pass through
  as a `Loss`.
- Added unit tests in the existing `#[cfg(test)] mod tests` block:
  - `simulate_repeated_position_is_draw_only` — a position already in the
    simulation path is only accepted as `Draw`.
  - `simulate_loss_branch_rejects_stalemate` — a stalemate position is not
    accepted as `Outcome::Loss`.
  - `try_use_tt_simulation_uses_current_path` — a `Draw` twin from another path
    is accepted when the current search prefix already contains the position.
  - `try_use_tt_rejects_win_twin_for_repeated_position` — a `Win` twin from
    another path is rejected when the current prefix makes the real outcome a
    draw by repetition.

## How the simulation now verifies twins

`try_use_tt` performs Kawano-style simulation for a twin that was stored for a
different path code.  Previously the simulation started with an empty
`sim_path`/`sim_stack` and recomputed path codes from the twin's original path.
It therefore could not tell whether a repetition used by the twin's proof also
existed in the current search prefix.

With the changes:

1. The simulation starts with the current search prefix already in `sim_path` and
   `sim_stack`.
2. All path codes are computed relative to the current node's `path_code` using
   `path_code ^ zobrist::path_random(mv, sim_stack.len())`.
3. If the simulation reaches a position that is already in `sim_path`, it is a
   repetition in the current tree and can only be consistent with `Outcome::Draw`.
4. Because the TT is probed with these current-path codes, a twin whose cached
   children live under a different path code will not match and the simulation
   will fail, preventing reuse of an incompatible twin.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
```

All passed:

- `cargo clippy --all-targets` is clean.
- `cargo test` passes all tests, including the 4 new `dfpn` unit tests.
- `cargo doc` builds without warnings.

## Remaining concerns

Plan 3 focused on the simulation path-prefix issue.  Several items from
`review1.md` are still open:

- `solve_refined` is still a linear scan, not a true binary search.
- `validate_pv` still does not verify move legality or that the final outcome
  matches the reported result.
- `MAX_TWINS = 2` may still be too small for heavily cyclic search graphs.

These are tracked for future work.
