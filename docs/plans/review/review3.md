# Correctness Review 3: `atomic_solver` after Plans 9–14

Date: 2026-07-17  
Scope: `src/{position,zobrist,search/{dfpn,tt,ordering},main}.rs`, with reference to `docs/plans/dfpn/research_epsilon.md`, `research_ghi.md`, and `research_parallel.md`, and the implementation reports `docs/plans/review/report{9..14}.md` and earlier reviews `review1.md` and `review2.md`.

## Executive summary

Plans 9–14 successfully close the issues identified in `review2.md`:

- `ε = 0.0` now behaves like classic `+1` DF-PN (Plan 9).
- Terminal detection is centralized in `Position::outcome_from_state` (Plan 10).
- `simulate` uses the twin's original `path_code` and `path_length`, seeded with the current search prefix (Plan 11).
- The transposition table and in-memory repetition set use different keys (Plan 12).
- A dedicated GHI regression suite is in place (Plan 13).
- Twin capacity is instrumented and the fixed eight-slot FIFO is sufficient for the tested positions (Plan 14).

`cargo fmt`, `cargo clippy --all-targets`, `cargo test --all-targets`, and `cargo doc --no-deps` all pass. The ignored release tests in `test_ghi` and `test_epsilon` also pass.

However, three new correctness gaps have appeared (or were not caught by the earlier reviews):

1. **`Position::outcome_from_state` checks `rule50 >= 100` and the two-piece draw heuristic before the no-legal-moves checkmate/stalemate test.** This misclassifies 50-move checkmates and some two-piece checkmates as draws.
2. **`dfpn` stores a *solved* `Outcome::Draw` when `max_depth == 0`.** A depth-limited cutoff is therefore treated as a proven draw in later searches with a larger remaining depth, which is unsound for `search_depth` and for shortest-PV refinement with transpositions.
3. **`extract_pv` recomputes path codes with the wrong move-depth index.** It uses `pv.len()` (0-indexed) instead of `pv.len() + 1` (1-indexed, matching `dfpn`), so it cannot follow path-dependent twin entries and can return an empty or truncated PV for wins that are stored as twins.

The GHI simulation is also still a pragmatic approximation: it is sound for finite proof trees and for cycles that are also present in the current search prefix, but it can accept a twin whose proof relies on a repetition that was legal in the twin's original path but is not an ancestor in the current path.

---

## 1. What is implemented correctly

### 1.1 `1 + ε` threshold (Plan 9)

`epsilon_ceil` in <ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="750-756" /> enforces a minimum step of `x + 1`, so `ε = 0.0` reproduces classic `+1` DF-PN. `set_epsilon` keeps the `[0.0, 1.0]` range, and `tests/test_epsilon.rs` confirms `ε = 0.0` solves the mate-in-two regression. This matches the updated `research_epsilon.md`.

### 1.2 Centralized terminal detection (Plan 10)

`Position::outcome_from_state` in <ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="104-127" /> is now the single terminal detector used by `dfpn`, `validate_pv`, and `simulate`. The helper correctly uses `state.checkers` to distinguish checkmate from stalemate when the move list is empty. (The *ordering* of the checks is wrong — see section 2.1 — but the helper itself is in the right place.)

### 1.3 Path-code-aware GHI simulation (Plan 11)

`TwinEntry` now carries `path_length` (<ref_snippet file="/workspace/atomic_solver/src/search/tt.rs" lines="10-16" />), and `try_use_tt` passes `twin.path_code` and `twin.path_length` to `simulate` (<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="599-609" />). `simulate` computes child path codes with the same 1-indexed depth used by `dfpn` (<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="665-675" />), so it can follow the path-dependent twin entries that make up a stored proof tree.

### 1.4 Separate TT and repetition keys (Plan 12)

`zobrist::board_hash` (<ref_snippet file="/workspace/atomic_solver/src/zobrist.rs" lines="110-112" />) and `Position::repetition_key` (<ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="134-136" />) provide a board-only key, while `Position::hash()` still XORs in the halfmove clock. `dfpn`, `simulate`, and `extract_pv` use `hash()` for the TT and `repetition_key()` for the in-memory `path` set and `seen` cycle set. This is consistent with `research_ghi.md` section 7.

### 1.5 GHI regression suite (Plan 13)

`tests/test_ghi.rs`, `tests/test_repetition.rs`, and the updated `tests/test_epsilon.rs` cover cross-path promotion transpositions, cyclic rook-safe-area draws, reversible move cycles with changing `rule50`, and epsilon values on cyclic positions. `src/search/tt.rs` also has unit tests for multiple twins with different path codes (<ref_snippet file="/workspace/atomic_solver/src/search/tt.rs" lines="468-500" />).

### 1.6 Twin capacity instrumentation (Plan 14)

`TranspositionTable` tracks `twin_insertions`, `twin_evictions`, and `peak_twins` (<ref_snippet file="/workspace/atomic_solver/src/search/tt.rs" lines="177-228" />). `examples/twin_stats.rs` shows at most 2 live twins per entry on the GHI positions, so the fixed `[TwinEntry; 8]` array is adequate and remains `Copy`.

