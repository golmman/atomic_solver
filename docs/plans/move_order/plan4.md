# Plan 4: Node-type-aware ordering and TT-bound-aware initial sort

## Start

1. Read `AGENTS.md` to confirm project conventions, file-size rules, and quality gates.
2. Read `docs/plans/move_order/ideas.md` for the full list of move-ordering ideas,
   especially ideas 3 (node-type-aware ordering) and 5 (TT-bound-aware initial sort).
3. Read `docs/plans/move_order/report3.md` for the current baseline, constants, and
   unresolved next steps.
4. Read the code that will be touched:
   - `src/search/ordering.rs` (`StaticAtomicScorer`, `score_with_map`, scoring constants)
   - `src/search/dfpn/history.rs` (`sort_moves`, `move_order_breakdown`, tests)
   - `src/search/dfpn/core.rs` (the `sort_moves` call site)
   - `src/search/dfpn/children.rs` (test that calls `sort_moves`)
   - `src/search/tt/entry.rs` (`TtSummary` fields)
   - `examples/static_move_scores.rs` and `examples/move_order_debug.rs`
5. Record a pre-change baseline:
   ```bash
   cargo run --release --example static_move_scores -- --name m22_white
   cargo run --release --example static_move_scores -- --name m25_white
   cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5 --runs 3
   cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 3
   cargo test --release --test test_move_order
   cargo test --release --test stress move_order_hard -- --test-threads=2
   ```

## Goal

Implement ideas **3** and **5** from `docs/plans/move_order/ideas.md` together.

- **Idea 3:** make `sort_moves` and `StaticAtomicScorer` aware of whether the current
  node is an OR node (the side trying to prove a win) or an AND node (the opponent
  trying to draw or delay). Use different scoring weights for each.
- **Idea 5:** use the proof/disproof bounds already stored in the transposition table
  to bias the initial sort so that the first expanded child is closer to the true
  most-proving node.

Only changes that pass the measurement gate below are kept. If a phase fails, revert
it before writing `docs/plans/move_order/report4.md`.

## Background

After plan3 the static scorer understands kamikaze, direct commoner threats, atomic
SEE, pawn storms, rook centralization, open-file alignment, and back-rank presence.
However, the same static profile is used for both OR nodes (attacker) and AND nodes
(defender). In atomic chess a move that is excellent for the attacker is often
weakening for the defender, so the two node types should not share identical move
ordering.

In addition, `sort_moves` currently pulls only the TT best move to the front. Every
other child starts with `(pn, dn) = (1, 1)`, which means the first child tried by the
DF-PN+ loop is simply the top statically-scored move. Because `Search` works in
iterative work chunks, the TT often already contains `pn`/`dn` bounds for several
children, but that information is ignored when the move list is sorted.

## Implementation tasks

### Phase A: Plumbing `is_or_node` into `sort_moves` and the scorer

1. Add a node-type parameter. A plain `bool` (`is_or_node: bool`) is enough to start;
   if the code becomes unreadable, introduce a small `enum NodeKind { Or, And }`.
2. Change `StaticAtomicScorer::score_with_map` to accept `is_or_node`. Keep the
   `MoveScorer` trait method `score` for backward compatibility by defaulting to
   `is_or_node = true` (OR / root perspective) and delegating to `score_with_map`.
3. Change `Search::sort_moves` to:
   ```rust
   pub(super) fn sort_moves(
       &self,
       pos: &mut Position,
       moves: &mut MoveList,
       best_from_tt: Move,
       is_or_node: bool,
   )
   ```
   `pos` becomes `&mut Position` because Phase B will temporarily apply each move
   to probe the child TT entry.
4. Update call sites and tests:
   - `src/search/dfpn/core.rs` passes the `is_or_node` parameter of `dfpn`.
   - `src/search/dfpn/children.rs` test `evaluate_all_children_stops_at_winning_child`
     passes `true` (it tests an OR-node capture).
   - `src/search/dfpn/history.rs` unit tests pass `true` or `false` as appropriate.
5. Update `Search::move_order_breakdown` to accept `is_or_node` and pass it through
   to `score_with_map`. The existing example call can default to OR.
6. Update `examples/static_move_scores.rs` and `examples/move_order_debug.rs` so they
   can optionally show the AND profile (e.g. `--and`). Update the file header doc
   comments accordingly.

### Phase B: Node-type-aware scoring profile

Keep the OR profile identical to the current scoring. For AND nodes, apply a defensive
multiplier to the speculative attacker-only bonuses while preserving genuine
 counter-threats and captures.

