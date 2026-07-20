# Plan 2: Remove trait-object dispatch for move scoring

## Start

Read `docs/plans/speed/analysis.md` and inspect `src/search/dfpn/mod.rs` and
`src/search/dfpn/history.rs`.  Confirm that `StaticAtomicScorer` is the only
scorer currently used and that the `MoveScorer` trait is the public API for
scoring.

## Goal

Eliminate the virtual call overhead of `Box<dyn MoveScorer>` from the hot move
sorting path.

## Background

`Search` stores a boxed trait object:

```rust
scorer: Box<dyn MoveScorer>,
```

`sort_moves` calls `self.scorer.score(...)` inside the comparator closure.  With
many moves and an O(N log N) sort, the dynamic dispatch becomes a small but real
overhead, and it also prevents inlining of `StaticAtomicScorer::score`.

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn/mod.rs" lines="39" /> <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/history.rs" lines="26-34" />

## Implementation tasks

1. Keep `MoveScorer` as a public trait so callers can still supply custom
   scorers in examples/tests.
2. Change `Search` to use a concrete `StaticAtomicScorer` or make `Search`
   generic over `S: MoveScorer`:
   ```rust
   scorer: StaticAtomicScorer,
   ```
   or
   ```rust
   scorer: S,
   ```
3. Update `Search::new` to construct the concrete scorer directly.
4. Update any `set_scorer`-style method if one exists; currently there is none,
   so no API surface changes.
5. If genericity is important, prefer the concrete field for now because it
   avoids monomorphisation bloat and keeps `Search` simple.

## File changes

- `src/search/dfpn/mod.rs`
- `src/search/dfpn/history.rs` (no change if `score` still takes `&dyn`? No;
  `sort_moves` can call `self.scorer.score` on the concrete type)

## Risks

- If future code wants to swap scorers at runtime (e.g. from the CLI), the
  concrete field would require a generic `Search` or an enum of scorers.  The
  current code does not need runtime swapping, so this is acceptable.
- A generic `Search` can bloat compile times and code size.  The concrete field
  avoids that.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
```

All outcomes and PVs must be identical.  The compiler should be able to inline
`StaticAtomicScorer::score` into `sort_moves`.

## Final task

Write `docs/plans/speed/report2.md` summarising the change and whether it made a
measurable difference on a few sample FENs.
