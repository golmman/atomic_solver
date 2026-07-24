# Plan 4: Restore PPV/SPPV correctness and finish the speed/bootstrap fixes

## Summary

The staged PPV/SPPV workflow introduced in `docs/plans/pv/report2.md` is still not producing a correct Proof PV for the regression FEN

```text
4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24
```

`cargo run --release -- --fen ... --no-refine-shortest --timeout 60` prints

```text
outcome: win
pv: e3f4 e8e1 b1b4 c6c5 b4b8 g8f7 a2a3 c5c4 b8g8 f7e6 g8g7 e6f5 g7g6
```

`e8e1` is not the strongest black reply.  Deeper analysis shows that the legal move `a4a3` (the move the user labels `a4a1`) lets black resist much longer, and even `e8c8` is stronger than `e8e1`.  Running the same FEN with `refine_shortest = true` and a 120 s timeout currently returns a 15-plies win, which is also not shortest.  The printed 13-plies line is therefore neither a PPV nor an SPPV.

`docs/plans/pv/report3.md` fixed several TT-bound/control-flow bugs but did not repair the underlying depth-aware child selection.  The depth-aware logic from `docs/plans/pv/report1.md` was lost in the `ultimattt` performance work: `is_solved_by_children` ignores the node type, the early-exit/depth-update code in `core.rs` uses the same `min`/`max` rule at OR and AND nodes, and `find_ppv` is currently run with `refine_shortest = false`, which prevents it from ever seeing the longest defensive reply.  In addition, the depth-bootstrap problems documented in `docs/plans/speed/checkpoint1.md` are still present: `max_depth == 0` leaves are stored as proven draws, and `refine_sppv`/`find_ppv` run with unbounded work budgets.

This plan restores correct node-type-aware depth selection, fixes the depth cutoff to be an unsolved frontier, and applies the work-bounded/TT-reuse discipline from `docs/plans/ultimattt/report4.md` to the whole staged API.

## Goal

- `Search::find_ppv` returns a **valid** PPV: every attacker move is a winning move and every defender reply is a longest resistance.
- `Search::refine_sppv` reduces that PPV to the SPPV when time allows.
- The CLI streams the outcome, the PPV, and any shorter PPVs exactly as specified in `docs/plans/pv/report2.md`.
- The iterative-deepening bootstrap no longer treats a depth cutoff as a proven draw, and no single refinement probe can consume the entire time budget.
- `tests/test_plan6.rs::m24_white_wins` and a new `m24_ppv` test pass (or can be enabled for a 60 s run).

## Non-goals

- No changes to `atomic-movegen` rules or move generation.
- No parallel search.
- No change to the transposition-table bucket/entry layout (the `best_child`/`work` extensions from `ultimattt` are preserved).

## Background and root cause

### Definitions

From `docs/theory/definitions.md` and `docs/plans/pv/report2.md`:

- **PPV**: defender replies maximize the length of the defense; attacker moves are any winning moves.
- **SPPV**: a PPV in which every attacker move is also a shortest winning move.

### What the search currently does wrong

1. **`is_solved_by_children` is node-type blind.**
   The function takes an `_is_or_node` parameter but never uses it.  At an AND node (defender to move) a proven `Win` for the parent means *all* defender replies lose.  The PPV must follow the **longest** losing reply, but the current code always picks the *shortest* child `Loss`.  After `1.e3f4`, black's `e8e1` loses faster than `a4a3`/`e8c8`, so the PV picks the weaker defense.

2. **The `dfpn` loop updates `best_win_depth`/`best_loss_depth` with the same rule for OR and AND nodes.**
   `core.rs` does
   ```rust
   if selection.depth < best_win_depth { ... }       // always min
   if selection.depth > best_loss_depth { ... }      // always max
   ```
   - `Win` at an OR node should be **min** (shortest attacker win).
   - `Win` at an AND node should be **max** (longest defender resistance).
   - `Loss` at an OR node should be **max** (attacker delays loss).
   - `Loss` at an AND node should be **min** (defender ends the game fastest).

