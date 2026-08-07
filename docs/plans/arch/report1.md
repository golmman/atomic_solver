# Implementation Report: Decouple search and proof tree with `ProofEvent`

## Summary

Implemented the `proof_event` protocol module and rewired `search` and
`proof_tree` so that `search` no longer depends on `proof_tree`. The search
now emits neutral `ProofEvent` messages; the proof-tree worker consumes them
and maintains the in-memory tree. A `ProofTreeWorkerHandle` replaces the
previous `Sender<ProofMessage>` API.

## Design choices confirmed and changed

### Confirmed

- `src/proof_event.rs` owns the search-to-worker contract with `Clear` and
  `NodeProven(Vec<Move>, Outcome, u32)`.
- `NodeProven::new` derives `mv` from the last move of `path` (or
  `Move::NONE` for the root), so the worker does not need to re-derive it.
- `proof_tree` keeps its `HashMap<String, usize>` index; `Move` is not `Hash`.
- UCI path-string construction moved to `notation::moves_to_uci_path` and
  is used by the worker, not the search.

### Changed

- The worker loop uses `std::sync::mpsc::recv_timeout(Duration::from_millis(1))`
  on the event channel and `try_recv` on the query channel. This was chosen
  over a single combined channel because search events (`ProofEvent`) and
  worker-control queries (`GetStats`/`GetTree`) have different lifetimes and
  callers. The small periodic wakeup is acceptable for the CLI.
- `ProofTreeWorkerHandle` exposes `event_sender()`, `stats()`, and `tree()`.
  `spawn` returns `(ProofTreeWorkerHandle, JoinHandle<()>)`.
- `ProofTreeWorker` methods `run`, `handle_event`, and `handle_query` are
  private to the `worker` module; only `ProofTreeWorkerHandle` and the
  crate-internal unit tests touch them.
- Memory accounting in the worker now tracks `pending_move_bytes` separately
  from `pending_path_bytes` so the `Vec<Move>` heap capacity for pending
  events is included in the estimate.
- `ProofTree::add_node` now takes the full UCI path string directly.

## Problems encountered

### Two-channel worker loop

`std::sync::mpsc` has no `select`, so the worker must poll. `recv_timeout` on
one channel with `try_recv` on the other keeps event throughput high while
still allowing queries. The trade-off is a query latency of up to ~1 ms and
a periodic wakeup every 1 ms when idle. No deadlocks or dropped-sender issues
were observed in testing.

### Public API churn

`Search::set_proof_tree_sender` and `ProofMessage` were removed from the
search API. Call sites in `main.rs`, `tests/test_proof_tree.rs`, and the
worker tests were updated. No example binaries required changes, because
only `inspect_pt` touches the proof tree and it reads `proof_tree.bin`.

### `emit_proof_tree` test placement

The original `emit_proof_tree_populates_validate_ppv` unit test lived in
`search/dfpn/pv.rs` and imported `ProofTreeWorkerHandle`, which reintroduced a
`search -> proof_tree` test-only dependency. The test was removed; equivalent
coverage is now provided by `solve_populates_proof_tree_with_nodes` in
`proof_tree/worker/tests.rs`, which asserts `tree.validate_ppv(&pv)` after a
full solve.

## Test results

```text
$ cargo fmt
$ cargo clippy --all-targets   # clean
$ cargo test --all-targets     # all passing
$ cargo test --release --all-targets  # all passing
$ cargo doc --no-deps
```

Specific checks:

- `search` no longer contains `use crate::proof_tree`.
- `search` no longer calls `move_to_uci`.
- `proof_tree` consumes `ProofEvent` from `proof_event`.
- `proof_tree.bin` round-trip still works (`inspect_pt` and `test_proof_tree`).
- CLI output for `4k3/8/8/8/8/8/8/4KRR1 w - - 0 1`:

```text
outcome: win length: 3
pv: f1f7 e8d8 g1g8
pre_exit: reason=Complete outcome=win nodes=26
proof_tree: nodes=4 win=2 loss=2 root_depth=3
proof_tree_dump: proof_tree.bin
```

- `inspect_pt proof_tree.bin` shows 4 nodes, `extract_ppv: f1f7 e8d8 g1g8`,
  and `validate_ppv: true`.
- `verify_ppv` with the full PPV returns `is_ppv: true`.

### Performance snapshot

`cargo run --release --example benchmark -- --timeout 1 --runs 3` after the
change:

| name | outcome | nodes | child_evals | mean (s) | pv_len |
|------|---------|------:|------------:|---------:|-------:|
| two_rook_mate | win | 26 | 73 | 0.000 | 3 |
| epsilon_mate | win | 50826 | 136387 | 0.033 | 5 |
| promotion_transposition | win | 240728 | 543684 | 0.096 | 7 |
| m26 | win | 2262730 | 5005150 | 1.000 | 7 |
| opening_f2 | win | 1322948 | 4067002 | 1.000 | 7 |
| rook_pawn_endgame | win | 1497884 | 3530575 | 0.711 | 7 |
| m19 | draw | 185952 | 3891492 | 1.000 | 0 |
| startpos | draw | 159065 | 3723714 | 1.000 | 0 |

A pre-change baseline was not captured, but node counts and PV lengths for
known decisive positions match the existing test expectations and no
regressions are visible in the benchmark suite.

## File-size notes

- `src/proof_tree/worker.rs` grew to ~11 KiB because it now contains both the
  worker state machine and the `ProofTreeWorkerHandle`. A header comment
  documents why the file exceeds the 10 KiB soft limit.
- `src/proof_tree/worker/tests.rs` is now under 10 KiB after rewriting tests.
- `src/search/dfpn/pv.rs` shrank slightly after removing UCI path formatting.
- No new files exceed the 20 KiB hard submodule limit.

## Unresolved parts and missing tests

- `ProofSink` trait is still a stretch goal. The search currently holds
  `Option<Sender<ProofEvent>>`; a trait would decouple it further.
- No dedicated stress test for the two-channel worker loop under very high
  event rates. The benchmark and existing tests exercise it, but a focused
  concurrent test could be added.
- `moves_to_uci_path` has no standalone unit test; it is covered indirectly
  by worker tests and the CLI round-trip.

## Next steps

1. **Add `ProofSink`.** Define a trait in `proof_event` so `Search` can hold
   `Option<Box<dyn ProofSink>>` and unit tests can collect events into a
   `Vec` without spawning a thread.
2. **Externalize the proof tree.** With `ProofEvent` as a stable protocol, the
   worker could move to a separate crate or binary.
3. **Optimize the `ProofTree` index.** If `Move` becomes `Hash`/`Ord` or a
   stable `u16` move code is introduced, switch to a `HashMap<Vec<Move>, usize>`
   or a compact bit-packed key to avoid UCI string formatting.
4. **Consider `crossbeam-channel`** if the 1 ms worker polling latency becomes
   a bottleneck.
