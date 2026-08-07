# Report: Remove GHI Twin/Simulation Code and Use the First-Player-Loss Shortcut

## Summary

Implemented `docs/plans/dfpn/plan7.md`. The twin table, Kawano-style simulation, and path-code hashing have been removed from the DF-PN solver. Repetition handling now relies on:

1. The existing `path_stack` of `Position::repetition_key()` values for local-cycle detection.
2. The `rule50` component in the main TT key so exact repetitions have a different hash from their first occurrence.
3. A first-player-loss store rule: a solved `Outcome::Draw` that depends on a repetition is stored as an unsolved `(1, 1)` entry instead of a solved result.
4. A cheap one-ply guard in `try_use_tt` that rejects a cached solved result when its stored `best_move` would immediately reach a board on the current search path.

All verification passes, including the previously ignored cyclic GHI regression tests, and the proof-tree dump and PV extraction now use only path-independent base TT entries.

## Changes Made

### Removed

- `src/search/dfpn/simulate.rs` — deleted entirely.
- `TwinEntry`, `MAX_TWINS`, and the `twins` array in `TtEntry` (`src/search/tt/entry.rs`).
- `repetition_seen` from `TtEntry` and `TtSummary`.
- `path_code`, `path_length`, and `zobrist::path_random` (`src/zobrist.rs`).
- `Search::twin_stats()` and `Search::peak_twins()`.
- `examples/twin_stats.rs`.
- `tests/test_twin_capacity.rs` (renamed to `tests/test_transpositions.rs`).

### Modified

- `src/search/tt/entry.rs` — simplified `TtEntry` to base-only fields and added `result_for`, `result_for_depth`, and `best_result` helpers.
- `src/search/tt/table.rs` — removed twin accounting and `store_twin`; `store` now takes only base fields.
- `src/search/tt/mod.rs` — exports `EntryResult`, `TtEntry`, `TtSummary`, `TranspositionTable`.
- `src/search/tt/tests.rs` — rewritten to test base-only storage, overwrite behavior, and generation handling.
- `src/search/dfpn/mod.rs` — removed `mod simulate`, `path_code`, and twin accessors; `prefix_path` now holds only repetition keys.
- `src/search/dfpn/core.rs` —
  - `try_use_tt` now checks a solved result's `best_move` against `path_stack` (one-ply guard).
  - Store logic suppresses repetition-dependent draws as unsolved `(1, 1)` entries.
  - `path_contains` is checked before `try_use_tt` at the start of `dfpn`.
  - Recursive `dfpn` no longer updates a `path_code`.
- `src/search/dfpn/children.rs` — removed path-code plumbing; `evaluate_child` sets `repetition_seen` only for local repetitions; moved `select_from_children`, `selection_for_child`, and `second_best_unsolved_excluding` here.
- `src/search/dfpn/selection.rs` — `repetition_seen` is now `false` for solved `Win`/`Loss` and uses the selected draw child for `Draw`.
- `src/search/dfpn/pv.rs` — PV extraction and proof-tree emission follow base TT entries only; `emit_proof_subtree` now sends each node after its children have been verified.
- `src/search/dfpn/tests.rs` — replaced twin/simulation tests with a local-repetition test and a `try_use_tt` one-ply-guard test.
- `examples/verify_ppv.rs` — no longer computes path codes; uses the simplified `search_depth_with_prefix` signature.
- `AGENTS.md` — updated architecture and example descriptions to remove twins and path codes.

## First-Player-Loss Store Rule

In `src/search/dfpn/core.rs`:

```rust
let suppress_draw = outcome_to_store == Some(Outcome::Draw)
    && outcome_to_store_repetition_seen;
let store_outcome = if suppress_draw { None } else { outcome_to_store };
let (store_pn, store_dn) = if suppress_draw {
    (1, 1)
} else if outcome_to_store.is_some() {
    (outcome_to_store_pn, outcome_to_store_dn)
} else {
    (pn.max(1), dn.max(1))
};
```

If the search proves a node is a `Draw` only because a child repeats a board on the current path, the result is not cached as a solved draw. The next search that reaches the same `tt_key` re-expands it, and the local `path_contains` check still returns `Draw` whenever the board is on the stack. Path-independent draws (stalemate, 50-move, two-piece) are still cached normally.

## One-Ply Repetition Guard in `try_use_tt`

```rust
if entry.best_move != Move::NONE {
    let mut child = pos.clone();
    child.do_move(entry.best_move);
    if self.path_stack.contains(&child.repetition_key()) {
        return None;
    }
}
```

This catches the obvious cross-path theta case: a cached win whose first move reaches a position already on the search path is rejected, forcing a fresh search. It is not a full simulation, so deeper proof-tree repetitions are still trusted to the Kishimoto & Müller first-player-loss argument.

## Proof-Tree Emission Fix

The initial base-only rewrite made `emit_proof_subtree` too strict: it sent each node only after *all* recursive children succeeded and propagated `None` for any missing TT entry. On deep or timed-out searches this produced a proof tree with a single root node because one side branch could not be fully reconstructed from the TT.