3. **`evaluate_all_children` and `select_child_with_early_exit` are not node-type aware.**
   A single child `Loss` is treated as a decisive parent `Win` regardless of whether the node is OR or AND.  For an AND node this is wrong: one defender reply losing does not mean the position is won.  When `find_ppv` runs with `refine_shortest = false` this early-exits after the first losing defender reply and produces the non-PPV `e8e1` line.

4. **`find_ppv` is run with `refine_shortest = false`.**
   `Search::find_ppv` currently disables shortest-refinement before its bounded `dfpn` call.  That mode early-exits at OR nodes and, because of (3), also early-exits at AND nodes, so it cannot discover the longest defensive reply.

5. **`max_depth == 0` is stored as a proven draw.**
   In `core.rs` a non-terminal leaf with `max_depth == 0` is stored with `outcome = Some(Draw)` and `(pn, dn) = (INF, 0)`.  Bounded searches therefore treat the depth horizon as a game-theoretic draw.  This is the `max_depth = 8` horizon cliff from `docs/plans/speed/checkpoint1.md`: a mate just beyond the horizon cannot propagate, and the search expands the entire frontier.

6. **`refine_sppv` and `find_ppv` use `u64::MAX` work budgets.**
   A single failed depth probe can expand until the wall-clock timeout, leaving no time for the next probe.  This matches the known limitation noted in `docs/plans/pv/report3.md`.

## Design

### 1. Node-type-aware solved-child selection

Rewrite `src/search/dfpn/selection.rs::is_solved_by_children` so that the `is_or_node` argument is used to choose the correct extremal child depth:

| parent outcome | child outcome needed | OR node (attacker to move) | AND node (defender to move) |
|---|---|---|---|
| `Win` | child `Loss` | shortest child `Loss` | longest child `Loss` |
| `Loss` | child `Win` | longest child `Win` | shortest child `Win` |
| `Draw` | child `Draw` | longest child `Draw` | longest child `Draw` |

The function should also only report a decisive result when the node type actually allows it:

- OR node `Win`: return as soon as a child `Loss` is seen (others may be unsolved).
- AND node `Win`: return only when **all** children are `Loss`.
- AND node `Loss`: return as soon as a child `Win` is seen.
- OR node `Loss`: return only when **all** children are `Win`.

`select_child_with_early_exit` must check the node type and the proof mode; it must not early-exit on an AND-node `Win` from a single losing child.  `evaluate_all_children` must not stop after the first child `Loss` at an AND node when looking for a PPV/SPPV (it may stop at the first child `Win` for an AND-node `Loss` in outcome-only mode).

### 2. Node-type-aware depth tracking in `core.rs`

In the solved-outcome block of `Search::dfpn`:

- For `Outcome::Win`, use `min` at OR nodes and `max` at AND nodes.
- For `Outcome::Loss`, use `max` at OR nodes and `min` at AND nodes.
- Only break early when the result is already final for the current node type and the proof mode does not require a full minimax depth.

`best_win_depth`/`best_loss_depth` initial values and the update direction must switch with `is_or_node`.

### 3. A `ProofMode` for the staged API

Introduce a small proof mode (or repurpose `refine_shortest`) so the three stages use the right search behavior:

```rust
enum ProofMode {
    Outcome, // stop as soon as the result is proven
    Ppv,     // defender replies are longest; attacker moves are any winning move
    Sppv,    // fully minimax: shortest attacker wins, longest defender replies
}
```

- `solve_outcome` uses `Outcome`.
- `find_ppv` uses `Ppv`.
- `refine_sppv` uses `Sppv`.

If adding the enum is too large a change, the same effect can be achieved by making `refine_shortest` truly node-type aware and running `find_ppv` with `refine_shortest = true` (it will then return an SPPV, which is also a valid PPV; `refine_sppv` will simply find nothing further).  The enum is preferred because it lets `find_ppv` avoid the extra OR-node work needed for an SPPV.

