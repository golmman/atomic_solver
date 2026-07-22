# Plan 1: Early exit on a proven winning child

## Goal

Reduce unnecessary child evaluations when the current player already has a move that forces a win. Once a winning child is proven, the parent's outcome is decided unless we are explicitly refining for the shortest PV.

## Background

`ultimattt` stops generating/evaluating siblings as soon as a child is a proven win for the side to move. `atomic_solver` currently builds the full `Vec<ChildInfo>` in `evaluate_all_children` and then computes `pn`/`dn` over every child in `select_from_children`. For a position where one move is immediately decisive, this is wasted work.

The same logic already exists for the proof-PV refinement path in `src/search/dfpn/core.rs` (it keeps the shortest winning child when `refine_shortest` is true and breaks otherwise), so the early-exit change must be made compatible with `refine_shortest`.

## Files to modify

- `src/search/dfpn/children.rs`
- `src/search/dfpn/selection.rs`
- `src/search/dfpn/core.rs` (minor, for `refine_shortest` plumbing if needed)

## Concrete changes

1. Add a `Search::select_child_with_early_exit` helper or extend `select_from_children` to accept a `refine_shortest` flag.
2. When `is_solved_by_children` detects a `Win` for the parent:
   - If `refine_shortest == false`, return a `ChildSelection` whose `solved_outcome` is `Win`, `pn = 0`, `dn = INF`, `best_move` set to the winning child, and `all_solved` left as the current all-solved state. Do not evaluate remaining siblings.
   - If `refine_shortest == true`, still compute over all *solved* winning children to keep the shortest one, but skip unsolved siblings (they cannot improve on a proven win).
3. In `evaluate_all_children`, stop generating children as soon as a `Win` child is found and `refine_shortest` is false. Return what has been evaluated plus an `early_win` marker, or set the remaining un-evaluated children to have `pn = INF, dn = 0` so they do not affect the parent bounds.

## Verification

- Run `cargo test` and `cargo test --release`.
- Run `cargo run --release --example benchmark` and compare node counts to the baseline.
- Verify `fen1` and `fen2` still return the same outcomes and PVs.

## Risks / notes

- `refine_shortest` must remain able to collect all winning children in order to pick the shortest. The early exit must not break the SPPV refinement stage.
- `Outcome::Loss` from the child's perspective is a win for the parent; `is_solved_by_children` already treats this case as `Outcome::Win` for the parent, so the existing `ChildInfo.outcome` encoding can be reused.
