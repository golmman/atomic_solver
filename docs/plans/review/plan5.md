# Plan 5: Make `solve_refined` a true binary search

## Start

- Read `docs/plans/review/report4.md` to confirm twin-capacity work is done and
  note any blockers before touching shortest-PV refinement.

## Goal

Replace the linear `for mid in 1..best_depth` scan in `solve_refined` with a
proper binary search, or update the comment to match the implementation. This
plan implements binary search because it materially reduces refinement time for
deep wins.

## Background

`solve_refined` first finds the full outcome with no depth limit, records
`best_depth`, then scans `mid` from 1 upward. For deep wins this is
`O(best_depth)`. The search is monotonic in `max_depth`: if depth `mid` proves
 the same outcome, any larger depth also proves it; if it does not, a smaller
 depth also does not.

## Implementation tasks

1. Verify the monotonicity assumption holds for `dfpn` with `max_depth` bounds
   (timeouts and TT effects can introduce noise; test on a few positions).
2. Replace the linear loop with binary search over `[1, best_depth]`:
   - `lo = 1`, `hi = best_depth`
   - while `lo < hi`:
     - `mid = (lo + hi) / 2`
     - reset search state, clear the TT, run `dfpn(pos, INF, INF, mid, true)`
     - if `o == outcome` then `hi = mid` else `lo = mid + 1`
   - Keep the `extract_pv_checked` update inside the success branch.
3. Keep a fallback linear loop (or a comment) in case `best_depth == u32::MAX`
   (unsolved/draw) or binary search returns an inconsistent result; the
   fallback should simply stop refining.
4. Add a test with a position known to have a deep win, if one exists in the
   test suite, and assert refinement finishes faster than a linear scan (or at
   least that it produces the same PV/outcome).
5. Run `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, `cargo doc`.
6. Final task: write `docs/plans/review/report5.md` documenting the binary
   search implementation and any monotonicity caveats.

## File changes

- `src/search/dfpn.rs`

## Risks

- Binary search assumes `dfpn(max_depth)` is monotonic. If `max_depth` affects
  TT contents or move ordering in a non-monotonic way, the binary search may
  fail; add an assertion/validation and fall back to the original depth.
- `best_depth` may be `u32::MAX` for draws or timeouts; guard against that.

## Verification

- Existing tests pass.
- `cargo run -- --fen <deep-win-FEN>` still prints a correct PV and refinement
  behaves correctly.
