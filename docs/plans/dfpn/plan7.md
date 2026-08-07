# Plan: Remove GHI Twin/Simulation Code and Use the First-Player-Loss Shortcut

## Summary

The solver currently keeps path-dependent "twin" transposition-table entries and runs Kawano-style simulation to verify them. This is only a pragmatic approximation of the full Kishimoto & Müller GHI fix, adds a lot of complexity, and makes proof-tree reconstruction path-sensitive (`docs/plans/storage/prompt.md` lists the twin entries as a source of incomplete proof-tree dumps).

The paper shows that the first-player-loss case can be handled without twins by **not storing disproofs that are caused by repetitions**. Since this solver treats a repeated position as `Outcome::Draw` (a disproof of a win for the attacker), and since it does not use a "player who repeats loses" rule, the relevant GHI scenario is exactly first-player-loss. Current-player-loss is irrelevant.

This plan removes:

- `TwinEntry` and the twin array inside `TtEntry`.
- `path_code` / `path_length` hashing (`zobrist::path_random`).
- `src/search/dfpn/simulate.rs`.
- All twin statistics/examples.

It keeps:

- The `path_stack` and `Position::repetition_key()` local-repetition detector.
- The `rule50` component in the main TT key.
- Path-independent base TT entries.

The only behavioral change is that a solved `Outcome::Draw` whose proof depended on a local repetition is no longer cached as a solved result; it is stored as an unsolved `(1, 1)` entry so the next search re-expands it and sees the local `Draw` correctly.

## Goal and Scope

### Goal

1. Eliminate path-dependent transposition-table entries and cross-path verification.
2. Keep the solver correct on all existing tests, including the cyclic GHI regression tests.
3. Make the proof-tree dump and PV extraction depend only on path-independent base TT entries.
4. Reduce the per-entry TT size and the amount of GHI-specific code.

### Non-goals

- No change to the repetition rule (still a draw, not a loss for the repeating side).
- No change to DF-PN thresholds, move ordering, history/killer heuristics, or iterative refinement.
- No new heuristic to recover performance on cyclic drawn positions; the first implementation accepts that cyclic draws may be slower.
- No rewrite of `Position`, `Outcome`, `Board`, or move generation.
- No change to the proof-tree worker/binary dump format beyond receiving path-independent data.

## Background

Kishimoto & Müller classify GHI into two scenarios:

> **first-player-loss:** A repetition is a loss for the first player (the player to move at the root). In checkmating problems a repetition is a draw, which is a disproof for the attacker. The paper says: "In the first-player-loss scenario, the GHI problem only causes invalid disproofs (first-player losses). Programs can avoid the GHI problem, accepting a loss of performance, by not storing any disproofs caused by repetitions." <ref_snippet file="/workspace/atomic_solver/docs/plans/dfpn/ghi.pdf" lines="1-1" />

> **current-player-loss:** A repetition is a loss for the player who repeats the position (Go situational super-ko). "This scenario does not occur in checkmating problems where only one player's king is under attack." <ref_snippet file="/workspace/atomic_solver/docs/plans/dfpn/ghi.pdf" lines="1-1" />

The current code stores a repeated board as `Outcome::Draw` in `Search::path_contains` <ref_snippet file="/workspace/atomic_solver/src/search/dfpn/core.rs" lines="99-101" />. That is a disproof for the root attacker, so we are in the first-player-loss case. The `Outcome::Draw` values that come from `path_contains` are the only disproofs caused by repetitions; stalemate, 50-move, and two-piece draws are path-independent and can still be cached safely.

`rule50` is part of the main TT key while `repetition_key()` is board-only, so exact repetitions in the current path already get a different TT key from their first occurrence while the path set still catches them <ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="151-155" /> <ref_snippet file="/workspace/atomic_solver/src/zobrist.rs" lines="79-81" />.

## Detailed Changes

### 1. `src/search/tt/entry.rs` — simplify `TtEntry`

- Remove `TwinEntry`, `MAX_TWINS`, the `twins: [TwinEntry; MAX_TWINS]` field, and all twin methods (`store_twin`, `clear_twins`, `reinit_base_for_twin`, `live_twin_count`).
- Remove `repetition_seen` from `TtEntry` and `TtSummary`; every solved result stored in the base entry is now path-independent by construction.
- Keep `EntryResult { best_move, depth }`.
- Replace the path-code lookup methods with simple base-only helpers:
  - `result_for(expected: Outcome) -> Option<EntryResult>`
  - `result_for_depth(expected: Outcome, remaining: u32) -> Option<EntryResult>`
  - `best_result() -> Option<(Move, Outcome, u32)>` (for PV extraction when `expected` is `None`)
- Remove `find_result_for_path`, `find_result_for_path_with_depth`, and `best_result_for_path`.

### 2. `src/search/tt/table.rs` — remove twin machinery

