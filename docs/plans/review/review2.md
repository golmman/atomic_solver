# Correctness Review 2: `atomic_solver` after Plans 1–8

Date: 2026-07-17  
Scope: `src/{position,zobrist,search/{dfpn,tt,ordering},main}.rs` and the implementation reports in `docs/plans/review/report{1..8}.md`, with reference to `docs/plans/dfpn/research_epsilon.md`, `research_ghi.md`, and `research_parallel.md`.

## Executive summary

The eight review plans have been implemented faithfully and the public test suite is green:

- `cargo test` passes all non-ignored tests.
- `cargo clippy --all-targets` is clean.
- `cargo doc` builds without warnings.

Terminal detection, path-code encoding, binary shortest-PV refinement, stronger PV validation, twin capacity, runtime epsilon configuration, and a regression suite are all in place. Compared with `review1.md`, the issues in sections 2.1 (no-legal-move terminals), 2.2 (path-code collisions), 3.2 (linear refinement), 3.3 (weak PV validation), and 3.6 (twin capacity) have been addressed.

Two significant correctness concerns remain:

1. **The `1 + ε` threshold implementation does not handle `ε = 0.0` safely.** With `ε = 0.0` the child threshold degenerates to the sibling's bound (`p2`) instead of the required `p2 + 1`, which makes DF-PN thrash and time out on any non-trivial position. The research note and tests treat `ε = 0.0` as "plain DF-PN", but it is not.
2. **The GHI twin/simulation mechanism is structurally incomplete.** `simulate` cannot validate terminal checkmate/stalemate positions because `Position::outcome()` does not detect no-legal-move terminals, and it probes child TT entries by the *current* path code while a twin's proof tree is stored under the *twin's* path codes. Cross-path reuse therefore only works for path-independent subtrees, largely defeating the purpose of the twin mechanism.

Several smaller issues are also noted below, most importantly that the repetition-detection set (`self.path`) uses the same Zobrist key as the TT, which includes the halfmove clock and therefore misses many real repetitions.

---

## 1. What is implemented correctly

### 1.1 Terminal detection and outcome ordering

`dfpn` now uses `legal_moves_with_state` and `state.checkers` to distinguish checkmate from stalemate when the move list is empty. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="363-386" />

`Position::outcome` now checks commoner extinction before the 50-move and two-piece draw rules. <ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="94-110" />

`outcome_from_pn_dn` correctly returns `None` for the ambiguous `(INF, 0)` pair and only recognizes `(0, INF)` as `Win`. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="1229-1235" />

### 1.2 Path-code encoding

`zobrist::path_random` encodes `(from, to, move_kind, depth)` by mixing them into a single 64-bit value and applying one `SplitMix64` round. `move_kind` distinguishes normal moves, castling, en passant, and each promotion piece. <ref_snippet file="/workspace/atomic_solver/src/zobrist.rs" lines="61-96" /> The unit tests verify that normal moves, queen promotions, other promotions, castling, and en passant with the same `from`/`to` squares produce different keys, and that move order matters. <ref_snippet file="/workspace/atomic_solver/src/zobrist.rs" lines="107-165" />

### 1.3 GHI data structures and current-path simulation seeding

`TtEntry` stores base bounds and up to eight twins. `TranspositionTable` tracks twin insertions and evictions. <ref_snippet file="/workspace/atomic_solver/src/search/tt.rs" lines="7-218" /> `try_use_tt` seeds `simulate` with the current search prefix (`sim_path = self.path.clone()`, `sim_stack = self.path_stack.clone()`) and passes the current `path_code` rather than the twin's. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="573-639" />

`simulate` now treats a repeated position as a draw and the `Outcome::Loss` branch no longer accepts an empty move list unconditionally. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="641-744" />

### 1.4 Shortest-PV refinement, validation, and runtime epsilon

`solve_refined` now performs a true binary search over `[1, best_depth]` with a final validation pass. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="147-221" />

`validate_pv` checks that every PV move is legal, that the PV length matches an expected depth if supplied, and that the terminal outcome is correct after accounting for the parity of the PV length. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="244-305" />

`Search::set_epsilon` and `main.rs` expose epsilon at runtime, validated to `[0.0, 1.0]`. <ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="75-81" /> <ref_snippet file="/workspace/atomic_solver/src/main.rs" lines="10-32" />

---

## 2. Significant correctness issues

### 2.1 `ε = 0.0` loses the `+1` threshold offset and breaks DF-PN

`epsilon_ceil` computes:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="746-752" />

For `ε = 0.0` this is `ceil(x * 1.0) = x`. The threshold passed to a child is then:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="498-514" />

The standard DF-PN formula is `new_th_pn = min(th_pn, second_pn + 1)` (see `research_epsilon.md` and `research_parallel.md` section 2.2). The multiplicative formula is intended to *replace* the `+1` additive margin, so for `ε = 0.0` it should reproduce `second_pn + 1`. Instead it produces `second_pn`.

With a threshold equal to the sibling bound, the selected child returns as soon as its `pn` reaches the bound, which is the same value as the second-best child. The parent loop then has no strict progress guarantee and can re-enter the same child with the same threshold indefinitely. The existing unit tests do not catch this because the only `ε = 0.0` test is on an immediate mate-in-one where no recursive threshold recursion is needed.

