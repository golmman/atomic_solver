# PV Extraction Issue for Mate-in-2 Position

## Observed behavior

Running the solver on the plan-2 mate-in-2 position

```text
rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3
```

produces a short, correct refutation but not the longest resistance:

```bash
$ cargo run -- --fen "rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3"
outcome: loss
pv: c7c5 d5d7
```

`c7c5 d5d7` is a legal, forced win for White, but it is not Black's best defense. Black can hold out longer with:

```text
d7d6 d5f7 e8d7 f7e7
```

After this line Black has no legal moves because the black king is destroyed by the blast on `e7`.

## Verification of the longer line

The line `d7d6 d5f7 e8d7 f7e7` was verified with the `atomic-movegen` `fen_after` example:

1. After `d7d6`: `rnbqcbnr/ppp1p2p/3p1pp1/3Q4/8/4P3/PPPP1PPP/RNB1CBNR w KQkq - 0 4`
2. After `d5f7`: `rnbqcbnr/ppp1pQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1CBNR b KQkq - 1 5`  
   Black has only one legal move: `e8d7`.
3. After `e8d7`: `rnbq1bnr/pppcpQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1CBNR w KQ - 2 6`
4. After `f7e7`: `rnb3nr/ppp4p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1CBNR b KQ - 0 7`  
   Black has zero legal moves; White has won.

In contrast, `c7c5 d5d7` ends with the same zero-legal-moves result after only two plies.

## Per-defense solver output

A temporary debug script solved each root Black move and let White solve the resulting position:

```text
Solver result from start:
outcome: Loss, pv: c7c5 d5d7, nodes: 26

a7a5: outcome Win, pv: d5d7, nodes: 1
a7a6: outcome Win, pv: d5d7, nodes: 1
b7b5: outcome Win, pv: d5d7, nodes: 1
b7b6: outcome Win, pv: d5d7, nodes: 1
b8a6: outcome Win, pv: d5d7, nodes: 1
b8c6: outcome Win, pv: d5d7, nodes: 1
c7c5: outcome Win, pv: d5d7, nodes: 1
c7c6: outcome Win, pv: d5d7, nodes: 1
d7d6: outcome Win, pv: d5f7 e8d7 f7e7, nodes: 7
...
```

All Black moves lose, but `d7d6` is the only defense that forces a four-ply win (`d5f7 e8d7 f7e7`); every other Black move lets White win in two plies (`d5d7`).

## Root cause

The DF-PN+ search itself is correct in the sense that it returns `Outcome::Loss` for the start position. The bug is in `best_move` selection and PV extraction, not in the proof/disproof numbers.

In `src/search/dfpn.rs`:

- `best_move` is updated to `selection.best_move` from `select_children`.
- `select_children` calls `best_and_second`, which orders children by `vpn` for OR nodes and `vdn` for AND nodes.
- For a solved `Win` child the stored pair is `(0, INF)`; for a solved `Loss`/`Draw` child the pair is `(INF, 0)`.
- All root Black moves lead to positions that are a forced win for White, so every child has the same collapsed `(pn, dn)` pair from the attacker's perspective.
- When all `vpn`/`vdn` values are tied, `best_and_second` falls back to the static move order produced by `sort_moves`.
- `sort_moves` uses `StaticAtomicScorer`, which ranks `c7c5` higher than `d7d6`. `extract_pv` then simply follows the stored `best_move` chain, producing `c7c5 d5d7`.

The solver therefore has no notion of mate distance or "longest defense". It proves that the position is lost, but it does not preserve the depth needed to choose the line that makes the opponent work the hardest.

## Related code notes

- `Outcome::to_pn_dn` in `src/position.rs` collapses `Loss` and `Draw` to `(INF, 0)`.
- `select_children` in `src/search/dfpn.rs` computes `pn`/`dn` but never computes a mate-distance.
- `best_and_second` in `src/search/dfpn.rs` only compares `vpn`/`vdn`.
- `extract_pv` in `src/search/dfpn.rs` is a pure `best_move` follower.
- `is_solved_by_children` in `src/search/dfpn.rs` returns `Loss` for any fully resolved set of children that are not all `Draw`, even if some children are `Draw` and others are `Win` (from the child's side). In a mixed `Win`/`Draw` case the player to move could choose the `Draw`, so the parent should be `Draw`, not `Loss`. This is a separate correctness issue that also becomes relevant once the solver handles draws correctly.

## Proposed fix

Track a `depth` (mate-distance) field for solved transposition-table entries and use it when selecting `best_move`:

1. Add `depth: u32` to `TtEntry`.
2. Add `depth` to `ChildInfo`/`ChildSelection`.
3. When a node's `Outcome` is resolved:
   - `Win`: `depth = 1 + min(depth(child))` over all winning children; `best_move` is the corresponding child.
   - `Loss`: `depth = 1 + max(depth(child))` over all losing children; `best_move` is the corresponding child (the longest defense).
   - `Draw`: `depth = 0` (or `1 + max(depth(child))` over drawing children).
4. Update `best_and_second` or `select_children` to prefer `depth` when the outcome is known.
5. Fix `is_solved_by_children` to return `Draw` if there is any `Draw` child and no winning child, instead of returning `Loss` for mixed `Win`/`Draw` resolved children.

A short-term workaround is to recompute `best_move` after a node is solved by re-evaluating the children and choosing the one with the longest (or shortest) continuation, but that is more expensive and less robust than propagating depth through the TT.

## Impact

- The game-theoretic outcome is still correct: the position is `Loss` for Black.
- The printed PV is misleading because it presents the fastest Black collapse (`c7c5`) as the principal variation instead of the most resistant defense (`d7d6`).
- With depth tracking, the solver will produce `d7d6 d5f7 e8d7 f7e7` for the reported position and will also consistently produce the shortest winning line for `Win` nodes regardless of static move ordering.