### 4. Fix the `max_depth == 0` leaf cutoff

In `core.rs`, when `max_depth == 0` and the position is not terminal, store an **unsolved frontier** entry:

```rust
outcome: None,
pn: 1,
dn: 1,
remaining_depth: 0,
```

The function may still return `Outcome::Draw` for callers that ignore the return value, but the transposition-table entry must not be a proven draw.  This makes bounded iterative deepening sound: a `max_depth` that is below the mate distance returns "unknown" rather than a false draw, and the next deeper probe can reuse the cheap `(1, 1)` bounds while growing past the horizon.

### 5. Work-bounded `find_ppv` and `refine_sppv`

`dfpn` already has a `max_work` parameter; `solve_outcome` already uses it.  Extend the same discipline to the PPV/SPPV stages:

- `find_ppv` calls `dfpn(..., bootstrap_success_depth, max_work)` with a work chunk instead of `u64::MAX`.
- `refine_sppv` gives each downward probe a `max_work` chunk that grows with the probe index (e.g. starting from the same `500_000` base used by `solve_outcome`).
- Do not clear the transposition table between `find_ppv` and `refine_sppv`; only reset `path_stack`, `path_code`, and similar path-dependent state.
- If a probe exhausts its work budget without a decisive result, treat it as `Draw` (unknown) and keep the best PV already found.

For `solve_outcome`, consider a finer `max_depth` schedule such as `1, 2, 4, 8, 12, 16, 20, 24, 32, 48, 64` instead of pure doubling, to reduce the gap between `8` and `16` that caused the original fen1/fen2 discrepancy.

### 6. PV extraction and validation

- `Search::find_ppv` must verify the extracted line with `extract_ppv`/`validate_pv` before returning it.
- `extract_pv_checked` should compare the extracted length to an expected depth when one is passed.
- `extract_pv` should follow the stored `best_move` whose `depth` matches the remaining plies for the path, so that it follows depth-optimal entries rather than stale shorter ones.

## File changes

### `src/search/dfpn/selection.rs`

- Make `is_solved_by_children` node-type aware for `Win`/`Loss`/`Draw` child selection and for the `all_solved` condition.
- Update `select_child_with_early_exit` to respect `is_or_node` and the proof mode.
- Add/extend unit tests:
  - AND-node `Win` picks the longest child `Loss`.
  - AND-node `Loss` picks the shortest child `Win`.
  - OR-node `Loss` still picks the longest child `Win` (existing test).
  - OR-node `Win` still picks the shortest child `Loss` (existing test).

### `src/search/dfpn/core.rs`

- Initialize and update `best_win_depth`/`best_loss_depth` using `is_or_node`.
- Only early-break on a solved `Win`/`Loss` when the node-type rule makes the result final and the proof mode does not require full expansion.
- Change `max_depth == 0` storage from `outcome = Some(Draw)` to `outcome = None` with `(pn, dn) = (1, 1)` and `remaining_depth = 0`.
- Pass `proof_mode` / `refine_shortest` down to `evaluate_all_children` and `select_from_children`.

### `src/search/dfpn/children.rs`

- `evaluate_all_children` early-exits only when the current node type/proof mode allows it.
- `evaluate_child` keeps the `ultimattt` unsolved-summary guards.

### `src/search/dfpn/mod.rs`

- Replace or augment `refine_shortest: bool` with `ProofMode` (or add a `proof_mode` field and keep `refine_shortest` as a thin setter for compatibility).
- `solve_outcome`: keep the depth-doubling/work-bounded hybrid, possibly with a finer schedule, and preserve `bootstrap_success_depth`/`bootstrap_fail_depth` correctly.
- `find_ppv`: use `ProofMode::Ppv` (or `refine_shortest = true` as a fallback), call `dfpn(..., bootstrap_success_depth, max_work)` with a work chunk (e.g. the same `500_000` base used by `solve_outcome`) instead of `u64::MAX`, and fall back to a fresh bounded search if the first extraction fails.
- `refine_sppv`: use `ProofMode::Sppv` and cap each downward probe with `max_work`.
- `Search::solve` wrapper sets the appropriate mode for each stage.