`emit_proof_subtree` in `src/search/dfpn/pv.rs` now:

1. Sends a `NodeProven` event as soon as a solved TT entry with the expected outcome is found.
2. Uses the cached `depth` from that entry for the node's depth.
3. For `Win` (OR) nodes, expands the cached `best_move` child if available but does not fail the whole tree if the child cannot be reconstructed.
4. For `Loss` (AND) nodes, attempts every legal defender reply and continues with the successful ones; missing side branches no longer prevent the parent `Loss` node from being emitted.

This restores the best-effort, partial-proof-tree behavior that existed before the twin removal while still keeping the principal variation intact.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --release
$ cargo test --release --test test_ghi -- --ignored
$ cargo test --release --test test_transpositions
$ cargo doc --no-deps
```

All passed. Selected CLI sanity checks:

```text
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1" --timeout 10
outcome: win length: 3
pv: f1f7 e8d8 g1g8
pre_exit: reason=Complete outcome=win nodes=26
proof_tree: nodes=4 win=2 loss=2 root_depth=3
proof_tree_ppv: f1f7 e8d8 g1g8
ppv_valid: true

$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1" --timeout 10
outcome: win length: 15
outcome: win length: 11
outcome: win length: 9
outcome: win length: 7
pv: b7b8r e8f7 b8b6 f7e8 b6b7 e8d8 a7a8q
pre_exit: reason=Complete outcome=win nodes=240728
proof_tree: nodes=104 win=52 loss=52 root_depth=7
proof_tree_ppv: b7b8r e8f7 b8b6 f7e8 b6b7 e8d8 a7a8q
ppv_valid: true

$ cargo run --release -- --fen "8/8/8/8/2k5/8/8/4KR2 w - - 0 1" --timeout 10 --first-outcome
timeout
pre_exit: reason=Timeout outcome=draw nodes=10258798

$ cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23" --timeout 10
timeout
pre_exit: reason=Timeout outcome=win nodes=2440024
proof_tree: nodes=14283 win=7142 loss=7141 root_depth=17
proof_tree_ppv: g1e1 f4e3 e1e7 e8a8 b1b8 a8b8 e7e8 g8h7 e8b8 c6c5 b8h8 h7g7 h8g8 g7f7 g8g6
ppv_valid: true
```

The cyclic rook-safe-area position now times out with `outcome=draw` and, crucially, does **not** claim a win. The ignored GHI regression tests also pass:

```text
$ cargo test --release --test test_ghi -- --ignored
running 2 tests
test cyclic_rook_position_does_not_claim_win ... ok
test reversible_cycle_does_not_claim_win ... ok
```

The `test_transpositions` integration test still finds the two-rook mate in well under 10,000 nodes, confirming that base-only TT entries still reuse transpositions effectively.

## Performance Observations

- `TtEntry` is now smaller (unit test asserts `<= 128` bytes; previously `<= 512` bytes), so the default 64 MB TT holds more entries.
- Decisive wins without cycles are unchanged or slightly faster because the simpler code path avoids twin bookkeeping and simulation.
- The cyclic rook position (`8/8/8/8/2k5/8/8/4KR2 w - - 0 1`) now exhausts a 10-second budget without a result. This is the expected performance cost of not caching repetition-dependent draws. It does not return a false win, which was the correctness goal.

## Risks and Open Questions

1. **Cross-path wins beyond one ply.** The one-ply guard catches the immediate repetition case, but it does not simulate the entire proof tree. The Kishimoto & Müller first-player-loss theorem says deeper cross-path wins cannot occur in this setting, but no concrete atomic-chess counter-example has been found or added as a regression test. If one is discovered, a bounded fresh-`dfpn` fallback in `try_use_tt` is the recommended next step.

2. **Cyclic drawn positions are slower.** Without twin draws, the solver re-explores cyclic endgames each chunk. This is acceptable for a correctness-first simplification, but a future optimization could add a separate per-search repetition cache (not the transposition table) if needed.

3. **`docs/plans/storage/prompt.md`** was present in the working tree before this change and is unrelated to the GHI work; it was not modified by this implementation.

## Files Changed

```text
 AGENTS.md
 docs/plans/dfpn/plan7.md  (new, later updated with the one-ply guard decision)
 docs/plans/dfpn/report7.md (this file)
 examples/twin_stats.rs    (deleted)
 examples/verify_ppv.rs
 src/search/dfpn/children.rs
 src/search/dfpn/core.rs
 src/search/dfpn/mod.rs
 src/search/dfpn/pv.rs
 src/search/dfpn/selection.rs
 src/search/dfpn/simulate.rs (deleted)
 src/search/dfpn/tests.rs
 src/search/tt/entry.rs
 src/search/tt/mod.rs
 src/search/tt/table.rs
 src/search/tt/tests.rs
 src/zobrist.rs
 tests/test_ghi.rs
 tests/test_twin_capacity.rs (deleted, replaced by tests/test_transpositions.rs)
 tests/test_transpositions.rs (new)
```