| Component | OR | AND starting hypothesis |
|---|---|---|
| `SCORE_WINNING_CAPTURE` | keep | keep (always top) |
| `SCORE_CAPTURE` base + `net * CAPTURE_NET_SCALE` | keep | keep (aSEE already measures material) |
| `SCORE_PROMOTION` | keep | keep |
| `SCORE_THREAT` / `SCORE_THREAT_LAST` | full | full or slightly higher (counter-threats can win) |
| `SCORE_KAMIKAZE` / `SCORE_KAMIKAZE_LAST` | full | full or slightly higher |
| `SCORE_PAWN_STORM` | full | multiply by `AND_PAWN_STORM_SCALE` (start 50 / 100) |
| `SCORE_ROOK_OPEN_FILE` / `SCORE_ROOK_BACK_RANK` | full | multiply by `AND_ROOK_ATTACK_SCALE` (start 50 / 100) |
| `SCORE_APPROACH` / `SCORE_CENTER` | full | multiply by `AND_APPROACH_SCALE` (start 75 / 100) |

Add integer scale constants:

```rust
const AND_PAWN_STORM_SCALE: i32 = 50;      // out of 100
const AND_ROOK_ATTACK_SCALE: i32 = 50;   // out of 100
const AND_APPROACH_SCALE: i32 = 75;        // out of 100
```

Rationale: at AND nodes the defender wants to survive, so quiet solidity and genuine
counter-threats are preferred over speculative pawn storms and rook lifts. The exact
multipliers are a starting guess; the measurement gate decides whether they are kept.

Unit-test ideas:
- A pawn-storm push is scored lower at an AND node than at an OR node for the same
  position.
- A direct commoner threat still scores high at an AND node (it is a counter-threat).
- A quiet centralizing move is preferred over a pawn storm at an AND node.

### Phase C: TT-bound-aware initial sort

In `sort_moves`, after computing the static + history + killer score for each move,
add a small TT-bound bonus before the final sort:

1. For each move `m` in the candidate list, temporarily apply it:
   ```rust
   pos.do_move(m);
   let summary = self.tt.probe_summary(pos.hash());
   pos.undo_move(m);
   ```
2. If `summary` is `Some` and `summary.outcome.is_none()` (unsolved), compute:
   - OR node: `bonus = TT_BOUND_BONUS_MAX - min(summary.pn * TT_BOUND_BONUS_SCALE,
     TT_BOUND_BONUS_MAX as u64)` as `i32` (saturating; `pn == INF` gives 0).
   - AND node: use `summary.dn` instead of `summary.pn`.
3. Add the bonus to the move's total. Keep the existing `best_from_tt` swap so the
   stored best move remains first when the TT provides one.

Starting constants:

```rust
const TT_BOUND_BONUS_SCALE: u64 = 10;
const TT_BOUND_BONUS_MAX: i32 = 5_000;
```

These are deliberately below `SCORE_CAPTURE` (5_000) and far below `SCORE_THREAT`
(10_000) so TT information breaks ties among quiet moves but does not override
tactical signals.

Unit-test ideas:
- After a child is stored in the TT, `sort_moves` moves it toward the front even when
  it is not the stored `best_move`.
- The bonus is 0 for a child whose `pn` (OR) or `dn` (AND) is `INF`.
- The position is unchanged after `sort_moves` returns (temporary moves are undone).

### Phase D: Measurement gate

