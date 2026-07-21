# Report: Plan 12 — Reduce Kawano-simulation overhead

This report documents the application of `docs/plans/speed/plan12.md`.

## Changes applied

### `src/search/dfpn/simulate.rs`

- `simulate` is now a free function that takes the transposition table and a
  mutable borrow of the real search path stack, instead of being an `&self`
  method that cloned `self.path_stack` on every call.
- `try_use_tt` no longer clones the path stack per twin; it passes
  `&mut self.path_stack` directly to `simulate`.
- `simulate` records `original_len = path.len()` on entry, pushes the current
  repetition key before recursing, and restores the stack with
  `path.truncate(original_len)` on every exit path.
- Repetition detection inside `simulate` uses `Vec::contains` on the borrowed
  stack.
- Updated unit tests in `src/search/dfpn/tests.rs` to call the free
  `simulate` function with `&search.tt` and a separate simulation stack.

## Why this is safe

`simulate` pushes and pops (truncates) the borrowed `path_stack` symmetrically.
Because `try_use_tt` is called while the main search is exploring a child, the
real `path_stack` at that moment is exactly the parent prefix that the old code
was cloning.  The function restores the stack to its original length before
returning, so the main search path is never corrupted.  All early-return paths
are covered by the single `path.truncate(original_len)` at the bottom of the
function.

## Benchmarks

### Depth-limited promotion transposition

A deterministic, twin-exercising position that completes well inside the
default timeout:

```text
$ cargo run --release --example solve_depth_limited -- \
    "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1" 7
```

| metric | before Plan 12 | after Plan 12 |
|--------|---------------:|--------------:|
| outcome | Win | Win |
| nodes | 2,067 | 2,067 |
| time (warm mean) | ~0.090 s | ~0.090 s |

Node counts are identical and timing is within run-to-run noise, because this
position solves too quickly for the path-stack clone to be a bottleneck.

### `twin_stats` example

The `twin_stats` example exercises `try_use_tt` simulation on the cyclic rook
positions.  Both the baseline and the Plan 12 version complete in roughly the
same wall time because the two cyclic positions are bounded by the 5-second
search timeout:

```text
before Plan 12:  ~10.03 s total
after Plan 12:    ~10.03 s total
```

`twin_stats` output is timing-sensitive at the timeout boundary, but the shape
is unchanged: `evictions` remain `0` and peak live twins per entry stay in the
same range (`3–4`).  A representative after run:

```text
promotion start:
  outcome: Win, nodes: 607, child_evals: 5422
  twin insertions: 0, evictions: 0
  peak live twins in one entry: 0
promotion transpose:
  outcome: Loss, nodes: 22, child_evals: 325
  twin insertions: 0, evictions: 0
  peak live twins in one entry: 0
cyclic rook safe:
  outcome: Draw, nodes: 1830344, child_evals: 21061484
  twin insertions: 157, evictions: 0
  peak live twins in one entry: 4
cyclic rook safe (after 4 reversible moves):
  outcome: Draw, nodes: 1833270, child_evals: 20989223
  twin insertions: 172, evictions: 0
  peak live twins in one entry: 3
```

### Regression suite

`cargo test --release tests/test_review`:

- before Plan 12: ~0.55 s
- after Plan 12: ~0.54 s

The promotion-transposition and two-rook-transposition tests, which stress
cross-path twin reuse, still pass with identical PV lengths.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --release
$ cargo doc --no-deps
$ cargo run --release --example twin_stats
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
```

Results:

- `cargo fmt` produced no diffs.
- `cargo clippy --all-targets` reported zero warnings.
- `cargo test --release` passed all tests, including all GHI/repetition and
  transposition tests.
- `cargo doc --no-deps` built cleanly.
- `cargo run --release --example twin_stats` produces consistent twin
  statistics (`evictions: 0`, peak twins 3–4).
- The sample FENs produce the same outcomes and PVs as before.

## Conclusion

Plan 12 removes the per-twin `Vec<u64>` clone from Kawano simulation by lending
the real search path stack and truncating it on return.  The change is
semantically neutral: node counts on twin-heavy positions are unchanged, and
all regression tests pass.  On the current benchmark set the speedup is not
measurable because simulation is a small fraction of total runtime and the
`solve_depth_limited` case finishes too quickly for allocation to matter.
The main benefit is reduced allocation and a simpler data flow, which may
become more visible if future work increases simulation depth or call frequency.
