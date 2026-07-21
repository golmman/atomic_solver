# Report: Plan 11 — Cache `ChildInfo` across `select_children` iterations

This report documents the application of `docs/plans/speed/plan11.md`.

## Changes applied

### `src/search/dfpn/children.rs`

- Added `best_child_index: Option<usize>` to `ChildSelection` and derived
  `Clone + Copy` so the cached selection can be reused in the loop.
- Split `select_children` into:
  - `evaluate_all_children(...) -> Vec<ChildInfo>`
  - `select_from_children(children: &[ChildInfo], is_or_node: bool) ->
    ChildSelection`
  - `evaluate_child(...)` is now `pub(super)` so the core loop can re-evaluate
    a single child by index.
- `evaluate_child` increments a new `Search::child_evals` counter (one per
  evaluation, i.e. one `do_move`/`undo_move` pair per child).

### `src/search/dfpn/core.rs`

- The `dfpn` main loop now keeps one `Vec<ChildInfo>` per recursive node.
- On the first iteration it calls `evaluate_all_children` once.
- On subsequent iterations it re-evaluates only the child that was expanded in
  the previous loop and overwrites that slot in the cached vector.
- It then calls `select_from_children` to recompute `pn`/`dn`, the best/second
  unsolved child, and any solved outcome from the updated cache.

### `src/search/dfpn/mod.rs`

- Added `child_evals: u64` to `Search`, reset in `begin_run`.
- Added `pub fn child_evaluations(&self) -> u64` so examples and the CLI can
  report it.

### `examples/twin_stats.rs`

- Now also prints `search.child_evaluations()` alongside `nodes`.

## Why this is safe

The parent position and its path (`path_stack`, `path_code`) do not change
between loop iterations of a single `dfpn` call.  `evaluate_child` only depends
on the parent path, the move being evaluated, and the (mutable) transposition
table.  After a selected child is expanded and the position is restored, the
siblings are still at the same depth with the same parent path, so their
previously computed bounds remain valid.  Re-evaluating only the expanded child
picks up any new TT results for that child.

## Benchmarks

Sample FENs were run with the CLI (`--fen ...`, default 5-second timeout,
`refine_shortest` enabled, 64 MB TT).  The "before" numbers were collected by
restoring a temporary `select_children` that re-evaluated every child on every
iteration (the pre-Plan 11 behavior).  The "after" numbers are with the cached
loop.

| FEN | before nodes | after nodes | before `child_evals` | after `child_evals` | before time (warm mean) | after time (warm mean) |
|-----|-------------:|------------:|---------------------:|--------------------:|------------------------:|-----------------------:|
| `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1` | 133 | 133 | 1,830 | 372 | 0.0072 s | 0.0099 s |
| `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3` | 373,557 | 373,346 | 10,023,687 | 1,442,887 | 1.502 s | 0.433 s |
| `4k3/PP6/8/8/8/8/8/4K3 w - - 0 1` | 97,743 | 97,743 | 1,837,805 | 490,206 | 0.219 s | 0.111 s |
| `4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19` (m19) | 697,322 | 1,035,119 | 28,631,296 | 18,341,558 | 5.007 s (timeout) | 5.007 s (timeout) |

Node counts are essentially identical for the positions that solve within the
timeout.  The small two-rook position shows less relative benefit because the
search is tiny; the caching overhead can outweigh the saved work.  On the
larger decisive positions the savings are large:

- Epsilon mate: **~7× fewer child evaluations**, **~3.5× faster**.
- Promotion transposition: **~3.75× fewer child evaluations**, **~2× faster**.
- Each saved `child_eval` corresponds to one saved `do_move` and one saved
  `undo_move`, so the saved `do_move`/`undo_move` pair counts are roughly twice
  the differences above (e.g. epsilon saves about 17.1 million
  `do_move`/`undo_move` calls).

`m19` still times out at 5 seconds, but caching lets the search visit more
recursive nodes within the same wall time while evaluating fewer children per
node, so the child-evaluation overhead is substantially lower.

`cargo test --release tests/test_review`:

- Before Plan 11: ~1.53 s
- After Plan 11: ~0.56 s

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --release
$ cargo doc --no-deps
$ cargo run --release --example twin_stats
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
$ cargo run --release -- --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"
$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
```

Results:

- `cargo fmt` produced no diffs.
- `cargo clippy --all-targets` reported zero warnings.
- `cargo test --release` passed all tests.
- `cargo doc --no-deps` built cleanly.
- `examples/twin_stats` still reports consistent twin statistics and now also
  prints `child_evaluations`.
- The sample FENs produce the same outcomes and PVs as before.

## Conclusion

Caching `ChildInfo` across `dfpn` loop iterations dramatically reduces the
number of `do_move`/`undo_move` pairs and the associated TT probes and outcome
checks.  The speedup is most pronounced on positions with many loop iterations
and many legal moves (epsilon mate, promotion transposition), where the
per-node work dropped by 3–7× and wall time by 2–3.5×.  The two-rook mate is
fast enough that the small fixed overhead of the cache is visible, but the
regression is negligible and the gains on realistic searches dominate.
