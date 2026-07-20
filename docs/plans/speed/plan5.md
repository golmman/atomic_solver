# Plan 5: Cache per-node move scores

## Start

Read `docs/plans/speed/analysis.md`.  Open `src/search/dfpn/history.rs` and
`src/search/ordering.rs`.  Identify the `MoveList` API that the project already
uses (`len`, indexing, `as_mut_slice`, `MoveList::new`).

## Goal

Stop scoring the same move many times inside the sort comparator.

## Background

`sort_moves` currently calls `scorer.score` for both `a` and `b` inside the sort
closure:

```rust
slice.sort_by(|&a, &b| {
    let sa = self.scorer.score(&pos.board, a, &state)
        + self.history[us][a.from_sq() as usize][a.to_sq() as usize]
        + self.killer_bonus(a, depth);
    ...
});
```

Because `score` is called for every comparison, an N-move node triggers
`score` O(N log N) times.  `StaticAtomicScorer::score` is expensive: it computes
attack bitboards, iterates over enemy commoners for distance, expands blast zones,
and performs several piece lookups.  Scoring once per move and sorting by the
cached value is a clear win.

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn/history.rs" lines="26-34" /> <ref_snippet file="/workspace/atomic_solver/src/search/ordering.rs" lines="79-175" />

## Implementation tasks

1. Precompute the total ordering score for each move in `moves`:
   ```rust
   let mut scored: Vec<(Move, i32)> = moves
       .as_slice()
       .iter()
       .copied()
       .map(|m| {
           let s = self.scorer.score(&pos.board, m, &state)
               + self.history[us][m.from_sq() as usize][m.to_sq() as usize]
               + self.killer_bonus(m, depth);
           (m, s)
       })
       .collect();
   ```
2. Sort `scored` by descending score.
3. If `best_from_tt` is present and found in `scored`, rotate it to the front
   (or add a large bonus before sorting).
4. Write the reordered moves back into `moves.as_mut_slice()`.
5. Keep the existing `StaticAtomicScorer` logic unchanged; only the sorting
   wrapper in `history.rs` changes.

## File changes

- `src/search/dfpn/history.rs`

## Risks

- `MoveList` may not expose `as_slice` or `as_mut_slice` beyond what is already
  used.  Verify the available methods and prefer writing into `as_mut_slice()`
  by index.
- The extra `Vec<(Move, i32)>` allocation per node is cheap compared to the
  repeated scoring it replaces.
- If `score` had side effects it does not; it is a pure function.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
```

Move order must be unchanged for any given position and the same `outcome`/`pv`
must be produced.  A micro-benchmark on `examples/static_move_scores.rs` or a
short timed solve should show faster sorting.

## Final task

Write `docs/plans/speed/report5.md` with a before/after node-count or timing
comparison and note whether move ordering remained stable.
