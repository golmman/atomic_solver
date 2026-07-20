# Plan 8: Improve and tune move ordering

## Start

Read `docs/plans/speed/analysis.md`.  Inspect `src/search/ordering.rs` and
`src/search/dfpn/history.rs`.  Review the current history/killer constants and
the `StaticAtomicScorer` scoring rules.

## Goal

Reduce the number of nodes searched by giving the solver a better move order
for atomic chess.

## Background

Move ordering in DF-PN is critical: the algorithm expands children in order of
their proof/disproof numbers, and a good order finds the decisive lines first,
which propagates bounds faster.  The current setup has three sources:

1. `StaticAtomicScorer` — a fixed static function.  It recomputes distance to the
   nearest enemy commoner for every move from/to square and does not share work
   between moves. <ref_snippet file="/workspace/atomic_solver/src/search/ordering.rs" lines="79-175" />
2. History table — a flat `+100` bonus on every winning child, aged by halving
   all entries every 10,000 nodes. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/history.rs" lines="10-15" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/history.rs" lines="47-67" />
3. Killer moves — two slots per ply. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/history.rs" lines="54-67" />

There is room for both static-scoring accuracy and dynamic-history tuning.

## Implementation tasks

1. **Share static-scorer work across moves.**
   - Precompute a nearest-commoner distance map for the enemy side once per node
     (`[i8; 64]` or `Option<i8>`) and look up `from` and `to` instead of
     iterating over the enemy commoners for every move.
   - Precompute `attacks::king_attacks(sq)` or other expensive bitboards once
     per call instead of inside the scoring loop.
2. **Scale history bonuses by depth.**
   - Replace the flat `HISTORY_BONUS` with `depth * depth` or a small table so
     moves that solve near the root get a bigger bonus than deep flukes.
   - Clamp to `HISTORY_MAX` as before.
3. **Counter-move and follow-up history (optional).**
   - Add a `counter_moves: [[Option<Move>; 64]; 64]` or 2D table keyed by
     `(previous_move, current_move)`.
   - When `update_history` is called, also update the counter-move table for the
     move that led to the current node.
4. **Tune killer slots.**
   - Only store a move as a killer if it is not already the first killer.
   - Consider clearing killer tables between iterative-deepening probes if they
     become polluted.
5. **Add atomic-chess-specific capture scoring.**
   - Reward captures that blast multiple enemy pieces or that leave the enemy
     with fewer commoners, not just the last-commoner blast.
   - Distinguish between a safe capture and a capture that also loses our own
     attacking piece in the blast.
6. **Aging.**
   - Replace the full-table halving pass with a periodic right-shift if it proves
     too slow, or keep it if it is not on the hot path.

## File changes

- `src/search/ordering.rs`
- `src/search/dfpn/history.rs`
- `src/search/dfpn/mod.rs` (if new history tables are added to `Search`)

## Risks

- Move ordering does not change game-theoretic results, but it can change the
  node count and the exact PV chosen when multiple winning sequences exist.
  Tests that compare node counts may need to be updated, but outcome tests should
  remain stable.
- Over-tuning the static scorer for specific positions can hurt others.  Test on
  a variety of FENs, including the integration tests in `tests/`.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19"
```

Compare node counts and wall-clock time before and after.  Outcomes must stay
the same.

## Final task

Write `docs/plans/speed/report8.md` documenting the ordering changes and the
node-count/time delta on a representative set of positions.