- Remove `twin_insertions`, `twin_evictions`, `peak_twins` fields and the `twin_stats`, `peak_twins`, `record_twin_action`, and `store_twin` methods.
- Simplify `store` signature to:

  ```rust
  pub fn store(
      &mut self,
      key: u64,
      best_move: Move,
      best_child: u8,
      work: u64,
      outcome: Option<Outcome>,
      pn: u64,
      dn: u64,
      depth: u32,
      remaining_depth: u32,
  )
  ```

- `probe_best_move` no longer takes a `path_code`. It returns `best_move` when `entry.outcome` is `Some` or when `entry.outcome` is `None` and `best_move` is already known.
- Update `insert_new` scoring: `(live, solved, work, generation)` without twins.
- Update `TtEntry` size test to assert `<= 128` bytes (or whatever the new layout actually is).

### 3. `src/search/tt/mod.rs` — clean up public exports

- Stop re-exporting `MAX_TWINS` and `TwinEntry`.
- Keep `EntryResult`, `TtEntry`, `TranspositionTable`.

### 4. `src/zobrist.rs` — remove path codes

- Remove `path_random`, `MAX_PATH_DEPTH`, and all tests that exercise path-code distinctness.
- Keep `rule50_key`, `hash(board, rule50)`, and `board_hash(board)`.

### 5. `src/search/dfpn/mod.rs` — drop `path_code` and twin accessors

- Remove the `path_code: u64` field from `Search`.
- Change `prefix_path` to `Option<Vec<u64>>` (only the repetition keys are needed for `verify_ppv`).
- Remove `Search::twin_stats` and `Search::peak_twins`.
- Remove `mod simulate`.
- Update `begin_run` and `reset_search_state` to initialize `path_stack` from `prefix_path` only.
- Update `search_depth_with_prefix` signature to:

  ```rust
  pub fn search_depth_with_prefix(
      &mut self,
      pos: &mut Position,
      max_depth: u32,
      prefix_keys: &[u64],
  ) -> (Outcome, u32, u64)
  ```

### 6. `src/search/dfpn/core.rs` — first-player-loss store rule and `try_use_tt`

- Move the `path_contains` check before `try_use_tt` so a repeated board always short-circuits to `Draw`:

  ```rust
  if self.path_contains(rep_key) {
      return Outcome::Draw;
  }
  if let Some(resolved) = self.try_use_tt(pos, tt_key, max_depth) { ... }
  ```

- Simplify `try_use_tt`:

  ```rust
  fn try_use_tt(&self, pos: &Position, key: u64, max_depth: u32) -> Option<Resolved>
  ```

  It returns a solved base result when `entry.outcome.is_some()`, `entry.remaining_depth >= max_depth`, and `entry.depth <= max_depth`. No simulation, no path codes. `Resolved` can drop `repetition_seen` (always `false`).
- **Cheap one-ply safety net.** Before trusting a solved base result, check whether its `best_move` would immediately lead to a board already on `path_stack`:

  ```rust
  if let Some(mv) = entry.best_move.as_non_none() {
      let mut child = pos.clone();
      child.do_move(mv);
      if self.path_stack.contains(&child.repetition_key()) {
          return None;
      }
  }
  ```

  If the stored winning (or losing) move repeats a position in the current path, the result is invalid for this path; fall back to search. This catches the obvious cross-path theta case without keeping the full simulation machinery.
- Remove `zobrist::path_random` updates around the recursive `dfpn` call.
- At store time, suppress repetition-dependent draws:

  ```rust
  let suppress_draw = outcome_to_store == Some(Outcome::Draw) && outcome_to_store_repetition_seen;
  let store_outcome = if suppress_draw { None } else { outcome_to_store };
  let (store_pn, store_dn) = if suppress_draw {
      (1, 1)
  } else if outcome_to_store.is_some() {
      (outcome_to_store_pn, outcome_to_store_dn)
  } else {
      (pn.max(1), dn.max(1))
  };
  ```

  This is the first-player-loss shortcut: a `Draw` that only holds because of a repetition in the current path is not cached as a solved draw, preventing it from being reused on a different path.
- Update all `tt.store` calls to the new signature (no `path_code`, `path_length`, `repetition_seen`).

### 7. `src/search/dfpn/children.rs` — remove path-code plumbing

- Remove `child_path_code` and `child_path_length` computation.
- Call `try_use_tt` without path arguments.
- `TtSummary` no longer has `repetition_seen`, so set `ChildInfo.repetition_seen` to `false` for unsolved TT summaries.
- Keep `ChildInfo.repetition_seen` for local repetitions and selection tie-breaking.

### 8. `src/search/dfpn/selection.rs` — tighten `repetition_seen`

- A solved `Win` or `Loss` cannot depend on a repetition, so their `repetition_seen` should be `false`.
- A solved `Draw` may depend on a repetition; use the selected draw child's `repetition_seen`.
- Update `select_child_with_early_exit` and `select_from_children` accordingly. This prevents over-suppressing path-independent wins and losses.

