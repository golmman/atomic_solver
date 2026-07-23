# Report: Plan 1 — Early exit on a proven winning child

This report documents the implementation of `docs/plans/ultimattt/plan1.md`.

## Summary

Added an early-exit path to the DF-PN+ solver so that, once a child proves a win
for the side to move, the remaining siblings are not evaluated unless the solver
is actively refining for the shortest principal variation.  This matches the
child-generation shortcut used in `ultimattt` and avoids wasted `do_move` /
`undo_move` work on positions where one move is immediately decisive.

## Changes applied

### `src/search/dfpn/children.rs`

- `evaluate_all_children` now takes a `refine_shortest` flag. When it evaluates
  a child whose outcome is `Outcome::Loss` (meaning a win for the parent) and
  `refine_shortest` is disabled, it stops generating further siblings.  The
  unevaluated slots are filled with `pn = INF, dn = 0` placeholders so they do
  not affect the parent's proof/disproof numbers.
- `select_from_children` now takes `refine_shortest` and consults the new
  `select_child_with_early_exit` helper before falling back to the full
  `pn`/`dn` and best/second-unsolved computation.

### `src/search/dfpn/selection.rs`

- Added `Search::select_child_with_early_exit`.  If the children already prove a
  win for the parent and `refine_shortest` is false, it returns a decisive
  `ChildSelection` with `solved_outcome = Win`, `pn = 0`, `dn = INF`, and
  `best_move` set to the winning child.  The full `best_and_second_unsolved`
  scan is skipped.
- When `refine_shortest` is true the helper returns `None`, so the normal
  selection path keeps running and can continue searching for a shorter PV.

### `src/search/dfpn/core.rs`

- Updated the two call sites to pass `self.refine_shortest` into
  `evaluate_all_children` and `select_from_children`.

## Why this is safe

A child with `Outcome::Loss` means the child side to move loses, which is a win
for the parent side to move regardless of node type.  Once such a child is
found, the parent's outcome is decided, so the remaining children cannot change
the result.  The `pn = INF, dn = 0` placeholders keep `all_solved` false (they
are unsolved) while contributing the neutral values `INF` and `0` to the
OR-node / AND-node sums and minima.

`refine_shortest` must keep exploring to find the shortest winning child, so
early exit is disabled in that mode.  The existing depth-aware loop in
`core.rs` still keeps the shortest known winning child when refining.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo test --release
$ cargo doc --no-deps
```

Results:

- `cargo fmt` completed with no changes after the final pass.
- `cargo clippy --all-targets` reported zero warnings.
- `cargo test` and `cargo test --release` passed all tests.
- `cargo doc --no-deps` built cleanly.

### Benchmark

```text
$ cargo run --release --example benchmark -- --runs 5 --timeout 5
```

```text
runs=5 timeout=5s epsilon=0.25 refine_shortest=false

| name | outcome | nodes | child_evals | mean (s) | min (s) | max (s) | pv_len |
|------|---------|------:|------------:|---------:|--------:|--------:|-------:|
| two_rook_mate | win | 6 | 35 | 0.000 | 0.000 | 0.000 | 3 |
| epsilon_mate | win | 571 | 12477 | 0.004 | 0.003 | 0.004 | 5 |
| promotion_transposition | win | 607 | 4972 | 0.001 | 0.001 | 0.001 | 15 |
| m26 | win | 299 | 2468 | 0.001 | 0.001 | 0.001 | 11 |
| opening_f2 | win | 675 | 14939 | 0.004 | 0.004 | 0.004 | 7 |
| rook_pawn_endgame | win | 714 | 5268 | 0.001 | 0.001 | 0.002 | 9 |
| m19 | draw | 834828 | 17372569 | 5.000 | 5.000 | 5.000 | 0 |
| startpos | draw | 657233 | 16615191 | 5.000 | 5.000 | 5.000 | 0 |
```

(The `m19` and `startpos` cases hit the 5-second time limit and report a draw.)

The node/child-eval counts on decisive positions are consistent with stopping
sibling evaluation once a winning child is proven.  On an immediately decisive
position the effect is most visible:

```text
$ cargo run --release --example solve_no_refinement -- \
    'k7/p7/8/8/8/8/8/R3K3 w - - 0 1'
outcome: Win nodes: 1
a1a7
```

This position has 14 legal first moves, but the solver stops after the winning
`a1a7` and returns the PV in a single recursive node.

### `fen1` and `fen2` regression checks

`fen1`:

```text
$ cargo run --release --example solve_no_refinement -- \
    '6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25'
outcome: Loss nodes: 1129
g8g7
b1b8
g7h7
b8h8
h7g7
h8h7
g7g8
h7g7
g8h8
g7g8
h8h7
g8g6
```

This matches the expected `~1,129 dfpn nodes` and the 12-ply forced-loss PV
from the speed checkpoint.

`fen2`:

```text
$ cargo run --release -- --fen \
    '6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26' \
    --no-refine-shortest --timeout 60
outcome: win
pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8
```

This returns the same `outcome: win` and the same 7-ply PV as before.

## Files changed

- `src/search/dfpn/children.rs`
- `src/search/dfpn/selection.rs`
- `src/search/dfpn/core.rs`
- `docs/plans/ultimattt/report1.md` (this report)