### `src/search/dfpn/pv.rs`

- `extract_ppv` validates the defender-reply/attacker-move chain and the terminal outcome.
- `extract_pv_checked` uses `expected_depth` to reject a mismatched line.
- `extract_pv` prefers a TT twin/base whose `depth` equals the remaining plies.

### `src/main.rs`

- No major changes; keep the streaming output from `report2`.
- Ensure `timeout` is only printed once and only after the stage that exceeded the budget.

### `tests/test_plan6.rs`

- Add `m24_ppv` (or `m24_shortest_pv`) for `4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24`:
  - `solve_outcome` returns `Win`.
  - `find_ppv` returns a PPV whose second move is a strong defender reply (`a4a3`, `e8c8`, or another move of equal maximal length) and whose terminal validation passes.
- Re-enable `m24_white_wins`/`m24_black_loses` if they now finish within the 60 s/5 s budgets.
- Keep the existing `m27_streaming_output`, `m27_ppv_only`, `m27_shortest_pv`, and `timeout_message` tests.

## Testing and verification

1. Format, lint, and unit tests:
   ```bash
   cargo fmt
   cargo clippy --all-targets
   cargo test
   cargo test --release
   cargo doc
   ```

2. Manual CLI checks:
   ```bash
   cargo run --release -- --no-refine-shortest \
       --fen "4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24" \
       --timeout 60
   ```
   Expected: `outcome: win` followed by a valid PPV starting with `e3f4` and a defender reply that is among the longest.

   ```bash
   cargo run --release -- \
       --fen "4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24" \
       --timeout 60
   ```
   Expected: `outcome: win`, a PPV, then shorter `pv:` lines leading to the SPPV (or `timeout` if the SPPV proof is not finished).

3. Speed/bootstrap regression checks from `docs/plans/speed/checkpoint1.md`:
   ```bash
   cargo run --release -- --fen "6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25" --timeout 60
   cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" --timeout 60 --no-refine-shortest
   ```
   Both should return decisive outcomes within the timeout.

4. `m27` regression:
   ```bash
   cargo test --release --test test_plan6 m27_streaming_output
   cargo test --release --test test_plan6 m27_shortest_pv
   ```

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Changing `is_solved_by_children` breaks existing OR-node tests. | Keep the existing unit tests and add AND-node counterparts; verify both before moving on. |
| Node-type-aware selection increases node counts for `find_ppv`. | Use work-bounded probes and the `Ppv` proof mode so AND nodes are fully expanded but OR nodes can stop after the first winning child. |
| `max_depth == 0` frontier changes bounded-search behavior elsewhere. | Validate with `solve_depth_limited` examples and the `test_plan6` suite. |
| `refine_sppv` still runs out of time on deep positions. | Cap each probe with `max_work`; a valid PPV is already printed before refinement starts. |
| The user mentions `a4a1`; the legal move is `a4a3`. | The success criterion uses the concrete legal move `a4a3` and notes that `a4a1` is not generated by `atomic-movegen` for this position. |

## Success criteria

- `cargo test` and `cargo clippy --all-targets` pass with no new warnings.
- The reported FEN with `--no-refine-shortest` prints `outcome: win` and a PPV whose defender reply is a longest legal defense (e.g. `a4a3` or `e8c8`), not `e8e1`.
- The default (refining) run prints a valid PPV and then the SPPV when time allows, or prints `timeout` after the last valid PPV.
- The fen1/fen2 positions from `docs/plans/speed/checkpoint1.md` still finish decisively within 60 s.
- `m27_streaming_output`, `m27_shortest_pv`, and `m27_ppv_only` still pass.
- A new `m24` PPV/SPPV test is added and passes (or is enabled for a 60 s run).