After each phase run:

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo run --release --example static_move_scores -- --name m22_white
cargo run --release --example static_move_scores -- --name m22_white --and
cargo run --release --example move_order_debug -- --name m22_white
cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5 --runs 3
cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 3
cargo test --release --test test_move_order
cargo test --release --test stress move_order_hard -- --test-threads=2
```

Keep a phase if:
- `cargo test` passes and no benchmark position is misclassified.
- Mean `nodes`/`child_evals` on `m24`–`m29` in first-outcome mode does not regress by
  more than 5% and ideally improves.
- A previously unsolved position (`m20`, `m21`, or `m22_white` in first-outcome) becomes
  decisive, or shows a clear reduction in nodes at the same timeout.
- The `m25_white` refined-mode regression noted in `report3.md` does not worsen.

Revert a phase immediately if any test fails, a wrong decisive outcome appears, or the
solvable suite slows down.

### Phase E: Regression test updates (only if improvements land)

- If `m22_white` becomes reliably decisive within 5 seconds in first-outcome mode,
  update the fixture note in `tests/fixtures/move_order_positions.txt` and add or
  extend `tests/test_move_order.rs`.
- If `m20` or `m21` become solvable within the stress budget, move them out of
  `tests/stress.rs` and into the regression suite.
- Add unit tests in `src/search/ordering/tests.rs` covering the new node-type and
  TT-bound behavior.

### Phase F: Final report

Write `docs/plans/move_order/report4.md` documenting:
- Which phases were kept, reverted, or partially kept.
- Final `is_or_node` plumbing and any public API changes.
- Final node-type scale constants and TT-bound bonus constants.
- Measured impact on the move-order suite and the default suite.
- Problems encountered, unresolved edge cases, and next ideas to evaluate (likely idea
  6, stronger dynamic heuristics, and/or idea 7, repetition/draw-avoidance ordering).

## File changes

- `src/search/ordering.rs` — `score_with_map` signature and node-type scoring profile.
- `src/search/dfpn/history.rs` — `sort_moves` and `move_order_breakdown` signatures,
  TT-bound bonus, and tests.
- `src/search/dfpn/core.rs` — update the `sort_moves` call site.
- `src/search/dfpn/children.rs` — update the test `sort_moves` call.
- `examples/static_move_scores.rs` — pass `is_or_node` (default OR, optional `--and`).
- `examples/move_order_debug.rs` — pass `is_or_node` to `move_order_breakdown` if the
  example supports showing the AND profile.
- `tests/test_move_order.rs` and `tests/stress.rs` — only if positions become solvable.
- `README.md` / `AGENTS.md` — only if a new public CLI flag is added.
- `docs/plans/move_order/report4.md` — final deliverable.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| `is_or_node` is threaded incorrectly and silently misorders moves. | Compiler catches signature changes; add unit tests comparing OR and AND scores; run `test_move_order` and `stress` after every edit. |
| `sort_moves` mutates `pos` for TT probes and fails to restore it. | Use `pos.do_move` / `pos.undo_move` in a tight scope; the deterministic `sort_is_deterministic` test and the full benchmark suite detect a corrupted position. |
| Duplicate TT probes slow the search. | First pass can re-probe; if profiling shows cost, cache the `TtSummary` per move and pass it to `evaluate_all_children` as a later optimization. |
| TT-bound bonus over-scales and overrides tactical signals. | Start with `TT_BOUND_BONUS_MAX = 5_000` (below `SCORE_CAPTURE`); tune by ±2_000 using `m24`–`m29` first-outcome nodes. |
| AND profile guesses are wrong and hurt the suite. | Keep the OR profile unchanged. Set all `AND_*_SCALE` values to 100 to disable the AND profile as a fallback; revert if the benchmark gate fails. |
| `m25_white` refined-mode regression from `report3.md` persists. | If the regression is caused by `SCORE_ROOK_OPEN_FILE` rather than node type, note it in `report4.md` as a separate follow-up (tightening the rook-open-file metric). |

## Verification

Run after every meaningful edit:

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo doc
```

Move-order diagnostics:

```bash
cargo run --release --example static_move_scores -- --name m22_white
cargo run --release --example static_move_scores -- --name m22_white --and
cargo run --release --example static_move_scores -- --name m25_white
cargo run --release --example move_order_debug -- --name m22_white
cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5 --runs 3
cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 3
```

Regression and stress:

```bash
cargo test --release --test test_move_order
cargo test --release --test stress move_order_hard -- --test-threads=2
```

If any benchmark position returns a decisive outcome that differs from the fixture, the
current phase is a regression.

## Success criteria

1. `cargo test`, `cargo clippy --all-targets`, and `cargo doc` pass with no new warnings.
2. No wrong decisive outcomes on the move-order suite.
3. `is_or_node` is correctly threaded through `sort_moves` and `score_with_map`.
4. Mean `nodes`/`child_evals`/first-outcome time on `m24`–`m29` does not regress by
   more than 5%; ideally improves.
5. At least one of the following is true:
   - `m20`, `m21`, or `m22_white` becomes decisive within the benchmark timeout, or
   - `m24`–`m29` mean nodes drop measurably.
6. New unit tests cover node-type-aware scoring and TT-bound sort behavior.
7. `docs/plans/move_order/report4.md` is written and accurately reflects kept and
   reverted changes.

## Final task

Write `docs/plans/move_order/report4.md` with:
- Which phases were implemented, reverted, or partially kept.
- Final `is_or_node` plumbing and any public API changes.
- Final node-type scale constants and TT-bound bonus constants.
- Measured impact on the move-order suite and the default suite.
- Any unresolved edge cases and next steps.
