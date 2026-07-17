# Plan 5 Implementation Report

This report documents the implementation of `docs/plans/review/plan5.md`, which
replaces the linear depth refinement in `solve_refined` with a true binary
search.

## Changes made

### `src/search/dfpn.rs`

- Rewrote `solve_refined` to use binary search over `[1, best_depth]` instead
  of the previous `for mid in 1..best_depth` linear scan.
- The binary search loop:
  1. Computes `mid = (lo + hi) / 2`.
  2. Resets search state, clears the TT, and runs `dfpn(pos, INF, INF, mid, true)`.
  3. If the returned `Outcome` matches the full-depth outcome, the search has
     found a winning proof at a depth of at most `mid`, so `hi = mid` and the
     PV is extracted/recorded.
  4. Otherwise the depth bound is too low and `lo = mid + 1`.
- Added a final validation pass at the converged depth `lo`.  If the final
  `dfpn` call does not reproduce the original outcome (e.g. because of timeout
  noise or TT monotonicity issues), the saved full-depth PV is restored as a
  safe fallback.
- The refinement is skipped when:
  - the outcome is `Draw`,
  - `best_depth` is `1` (already a terminal or one-ply result), or
  - `best_depth` is `u32::MAX` (no useful depth was recorded).
- Added a new integration test `two_rook_mate_refinement_stays_short` in
  `tests/test_plan5.rs`.  It solves a transposition-heavy two-rook mate with
  `refine_shortest(true)` and asserts the returned PV is at most three plies
  long, confirming the binary search finds a short win rather than a longer
  accidental one.

## Monotonicity assumptions

The binary search relies on the monotonicity of `dfpn(max_depth)`:

- If `dfpn(d)` proves the same decisive outcome as the unbounded search, then
  any depth `d' >= d` also proves that outcome.
- If `dfpn(d)` returns `Draw` because the depth bound is too low, then any depth
  `d' <= d` also returns `Draw`.

`dfpn` resets the path and clears the TT at each refinement probe, so the
only source of non-monotonicity is the global timeout.  The loop breaks on
`time_exceeded()`, and the final validation step detects any mismatch with the
full-depth result, falling back to the original full-depth PV.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
```

All passed:

- `cargo clippy --all-targets` is clean.
- `cargo test` passes all tests, including the new `two_rook_mate_refinement_stays_short` test.
- `cargo doc` builds without warnings.

## Remaining concerns

- The final validation pass at `lo` runs one extra `dfpn` search.  For very
  deep wins this is still much cheaper than the old linear scan, but it could
  be merged with the last successful probe in a future optimization.
- `validate_pv` still only checks that the PV reaches a terminal position; it
  does not verify that each individual move is legal or that the final outcome
  matches the reported result.
- The monotonicity fallback has not been stress-tested on positions that time
  out during refinement; a larger benchmark could expose edge cases.
