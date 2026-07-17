# Plan 11: Fix `simulate` cross-path verification

## Start

- Read `docs/plans/review/report10.md` to confirm terminal detection is
  centralized before changing `simulate`.

## Goal

Allow `simulate` (or a pragmatic fallback) to verify twin proof/disproof trees
that were stored under a different path code, so cross-path GHI reuse is
effective.

## Background

- `simulate` is called by `try_use_tt` for a twin whose `path_code` differs from
  the current search path.
- It currently computes child path codes from the current `path_code` and probes
  the TT. The child entries in the twin's stored proof tree were generated with
  the twin's original path codes, so they are usually not found.
- At minimum, `simulate` must handle terminal no-legal-move nodes correctly
  (covered in plan 10).
- A pragmatic fallback is to run a bounded fresh `dfpn` under the current path
  when the TT child entry is not available.

## Implementation tasks

1. Ensure `simulate` uses the centralized terminal helper from plan 10 at its
   top-level check and in the `Outcome::Loss` empty-move branch.
2. Add a `path_length` field to `TwinEntry` in `src/search/tt.rs` and record it
   when a twin is stored. `dfpn` can compute this from
   `self.path_stack.len()` at store time (which corresponds to the node's
   depth/length).
3. Extend `simulate` to accept the starting `path_code` and `path_length` for
   the subtree it is verifying:
   - When verifying a twin, pass `twin.path_code` and `twin.path_length`.
   - When verifying the current path, pass `self.path_code` and
     `self.path_stack.len()`.
   - Use the `path_length` (incremented each ply) as the depth index for
     `zobrist::path_random`, instead of `sim_stack.len()`, so child path codes
     match the stored proof tree.
4. If the path-code-aware approach proves too complex, implement the pragmatic
   fallback in `try_use_tt`:
   - When a twin from another path is found, clone the position and run a
     node-bounded search (capped by `SIM_MAX_NODES`) under the current path and
     `max_depth = twin.depth`.
   - Accept the twin only if the bounded search returns the same `outcome`.
5. Bound simulation depth explicitly: pass a `remaining_depth` counter to
   `simulate` and decrement it on recursion, comparing against `SIM_MAX_DEPTH`
   instead of relying on `sim_stack.len()` (which tracks the current search
   prefix, not the simulation depth from the twin root).
6. Update existing unit tests that call `simulate` directly to supply the new
   parameters, and add a test for cross-path twin reuse:
   - Store a solved twin for one path code, then call `try_use_tt` with a
     different path code and verify the result is accepted.
7. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
8. Final task: write `docs/plans/review/report11.md` documenting the chosen
   approach (path-code aware or bounded fallback), test results, and any
   remaining limitations.

## File changes

- `src/search/dfpn.rs`
- `src/search/tt.rs`

## Risks

- Path-code arithmetic is subtle; a bug can cause `simulate` to accept an invalid
  result or to reject a valid one. Add unit tests for `zobrist::path_random`
  ordering before relying on it.
- A bounded fresh `dfpn` is correct but may be more expensive than pure
  simulation. Keep the node and depth caps tight.
- Changing `TwinEntry` layout affects `TtEntry` size and may impact TT memory.

## Verification

- `cargo test` passes, including `try_use_tt` and `simulate` unit tests.
- A manually constructed cross-path twin test accepts the correct outcome.
- `cargo run` on a cyclic position returns the same result before and after the
  change.