Demonstration:

```text
$ timeout 7 cargo run --release -- --epsilon 0.0 \
    --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"
outcome: draw
```

The same position with `ε = 0.01` or `ε = 0.25` solves quickly:

```text
$ timeout 6 cargo run --release -- --epsilon 0.25 \
    --fen "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3"
outcome: win
pv: h5d5 d7d6 d5f7 e8d7 f7e7
nodes: 372384
```

**Implication:** `ε = 0.0` is not a valid setting, despite `research_epsilon.md` stating that it is equivalent to the original `+1` threshold. Users who set `ε = 0.0` will get timeout / `Draw` results for any position that is not a one-ply mate.

### 2.2 `simulate` cannot validate terminal no-legal-move positions

`simulate` begins by checking `pos.outcome()`:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="657-659" />

`Position::outcome()` does **not** detect checkmate or stalemate; it only handles commoner extinction, rule50, and the two-piece heuristic. <ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="94-110" />

In the `Outcome::Loss` branch, an empty legal-move list falls back to `pos.outcome() == Some(expected)`:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="704-709" />

For a checkmate (the side to move has no legal moves and is in check), `pos.outcome()` is `None`, so `simulate` returns `false` even though the position is a genuine `Loss`. In the `Outcome::Win | Outcome::Draw` branch, `best_move == Move::NONE` also returns `false`:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="673-677" />

This means `simulate` cannot validate any twin whose proof ends in a terminal checkmate or stalemate. Since many atomic-chess wins end by checkmating a lone commoner, cross-path twin reuse is effectively blocked for endgame twins. The main `dfpn` routine does handle these terminals correctly using `state.checkers` (see `dfpn` lines 367-386), but that logic was not duplicated in `simulate`.

### 2.3 Cross-path GHI simulation cannot follow path-dependent proof subtrees

`simulate` computes child path codes relative to the current search prefix and then probes the TT:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="678-690" />

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="713-718" />

`find_result_for_path` only returns a twin when `twin.path_code == path_code`:

<ref_snippet file="/workspace/atomic_solver/src/search/tt.rs" lines="71-87" />

The child entries that belong to a twin's proof tree were stored with path codes derived from the **twin's original path**, not the current path. Unless those child results are path-independent base entries (`!repetition_seen`), `find_result_for_path` will not find them and `simulate` will return `false`.

As a result, `try_use_tt` simulation succeeds only when the entire subtree under the twin is base (path-independent). In that case the twin at the parent is unnecessary; a base entry would suffice. The very cases the twin mechanism is meant to handle — path-dependent results whose child results are also path-dependent — cannot be verified across different paths with the current code. It fails safe (it does not accept an invalid result), but the GHI reuse mechanism is largely ineffective.

The existing unit tests (`try_use_tt_simulation_uses_current_path` and `try_use_tt_rejects_win_twin_for_repeated_position`) only exercise the case where the current position is already in `self.path`; they do not cover true cross-path reuse of a stored proof tree.

### 2.4 Repetition detection (`self.path`) uses the rule50-inclusive TT key

`Position::hash()` returns the full Zobrist key:

<ref_snippet file="/workspace/atomic_solver/src/position.rs" lines="112-114" />

`zobrist::hash` XORs the halfmove clock into `board.hash()`:

<ref_snippet file="/workspace/atomic_solver/src/zobrist.rs" lines="98-101" />

The search uses this same key for both the TT and the in-memory `path` set:

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="353-356" />

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn.rs" lines="359-361" />

The TT *should* include `rule50` because the 50-move draw is path-dependent. However, draw by repetition is determined by the board position (pieces, side, castling, en passant) and **not** by the 50-move counter. Because `self.path` uses the rule50-inclusive key, the solver does not detect a repetition that occurs after a sequence of reversible moves, where the halfmove clock has increased. This has two consequences:

1. The solver may classify as a win a line that passes through a repeatable position where the opponent could claim a draw.
2. Any cycle of reversible moves (where `rule50` increments on each lap) will never be detected by `self.path`, so the search can revisit the same board state with a different key indefinitely, leading to timeout or incorrect results.

A correct design keeps `rule50` in the TT key but uses a board-only (or board + side + castling/en passant) key for the repetition set.

---

## 3. Other issues and observations

### 3.1 `Position::outcome()` is missing terminal detection

The no-legal-move terminal logic is duplicated in `dfpn` and `validate_pv` but absent from `Position::outcome`. Centralizing it in `Position` would remove the inconsistency and make `simulate` simpler to fix. The current split means any new caller of `pos.outcome()` must independently reproduce the checkmate/stalemate check.

### 3.2 `MAX_TWINS = 8` replacement policy

The fixed array of eight twins and FIFO-ish eviction of slot 0 is safe but may be insufficient for heavily cyclic positions. Eviction causes repeated re-solving, not wrong results, but it interacts with the simulation limitations above to reduce reuse. If a cyclic position is added to the regression suite, a dynamic `Vec<TwinEntry>` or an LRU policy should be evaluated.

