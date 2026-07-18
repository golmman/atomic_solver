# Plan 15 Implementation Report

## Summary

Implemented the terminal-detection ordering fix and added regression tests.
Investigated the `extract_pv` path-code claim and found the existing code already
uses the correct 1-indexed depth; a regression test now locks that behavior in.

## Changes made

### `src/position.rs`

Reordered `Position::outcome_from_state` so that the no-legal-moves
checkmate/stalemate branch is evaluated before the `rule50 >= 100` and
`occupied().count() == 2` draw heuristics:

1. Own commoners gone -> `Loss`
2. Opponent commoners gone -> `Win`
3. No legal moves and in check -> `Loss`
4. No legal moves and not in check -> `Draw`
5. `rule50 >= 100` -> `Draw`
6. Only two pieces remain -> `Draw`
7. Otherwise `None`

This makes `outcome()`/`outcome_from_state` consistent with the rule that a
game ends by checkmate or stalemate before any draw-by-rule or material draw
can be claimed.

Added unit tests:

- `fifty_move_checkmate_is_loss_not_draw`: `7K/8/8/8/8/8/1Q6/k7 b - - 100 1` now
  returns `Loss` instead of `Draw`.
- `fifty_move_stalemate_is_draw`: `7k/8/8/8/8/8/2q5/K7 w - - 100 1` is still
  `Draw` because stalemate is terminal.
- `two_piece_touching_commoners_is_draw`: `8/8/8/8/8/8/1K6/k7 b - - 0 1` is a
  draw.  This replaces the plan's "two-piece adjacent-king checkmate" test
  because, in standard atomic chess, touching commoners are allowed and are not
  treated as an attack (see `docs/plans/movegen/feedback.md` and
  `docs/plans/movegen/research.md` section 2.6).  The black commoner has legal
  moves, so the position is not checkmate and the two-piece material draw
  heuristic correctly applies.

### `tests/test_terminal_ordering.rs`

Added integration tests for the same three FENs, confirming the solver reports
the expected outcomes through the full `Search::solve` pipeline.

### `src/search/dfpn.rs`

No source change to `extract_pv` was needed.  The existing line

```rust
path_code ^= zobrist::path_random(mv, pv.len());
```

already uses the correct 1-indexed depth.  `pv.push(mv)` precedes this update,
so `pv.len()` is `1` for the first move, `2` for the second, and so on — exactly
matching `dfpn`'s use of `self.path_stack.len()` when the edge is made.

Added a regression test `extract_pv_follows_path_dependent_twin_entries` that:

1. Solves a short forced-mate position.
2. Re-stores every node along the principal variation as a path-dependent twin
   keyed by the 1-indexed path code.
3. Asserts `extract_pv` can still reproduce the full PV.

This test fails if `extract_pv` uses a 0-indexed or 2-indexed depth for path-code
recomputation.

## Why the plan's proposed `extract_pv` change is incorrect

Plan 15 suggested:

```rust
path_code ^= zobrist::path_random(mv, pv.len() + 1);
```

Because `pv.len()` is already `1` after the first push, the `+ 1` would make
the depth `2` for the first move.  `dfpn` uses `self.path_stack.len()` at the
point of recursion, which is `1` for the first edge, so the proposed change
would break twin lookup.  The regression test above confirms the current code is
correct.

## Why the second CLI FEN does not become `loss`

The plan expected:

```text
$ cargo run --release -- --fen "8/8/8/8/8/8/1K6/k7 b - - 0 1"
outcome: loss
```

This FEN is two commoners on adjacent squares (`Ka1` and `Kb2`).  In the
`atomic-movegen` model used by this project, touching commoners are legal and
do not count as an attack; the black commoner has safe moves (`...Kb1` and
`...Ka2`).  Therefore the position is not checkmate and the two-piece material
draw heuristic is the correct terminal classification.  The solver reports
`outcome: draw` for this FEN.

## Verification results

```text
$ cargo fmt                    # passed
$ cargo clippy --all-targets   # passed
$ cargo test --all-targets     # passed
$ cargo doc --no-deps          # passed
$ cargo test --release --test test_epsilon      # passed
$ cargo test --release --test test_ghi -- --ignored  # passed
```

All unit, integration, release, and example tests pass.

CLI checks:

```text
$ cargo run --release -- --fen "7K/8/8/8/8/8/1Q6/k7 b - - 100 1"
outcome: loss
pv:
nodes: 2

$ cargo run --release -- --fen "8/8/8/8/8/8/1K6/k7 b - - 0 1"
outcome: draw
pv:
nodes: 8
```

The first FEN correctly reports `loss`; the second reports `draw` because it is
not a checkmate under the implemented atomic-chess rules.

## Remaining concerns

- The `max_depth == 0` cutoff storage issue identified in `review3.md` section
  2.2 is intentionally outside the scope of Plan 15 and remains unaddressed
  here.
- The two-piece adjacent-commoner case in Plan 15 is based on a
  misunderstanding of the `atomic-movegen` rules.  The `feedback.md` document
  already notes that touching commoners are allowed, so the test expectation
  should be `Draw`, not `Loss`.