### 1.7 PV validation and binary refinement

`validate_pv` verifies move legality, final outcome, and optional expected depth (<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="253-297" />). `solve_refined` performs a binary search over `[1, best_depth]` with a final validation pass (<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="156-230" />).

---

## 2. Significant correctness issues

### 2.1 `Position::outcome_from_state` misorders terminal checks

`outcome_from_state` currently evaluates terminal conditions in this order:

<ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="107-125" />

`rule50 >= 100` and `occupied().count() == 2` are tested *before* the no-legal-moves branch. This is wrong because checkmate ends the game before a 50-move draw can be claimed, and stalemate/checkmate has priority over any material-based draw heuristic.

**Reproducible examples:**

```text
$ cargo run --release -- --fen "7K/8/8/8/8/8/1Q6/k7 b - - 100 1"
outcome: draw
```

Black has no legal moves and the black commoner is attacked by the queen on b2, so this is checkmate and should be `loss` for the side to move. Because the halfmove clock is 100, it is reported as `draw`.

```text
$ cargo run --release -- --fen "8/8/8/8/8/8/1K6/k7 b - - 0 1"
outcome: draw
```

Black is in check by the white commoner on b2 and has no legal move, yet the two-piece heuristic reports `draw`.

**Fix:** Move the `moves.is_empty()` branch (using `state.checkers` to choose between `Loss` and `Draw`) above the `rule50` and two-piece checks. Only after confirming the position is not terminal by checkmate/stalemate should the draw-by-rule and material-draw heuristics apply.

Because `dfpn`, `validate_pv`, and `simulate` all rely on `outcome()`/`outcome_from_state`, this bug affects search results, PV validation, and twin simulation.

### 2.2 `max_depth == 0` cutoff is stored as a proven draw

In `dfpn`:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="338-352" />

When the remaining depth budget is exhausted at a non-terminal node, the code returns `Outcome::Draw` and stores it in the TT with `depth = 0` and `outcome = Some(Draw)`. `try_use_tt` then treats any base entry with a solved `outcome` and `entry.depth <= max_depth` as a final result:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="561-570" />

A node that was *not* solved within a small depth bound is therefore reused as a *proven* draw when the same node is reached later with a larger remaining depth. This is unsound for:

- `search_depth`, which does not clear the TT between calls.
- `solve_refined`, where the same node can be reached via paths of different lengths within a single depth-bounded probe.

**Consequences:**

- `search_depth(0)` followed by `search_depth(larger)` can incorrectly return `Draw`.
- The binary search in `solve_refined` may treat a transposition-induced cutoff as a true draw and converge to a non-minimal depth.
- Shortest-PV refinement is not guaranteed to return the shortest win on positions where transpositions cause a `max_depth == 0` cutoff before the winning line is discovered.

**Fix:** Do not store a depth cutoff as a solved `Outcome::Draw`. Options:

- Store `outcome = None` with a flag or a separate `max_depth` field indicating the result is valid only up to that remaining depth.
- Include the remaining `max_depth` in the TT lookup so that a cutoff for one remaining-depth budget is never reused for another.
- For depth-bounded searches, do not overwrite a base result with a cutoff draw; keep the cutoff as a separate, non-solved bound.

### 2.3 `extract_pv` uses the wrong path-code depth index

`dfpn` computes a child path code with:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="494-497" />

Here `self.path_stack.len()` after the push is `current_depth + 1`, i.e. the 1-indexed move depth. `extract_pv` recomputes path codes with:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="1115-1118" />

At the first move `pv.len()` is 0, at the second move it is 1, etc. This is one less than the depth used during search. Path codes for path-independent base entries are found regardless of `path_code`, but path-dependent solved results are stored as twins keyed by the exact 1-indexed path code. `extract_pv` therefore fails to follow any twin entry and stops early, returning an empty or truncated PV.

`try_use_tt` and the search itself are not affected — they use the correct depth — but `extract_pv` is used in:

- `Search::solve` (final PV output),
- `Search::solve_refined` (`print_pv_update` and the final validation),
- `Search::extract_pv_checked`.

A win whose TT entry is a twin (which happens whenever `repetition_seen` was set anywhere in its proof tree) will not produce a valid PV. `solve_refined` also reads the root depth from `best_result_for_path(0)`, which works for the root but not for deeper twin entries during extraction.

**Fix:** Use `pv.len() + 1` as the depth index in `extract_pv`:

```rust
path_code ^= zobrist::path_random(mv, pv.len() + 1);
```

---

## 3. Other issues and observations

### 3.1 GHI simulation is still an approximation

`simulate` follows the stored proof tree using the twin's `path_code` and `path_length` and seeds `sim_path` with the current search prefix. This works when the proof is a finite tree, or when any repetition in the proof is also a repetition in the current prefix (so `sim_path` catches it).

