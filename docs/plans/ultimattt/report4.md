# Report: Plan 4 — Work-bounded iterative deepening

This report documents the implementation of `docs/plans/ultimattt/plan4.md`.

## Summary

Extended `Search::dfpn` with a `max_work: u64` budget and rewrote
`Search::solve_outcome` to bootstrap with a depth-doubling loop where each depth
probe is also work-bounded.  This eliminates the `max_depth=8` horizon cliff
seen on `fen1`: a shallow probe cannot consume the entire time budget, and the
transposition table is reused as the depth bound grows.  `refine_sppv` now uses
binary search on the depth bound instead of decrementing from the top, and the
TT bound reuse in `evaluate_child` was tightened so that work-bounded searches
do not leak inconsistent `pn`/`dn` values into depth-bounded probes.

The final `solve_outcome` is therefore a *hybrid*: it keeps the depth-doubling
structure that produces a tight upper bound for `find_ppv`/`refine_sppv`, but
caps each depth iteration with a work budget so an over-expanded `max_depth=8`
probe returns quickly instead of timing out.

## Changes applied

### `src/search/dfpn/core.rs`

- Added `max_work: u64` to `dfpn` and captured `self.child_evals` at entry.
- In the main loop, if `self.child_evals - start >= max_work`, the search
  breaks and stores the current bounds as an unsolved result (a work cutoff,
  analogous to a `max_depth` cutoff).
- The recursive child call computes the *remaining* work budget and passes it
  down, so the `max_work` limit is global to the whole `dfpn` invocation.

### `src/search/dfpn/mod.rs`

- `search_depth` and the `solve`/refine paths pass `u64::MAX` for unbounded
  work.
- `solve_outcome` was rewritten:
  - It still doubles `max_depth` (1, 2, 4, 8, 16, ...), giving
    `refine_sppv` a tight initial upper bound.
  - Each `dfpn` call is also given a doubling `max_work` chunk starting at
    500,000.  This prevents a single `max_depth=8` probe from expanding until
    the wall-clock timeout.
  - The transposition table is *not* cleared between probes, so work spent on
    a shallower, failing probe is reused by the next deeper probe.
  - After a decisive probe, `bootstrap_success_depth` is read from the freshly
    stored root entry (even if it is marked `repetition_seen`), and
    `bootstrap_fail_depth` is recorded from the last failing depth.
  - The previous unbounded fallback is preserved for positions that do not
    resolve within 64 plies.
- `refine_sppv` now binary-searches the shortest winning depth with
  `probe = lo + (hi - lo) / 2` instead of probing `hi - 1` repeatedly.  The
  predicate "`outcome` is decisive at depth `d`" is monotonic in `d`.

### `src/search/dfpn/children.rs`

- `evaluate_child` no longer reuses unsolved `pn`/`dn` bounds from a TT entry
  whose `remaining_depth` is `u32::MAX` or larger than the current
  `child_max_depth`.  This prevents work-bounded (or previously deeper)
  searches from feeding over-optimistic or over-pessimistic starting values
  into a shallower depth probe, which had caused `refine_sppv` to explore
  huge trees or return incorrect `Draw` answers.

### `src/main.rs`

- The CLI still runs `solve_outcome` + `find_ppv` + optional `refine_sppv`.
- For `--no-refine-shortest` this returns the Proof PV produced by
  `find_ppv`, matching the previous regression output for `m27`.

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

### `TtEntry` size

`TtEntry` size remains 248 bytes, well under the 512-byte cap.

### Benchmark

```text
$ cargo run --release --example benchmark -- --runs 10 --timeout 5
```

```text
runs=10 timeout=5s epsilon=0.125 refine_shortest=false

| name | outcome | nodes | child_evals | mean (s) | min (s) | max (s) | pv_len |
|------|---------|------:|------------:|---------:|--------:|--------:|-------:|
| two_rook_mate | win | 6 | 35 | 0.000 | 0.000 | 0.000 | 3 |
| epsilon_mate | win | 533 | 11582 | 0.003 | 0.003 | 0.004 | 5 |
| promotion_transposition | win | 819 | 6601 | 0.002 | 0.001 | 0.002 | 15 |
| m26 | win | 299 | 2461 | 0.001 | 0.000 | 0.001 | 11 |
| opening_f2 | win | 658 | 13675 | 0.004 | 0.004 | 0.004 | 7 |
| rook_pawn_endgame | win | 714 | 5268 | 0.001 | 0.001 | 0.002 | 9 |
| m19 | draw | 866054 | 17800937 | 5.000 | 5.000 | 5.000 | 0 |
| startpos | draw | 688648 | 16851720 | 5.000 | 5.000 | 5.000 | 0 |
```

The six decisive positions sum to 3,029 nodes and 39,622 `child_evals`,
identical to the Plan 3 baseline for the same `epsilon = 0.125` settings.

### `fen1` and `fen2` regression checks

`fen2`:

```text
$ cargo run --release -- --fen \
    '6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26' \
    --timeout 60
outcome: win
pv: b1b8 g8f7 b8f8 f7g7 d6e5 g7h7 f8h8
```

`fen1` (the original `max_depth=8` timeout case):

```text
$ cargo run --release -- --fen \
    '6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25' \
    --timeout 60
outcome: loss
pv: g8g7 b1b8 g7h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6
```

Both return the expected decisive outcome within the 60-second window; `fen1`
no longer times out on the `max_depth=8` horizon.

### `--no-refine-shortest`

```text
$ cargo run --release -- --no-refine-shortest --fen \
    '6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25' \
    --timeout 60
outcome: loss
pv: g8g7 b1b8 g7h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6
```

## Files changed

- `src/search/dfpn/core.rs`
- `src/search/dfpn/mod.rs`
- `src/search/dfpn/children.rs`
- `src/main.rs`
- `docs/plans/ultimattt/report4.md` (this report)