### 9. `src/search/dfpn/pv.rs` — follow base entries only

- Replace `extract_pv_internal` path-code logic with base-only lookups:
  - If `expected` is `None`, infer it from `entry.best_result()`.
  - If `expected_depth` is `Some(remaining)`, try `entry.result_for_depth(expected, remaining)` first, then fall back to `entry.result_for(expected)`.
- Replace `emit_proof_subtree` path-code logic with base-only lookups:
  - For `Outcome::Win`, follow `entry.result_for(Outcome::Win).best_move`.
  - For `Outcome::Loss`, iterate all legal moves and require each child to have `entry.result_for(Outcome::Win)`.
- Remove `extract_pv_follows_path_dependent_twin_entries` test and any test relying on `zobrist::path_random`.

### 10. `examples/verify_ppv.rs` — drop path-code computation

- Remove the `path_codes` vector and `zobrist` import.
- Call `search.search_depth_with_prefix(child, next_remaining as u32, &prefix_keys)` with only the prefix keys.

### 11. `examples/twin_stats.rs` — delete

- This example no longer makes sense without twins; remove it.

### 12. Tests

- `src/search/tt/tests.rs`: remove or rewrite twin-focused tests (`twin_metrics_track_insertions_and_evictions`, `clear_resets_twin_stats`, `peak_twins_tracked`, `find_and_best_result_for_multiple_paths`). Add tests that verify base-only storage, overwriting, and new-generation behavior.
- `src/search/dfpn/tests.rs`: remove all `simulate_*` and `try_use_tt_*` tests that depend on twins/simulation. Add a small test that a local repetition returns `Outcome::Draw` and that a `Draw` caused by repetition is not stored as a solved TT result.
- `tests/test_ghi.rs`: keep `promotion_transposition_outcome_is_consistent`, `cyclic_rook_position_does_not_claim_win`, and `reversible_cycle_does_not_claim_win`. Remove the empty `cross_path_repetition_dependent_win_is_not_reused` placeholder (or replace it with a test that checks the new behavior, e.g. that the solver still returns `Draw` after a repeated board and does not claim `Win`).
- `tests/test_twin_capacity.rs`: rename to `tests/test_transpositions.rs` (the file name becomes the crate module name). It actually checks transposition performance, not twins.
- `tests/verify_ppv.rs`: no changes unless the example binary interface changes; it should continue to pass.

## Verification

After implementation, run:

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --release
$ cargo test --release --test test_ghi -- --ignored
$ cargo test --release --test test_transpositions
$ cargo test --release --example verify_ppv           # or run the example manually
$ cargo doc --no-deps
```

Additional manual checks:

```text
$ cargo run --release -- --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
outcome: win
pv: f1f7 e8d8 g1g8

$ cargo run --release -- --fen "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1"
outcome: win
pv: a7a8q e8d7 b7b8q d7e6 b8e5 e6d7 e5d6

$ cargo run --release -- --fen "8/8/8/8/2k5/8/8/4KR2 w - - 0 1" --first-outcome --timeout 10
# may still time out; the important property is that the solver does NOT print "outcome: win"
```

## Risks

- **Cyclic drawn positions will re-search more.** The cyclic rook safe-area FEN currently uses 115 twin insertions to cache draw results. Without twins it will be slower and may exhaust the default 5 s budget before producing a definite result. This is acceptable because the goal is correctness/simplicity, but the CLI output may show `timeout` more often on these positions.
- **Cross-path wins.** The first-player-loss shortcut is safe for invalid *disproofs*, but cross-path *wins* that depend on repetition rights are the case where full simulation would be needed. The Kishimoto & Müller paper argues this cannot happen in the first-player-loss case. A one-ply guard in `try_use_tt` catches the immediate repetition case; deeper proof-tree repetitions are still trusted to the first-player-loss theorem. If a concrete atomic-chess counter-example is found later, a bounded fresh-`dfpn` fallback can be added in `try_use_tt` without reintroducing the full twin table.
- **Regression in shortest-PV refinement.** PV extraction now relies solely on base entries. Transposition-heavy wins must still extract the shortest PV correctly; the existing `test_review.rs` shortest-PV tests cover this.
- **Proof-tree completeness.** Removing twins means every `NodeProven` event and the final proof-tree dump will be built from path-independent results. This should fix the "path-dependent / Kawano twins are not expanded" issue in `docs/plans/storage/prompt.md`, but the dump must be verified on a few solved positions.

## Final Task

Write `docs/plans/dfpn/report7.md` documenting:

1. Which files were changed and how the twin/simulation code was removed.
2. The new first-player-loss store rule.
3. Verification results (`cargo test`, `cargo clippy`, `cargo doc`, CLI output on key positions).
4. Any positions that became slower or timed out.
5. Remaining open questions (e.g. whether a concrete atomic-chess cross-path win ever exists, and whether a bounded fallback should be added later).
