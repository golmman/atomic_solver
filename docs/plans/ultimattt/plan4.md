# Plan 4: Work-bounded iterative deepening

## Goal

Replace the depth-doubling `solve_outcome` bootstrap with a work-bounded root loop that reuses the transposition table across iterations, eliminating the `max_depth=8` horizon cliff seen on `fen1`.

## Background

`ultimattt` runs its sequential `dfpn` in work-bounded chunks of `CHECK_TICK_WORK` nodes:

```rust
while !root.bounds.solved() {
    let (out, this_work, _) = worker.mid(
        Bounds { phi: INFINITY/2, delta: INFINITY/2 },
        CHECK_TICK_WORK,
        root,
        &self.root,
    );
    root = out;
    work += this_work;
    // check time limit, dump table, print debug info, etc.
}
```

Each call returns when its node budget is exhausted, even if the position is not solved, and the TT persists between calls. This means the search naturally grows past any fixed depth horizon: as soon as a 12-ply mate is found inside the tree, it propagates into the next work chunk and prunes the rest.

`atomic_solver` currently uses `max_depth` doubling (`1, 2, 4, 8, 16, ...`). If the solution is 12 plies and the timeout is 60 seconds, the `max_depth=8` iteration expands the entire 8-ply tree and times out before ever reaching `max_depth=16`.

## Files to modify

- `src/search/dfpn/mod.rs`
- `src/search/dfpn/core.rs`
- `src/main.rs`
- `src/search/tt/table.rs` (minor, for TT persistence between chunks)
- `examples/benchmark.rs` (optional, to add a work-chunk parameter)

## Concrete changes

1. Add a `max_work: u64` parameter to `Search::dfpn` (or to a new `dfpn_limited` wrapper).
   - Track the starting `self.child_evals` or `self.nodes` value at entry.
   - In the main loop, if `self.child_evals - start >= max_work` (or `self.nodes - start_nodes >= max_work`), break and store the current bounds as an unsolved result.
   - On break, treat the result like a `max_depth` cutoff: store `outcome = None` with current `pn`/`dn` and `remaining_depth` unchanged.
2. Refactor `Search::solve_outcome` to call the work-bounded `dfpn` in chunks instead of `max_depth` doubling:
   ```rust
   let mut chunk = 500_000u64;
   while !self.time_exceeded() {
       self.reset_search_state();
       let outcome = self.dfpn(pos, INF, INF, chunk, true);
       if outcome != Outcome::Draw {
           // decisive result found
           break;
       }
       chunk = chunk.saturating_mul(2);
   }
   ```
   The TT is *not* cleared between chunks; only the path state is reset.
3. For `main.rs`, when `--no-refine-shortest` is given, call `Search::solve()` (unbounded or work-bounded) directly instead of `solve_outcome`, to avoid the refinement-oriented bootstrap.
4. Keep `bootstrap_success_depth` available for `refine_sppv` when refinement is enabled. The work-bounded search can record the number of plies in the first decisive PV and use that as the initial `hi` bound.

## Verification

- `cargo run --release -- --fen "$fen1" --timeout 60` must return `outcome: loss` instead of timing out.
- `cargo run --release -- --fen "$fen2" --timeout 60` must remain fast.
- `cargo test` and `cargo test --release` pass.
- `examples/benchmark.rs` shows no regressions on the benchmark suite.

## Risks / notes

- The `dfpn` function returns `Outcome` today. A work-bounded run that does not reach a terminal must return `Outcome::Draw` or a new internal `Unknown` sentinel. Returning `Draw` is what the current `max_depth` cutoff already does, so reusing that behavior is safe.
- Time checks inside `dfpn` already stop the search, but they leave the TT in an inconsistent state if the check happens deep in the tree. Work-bounded chunks are cleaner because the budget is consumed node-by-node at the root level.
- `refine_shortest` still needs an initial depth estimate. The first decisive work chunk already produces a PV of some length `L`; that `L` becomes the initial `hi` bound for the binary search in `refine_sppv`.
- This change is the largest of the four plans. It should be implemented and tested after Plans 1–3 are in place, because the work-bounded loop reuses the TT and relies on good threshold/selection/replacement behavior.
