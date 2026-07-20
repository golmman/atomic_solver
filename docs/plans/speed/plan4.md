# Plan 4: Avoid the double `tt.probe` in `dfpn`

## Start

Read `docs/plans/speed/analysis.md`.  Open `src/search/dfpn/core.rs` and locate
the two `tt.probe(tt_key)` calls near the top of `dfpn`.

## Goal

Probe the transposition table once per node instead of twice.

## Background

At the start of `dfpn` the same `tt_key` is first probed for `try_use_tt`, then
probed again to fetch the stored best move when the entry could not be reused:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn/core.rs" lines="77-102" />

Both calls currently copy a large `TtEntry` (see Plan 7).  Even before fixing
that, the second probe is unnecessary work: the first probe already has the entry.

## Implementation tasks

1. Restructure the top of `dfpn` so that the result of the first probe is kept in
   a local variable.
2. If `try_use_tt` returns `Some(resolved)`, return early as before.
3. If `try_use_tt` returns `None`, derive `best_from_tt` from the locally stored
   entry instead of probing again.
4. Be careful with lifetimes and mutability: `try_use_tt` can call
   `tt.store_twin`, so holding a reference into the table across the call is not
   possible.  Copy or extract only the `best_move`/`outcome` fields that are
   needed before calling `try_use_tt`, or restructure `try_use_tt` to take the
   entry data it needs by value.

## File changes

- `src/search/dfpn/core.rs`
- Possibly `src/search/dfpn/simulate.rs` if `try_use_tt` is refactored to accept
  an owned `TtEntry` summary

## Risks

- `try_use_tt` may mutate the table (store a twin), so the previously probed
  entry can become stale.  The second probe was partially there to re-read after
  a possible twin store.  If we keep the entry locally, we must only use fields
  that are guaranteed stable (e.g. the `best_move` for non-resolved entries) or
  explicitly re-probe only when a twin was stored.
- Plan 7 (avoid copying the full entry) interacts with this change; consider
  doing Plan 4 together with Plan 7 or after it.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
```

All tests must pass and the `outcome`/`pv` output must be unchanged.

## Final task

Write `docs/plans/speed/report4.md` describing the refactor and whether it
reduced probe overhead.