It is not fully sound for the general cross-path case described in `research_ghi.md`: a twin may have been proven by repeating a board state that was an ancestor in the twin's original path but is not an ancestor in the current path. `sim_path` does not contain that ancestor, so the simulation may accept a draw that is not legal under the current path. The existing regression suite does not contain a position that exposes this, but the code is not a complete implementation of Kawano's cross-path verification.

### 3.2 `solve_refined` shortest-PV guarantee is weakened

Because of the `max_depth == 0` cutoff bug (section 2.2), the binary search can be misled by transpositions. The final validation at the converged `lo` re-searches with a clear TT, so the *outcome* is correct, but `lo` may be larger than the true shortest winning distance. The returned PV is therefore not guaranteed to be the shortest possible on transposition-heavy positions.

### 3.3 CLI output is duplicated

`Search::print_pv_update` and `main` both print `outcome:` and `pv:`. For draws the PV line is empty. This is existing behavior and does not affect correctness, but it is confusing.

### 3.4 `Position::outcome()` always generates legal moves

This is the intended centralized API, but it is more expensive than the old branch-specific checks. Callers that already have a move list (`dfpn`, `validate_pv`, `simulate`) now either call `outcome_from_state` or `outcome()`. Exposing `outcome_from_state` publicly could avoid some redundant move generation.

### 3.5 `MAX_TWINS = 8` is still a fixed FIFO

Plan 14 instrumentation showed a peak of 2 live twins on the tested cyclic positions, so the capacity is currently adequate. If future positions exceed 8, the FIFO replacement will cause repeated re-solving, never a wrong result. An LRU within the fixed array would be a simple future improvement.

---

## 4. Research alignment

### `research_epsilon.md`

The implementation now matches the corrected description: `epsilon_ceil` enforces `x + 1`, `ε = 0.0` works as classic DF-PN, and the recommended starting value `0.25` is still the default. The CLI and tests expose `epsilon` at runtime.

### `research_ghi.md`

The base/twin split, path-code Zobrist encoding, base re-initialization for twins, and the separation of the `rule50`-inclusive TT key from the board-only repetition key are all implemented. The simulation follows the paper's structure but does not fully realize the cross-path verification because it does not run a bounded fresh search under the current prefix and does not track the full original ancestor set.

### `research_parallel.md`

The solver remains sequential. The OR/AND aggregation, threshold propagation, and `1 + ε` enhancement are consistent with the literature. The `1 + ε` formula is now safe for `ε = 0.0`.

---

## 5. Test status

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
$ cargo doc --no-deps
$ cargo test --release --test test_ghi -- --ignored
$ cargo test --release --test test_epsilon
```

All pass.

New manual checks that expose the ordering bug:

```text
$ cargo run --release -- --fen "7K/8/8/8/8/8/1Q6/k7 b - - 100 1"
outcome: draw

$ cargo run --release -- --fen "8/8/8/8/8/8/1K6/k7 b - - 0 1"
outcome: draw
```

Both should be `loss`.

---

## 6. Recommendations

1. **Fix `Position::outcome_from_state` ordering.** Move the `moves.is_empty()` check (with `state.checkers`) above `rule50 >= 100` and `occupied().count() == 2`.
2. **Fix `max_depth == 0` cutoff storage.** Do not store a depth-bound cutoff as a solved `Outcome::Draw`; keep it as an unsolved bound keyed to the remaining depth, or store the remaining depth explicitly.
3. **Fix `extract_pv` path-code depth.** Use `zobrist::path_random(mv, pv.len() + 1)` to match the 1-indexed depth used by `dfpn`.
4. **Strengthen GHI regression tests.** Add a position where a twin proven along one path is not valid along another because of a repetition that is legal only in the first path, or switch cross-path verification to a bounded fresh `dfpn` under the current prefix.
5. **Add regression tests for the ordering bug.** Include the two FENs above and a stalemate-with-rule50 case to lock in the correct precedence.
6. **Reconsider shortest-PV refinement for transpositions.** Either fix the depth-bound TT interaction or document `solve_refined` as a best-effort refinement that is correct on the outcome but not always minimal in PV length.
7. **Minor cleanup:** deduplicate CLI output and consider exposing `Position::outcome_from_state` to avoid redundant move generation.

---

## 7. Conclusion

Plans 9–14 leave the solver in a much stronger state: the `ε = 0.0` threshold, centralized terminal detection, path-aware GHI simulation, separate TT/repetition keys, and twin instrumentation are all in place and tested. The remaining risks are concentrated in three areas:

- **Terminal detection ordering**, which already produces wrong results on simple FENs and is the highest priority fix.
- **Depth-bound TT storage**, which undermines the soundness of `search_depth` and the shortest-PV guarantee of `solve_refined` in the presence of transpositions.
- **PV extraction for twin entries**, which can return empty or truncated PVs for wins whose proof trees are stored as path-dependent twins.

Once these three issues are fixed and the ordering bug is covered by regression tests, the core DF-PN+ solver will be correct for the tested GHI cases, with the GHI simulation remaining the main approximation for deeply cyclic proof trees.
