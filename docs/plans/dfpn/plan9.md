# Plan 9: Per-search repetition cache

Initiative: `dfpn` backlog #1. Prerequisite reading: `report7.md` (why twins
were removed), `research_ghi.md` §7/§9, `research_repetition_cache.md` (the
soundness design spike this plan implements; its §4 argument is normative for
this plan).

## Goal

Add a per-search cache of repetition-dependent draw results, outside the
transposition table, so that shuffle-heavy subtrees stop being re-walked on
every re-entry within a single search run. Primary metric: `child_evals` to
first decisive outcome on the deep-shuffle stress case. No behavior change
other than work reduction: every outcome and PV the search returns must be
one the search could have returned before this plan.

## Background (one paragraph)

The first-player-loss shortcut (report7) stores repetition-dependent draws as
unsolved `(1, 1)` TT entries, so the whole chain of draw proofs above a
repetition is uncacheable. The sizing spike (`research_repetition_cache.md`
§2) measured 31,620 re-proofs of 1,467 distinct positions on the stress case
(96% redundancy) and concluded the cost is the churn of re-descent around the
uncacheable chain. The spike's §3–§5 fix the semantics: a node's value is a
deterministic function f(P, A) of the position hash P and the *set* of
ancestor repetition keys A; caching f(P, A) = Draw keyed by exactly (P, A)
within one run cannot produce a false decisive outcome because only draws are
cached and no cross-context reuse exists.

## Soundness contract (must hold after the change)

1. Only `Outcome::Draw` entries with `repetition_seen`-dependent proofs are
   stored; wins/losses never enter or leave the cache.
2. Cache key = (position hash including rule50, order-independent multiset
   hash of the current `path_stack` repetition keys). A hit requires both to
   match exactly.
3. The cache is cleared in `Search::begin_run` (once per run; never per chunk,
   never per refinement round). It is not serialized into TT snapshots, proof
   events' stored facts, or any other artifact.
4. A cache hit behaves exactly like a freshly proven repetition-dependent
   draw: it may be returned as `Draw`, and it must emit `NodeProven` with the
   cached depth.
5. `child_eval_budget` / `ExitReason::BudgetExhausted` semantics are
   unchanged (a cache hit consumes no budget; a budget-exhausted search still
   stores only unsolved TT entries and reports `BudgetExhausted`).

## Design

New file `src/search/dfpn/repetition_cache.rs` (keep under 10 KB):

```rust
pub(super) struct RepetitionCache {
    map: HashMap<(u64, u64), u32>,   // (tt_key, context_hash) -> proven depth
    capacity: usize,
}
```

- `depth` is the only payload: the outcome is `Draw` by contract.
- `context_hash`: wrapping sum over `path_stack` of `splitmix64(key)` (or the
  existing mixing helper if one is already available; do not XOR raw keys —
  cancellation). `path_stack` currently never contains duplicates, but the
  sum stays multiset-safe if that invariant ever changes.
- `capacity`: start with `1 << 20` entries; when full, new inserts are dropped
  (deterministic, no eviction). Record the actual stress-case working set in
  the report and shrink if it is far below the cap.
- Wire as a `Search` field; add a `set_repetition_cache_capacity`-style knob
  only if a test needs it, otherwise keep it private.

Integration points in `dfpn` (`core.rs`):

- **Store**: in the `suppress_draw` branch (where the TT store is turned into
  an unsolved `(1, 1)`), also insert `(tt_key, context_hash)` with the proven
  depth. Do *not* store when the frame is returning through the early
  `path_contains` exit — those are already O(1).
- **Probe**: in `dfpn` after the existing local-repetition check and the TT
  solved-result check, before `sort_moves`: if `(tt_key, context_hash)` hits,
  emit `NodeProven(pos, Draw, cached_depth)` and return `Outcome::Draw`.
  Keep the probe *after* the TT solved-result check so non-repetition
  behavior is byte-identical.
- **Clear**: `begin_run` clears the cache (before or after `reset_search_state`
  — observe that `reset_search_state` re-seeds `path_stack` from
  `prefix_path`, so the clear must happen regardless of prefix presence).
- Context hash helper: compute at probe/store time from `path_stack`
  (O(path length)); do not maintain it incrementally across `path_push`/
  `path_pop` unless the report shows the O(depth) recompute is hot.

Out of scope, explicitly: cross-context reuse with verification (backlog #4),
clock-budget-aware TT reuse (backlog #2), any change to the Zobrist keys, any
change to what the TT stores.

## Tasks

1. Implement `repetition_cache.rs` with unit tests: insert/probe round-trip,
   capacity stop, context-hash order-independence (same keys, different order
   → same hash), distinct-clock boards mapping to distinct entries.
2. Wire into `Search`/`dfpn` per the design; add a `#[cfg(test)]` test in
   `src/search/dfpn/tests.rs` that a second bounded search over a cyclic
   position within the same `Search` instance hits the cache (node count for
   equal work chunks drops) and still returns `Draw`, never `Win`.
3. Soundness gate: `cargo test --release --test test_repetition --
   --include-ignored` (both cyclic-rook regression tests must pass; they are
   plain `#[ignore = "slow: ..."]` gated since the 2026-09-12 convention
   fix). Add one integration test to that file: solving the cyclic rook
   position with a two-step scenario (solve → reset → solve in one process)
   never yields `Outcome::Win`.
4. Drift protocol: `benchmark --suite quick --json --first-outcome` before
   and after. Cases without repetition-dependent proofs must be bit-identical.
   For any case whose `child_evals` changes (repetition-heavy ones), verify
   outcome and PV are unchanged and list the delta in the report. Run the
   move-order suite (`cargo test --release --test test_move_order --
   --include-ignored`) for regressions.
5. Deterministic-budget contract: run the budget tests
   (`cargo test --release --test test_plan6 -- --include-ignored`) and confirm
   `ExitReason::BudgetExhausted` behavior is untouched.
6. Stress measurement: `atomic_solver --fen '4r2k/3p4/2pB2p1/p4p1p/7P/
   2N1PPP1/P1PP4/1R4RK w - - 0 21' --timeout 120 --first-outcome
   --outcome-only` — record `child_evals`/`nodes`/wall time vs. the baseline
   (20,313,879 nodes / 351,297,052 child evals, first line 405 plies). Also
   run the default mode (refinement) once and record the delta. Sanity: the
   cyclic rook position must still not claim a win within the timeout.
7. Memory check: report the peak cache entry count on the stress case; shrink
   the capacity default if the working set is far below `1 << 20`.
8. Update AGENTS.md (dfpn paragraph: one sentence on the per-search
   repetition cache and its clear point) and `initiative.md` backlog #1
   status.
9. Write `report9.md` in this directory (tools/examples used, problems,
   unresolved parts, missing tests, next steps).

## Validation checklist (summary)

- `cargo fmt`, `cargo clippy --all-targets`, `cargo test --release` (fast
  gate), `cargo doc --no-deps`.
- Repetition soundness gate with `--include-ignored`.
- Quick-suite drift (bit-identical except intended deltas) + move-order suite.
- Stress-case measurement recorded against the baseline.

## Non-goals

- Twins, path codes, or Kawano-style simulation (plan5's removed mechanism).
- Cross-context or cross-run reuse of repetition-dependent results.
- Changing rule50 handling in keys or evaluation.
- Throughput work unrelated to repetition churn (that is `lean`'s mandate).