### 3.3 Binary refinement and timeout monotonicity

`solve_refined` assumes `dfpn(max_depth)` is monotonic in `max_depth`. Under timeout, a shallower probe can return `Draw` simply because time ran out before the winning line was found, causing the binary search to converge to a larger-than-minimal depth or fall back to the full-depth PV. The final validation at depth `lo` mitigates this, but the returned PV may not be the true shortest win if timeouts perturb the binary-search invariant.

### 3.4 `simulate` does not bound depth against the current search's `max_depth`

`SIM_MAX_DEPTH` and `SIM_MAX_NODES` cap simulation, but the simulation does not know the `max_depth` passed to the current `dfpn` search. A twin stored at depth larger than the current allowed depth could, in principle, be simulated and accepted. `try_use_tt` does guard `twin.depth <= max_depth` (see `dfpn.rs` lines 612-614 and 593-596), so this is only a minor concern inside the recursive simulation of descendants.

### 3.5 `outcome_from_pn_dn` is only safe for `Win`

The function is now documented and implemented correctly. Callers must still use the `outcome` field to distinguish `Loss` from `Draw`.

---

## 4. Research alignment

### `research_epsilon.md`

The `epsilon_ceil` and threshold updates follow the formula in the research note literally. However, the note incorrectly claims that `ε = 0.0` reproduces the original `p2 + 1` threshold; the implementation inherits that error. The default `ε = 0.25` works correctly, but the allowed range and tests should be updated to exclude `0.0` or `epsilon_ceil` should guarantee a minimum `+1` step.

### `research_ghi.md`

The base/twin split, base re-initialization, path-code Zobrist encoding, and halfmove-clock key are implemented. Kawano-style simulation is present, but it does not fully realize the paper's design because (a) it cannot validate terminal checkmate/stalemate nodes, and (b) it looks up child results with the current path code while the stored proof tree uses the twin's path codes. Cross-path reuse is therefore limited to path-independent subtrees. The `repetition_seen` flag is also a heuristic, not a proof of path-independence.

### `research_parallel.md`

The implementation is purely sequential. The OR/AND aggregation and threshold propagation match the literature. The `1 + ε` enhancement is the main deviation from the classic `+1` formula, and as noted it is broken for `ε = 0.0`.

---

## 5. Test status

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test
$ cargo doc
```

All pass. The regression suite in `tests/test_review.rs` covers single-commoner checkmates, stalemates, transpositions, and promotion path-code collisions. The `test_epsilon.rs` suite passes for the simple mate-in-one it uses, but does not exercise `ε = 0.0` on a position that requires search.

Additional manual testing showed that `ε = 0.0` times out on the mate-in-two position `rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3`, while `ε = 0.25` and `ε = 0.01` solve it in under a second.

There are no tests for true cross-path twin reuse or for repetition cycles with a changing halfmove clock.

---

## 6. Recommendations

1. **Fix `epsilon_ceil` for `ε = 0.0`.** Change the calculation to guarantee at least `x + 1` for `x > 0`, e.g. `max(x.saturating_add(1), scaled)`, or disallow `ε = 0.0` in `set_epsilon` and the CLI. Update the unit tests and `research_epsilon.md` accordingly.
2. **Centralize terminal detection.** Add a `Position` helper that uses `StateInfo::checkers` to classify no-legal-move positions as `Loss` (checkmate) or `Draw` (stalemate). Call it from `dfpn`, `validate_pv`, and `simulate`.
3. **Make `simulate` path-code aware or bounded.** For true cross-path verification, `simulate` must be able to borrow the twin's stored proof tree. One pragmatic fix is to run a bounded fresh `dfpn` under the current path instead of following TT entries whose path codes do not match. At minimum, `simulate` must handle terminal nodes using the centralized helper from recommendation 2.
4. **Separate TT and repetition keys.** Use a board-only (or board + side + castling + en passant) key for `self.path` repetition detection while keeping the rule50-inclusive key for the transposition table. This aligns with standard repetition rules and prevents missed cycles.
5. **Add dedicated GHI regression tests.** Create positions where the same board is reached by two different move orders and the result depends on the path, e.g. positions with a forced repetition that can be avoided. These will stress the twin and simulation logic.
6. **Reconsider twin replacement.** If cyclic positions cause frequent twin evictions, replace the fixed eight-slot FIFO with a dynamic list or an LRU replacement strategy.

---

## 7. Conclusion

Plans 1–8 successfully fixed the concrete issues identified in `review1.md`: terminal detection, move-type path-code encoding, binary refinement, PV validation, twin capacity, and runtime epsilon configuration are all in place and tested. The solver is solid for the regression suite it has.

The remaining risks are concentrated in two areas: **epsilon thresholding** (`ε = 0.0` is broken) and **GHI/repetition handling** (the simulation cannot validate terminal nodes or follow path-dependent proof subtrees, and the repetition set uses the wrong key). Fixing the epsilon bug is the highest priority because it can produce wrong timeout results for a user-visible setting. Improving GHI correctness will require either a more faithful Kawano simulation or a separation of the repetition key from the TT key, plus targeted regression tests.
