# Plan 2: Static move-ordering fixes — near-commoner heuristics and atomic SEE

## Start

1. Read `AGENTS.md` to confirm project conventions, file-size rules, and quality gates.
2. Read `docs/plans/move_order/ideas.md` for the full list of move-ordering ideas.
3. Read `docs/plans/move_order/report1.md` to understand the benchmark suite and current baselines.
4. Read the code that will be touched:
   - `src/search/ordering.rs` (`StaticAtomicScorer`)
   - `src/search/dfpn/history.rs` (where `sort_moves` applies the scorer)
   - `examples/benchmark.rs`, `examples/static_move_scores.rs`, `examples/move_order_debug.rs`
   - `tests/test_move_order.rs`, `tests/stress.rs`
5. Record a pre-change baseline for the move-order suite and a few diagnostic positions (`m19`, `m20_white`, `m26_white`):
   - `cargo run --release --example static_move_scores -- --name m20_white`
   - `cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 3`
   - `cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5 --runs 3`

## Goal

Improve `StaticAtomicScorer` in `src/search/ordering.rs` so it matches atomic-chess tactics better. The first batch implements the two lowest-risk, highest-expected-impact ideas from `ideas.md`:

1. **Fix the "near commoner" heuristics** (idea 1): stop rewarding attacks on squares *adjacent* to the enemy commoner and instead reward moves that land adjacent to a commoner (a real kamikaze threat). Remove the extra one-square ring from the blast heuristic so it no longer treats a piece two squares away as an immediate threat.
2. **Atomic static exchange evaluation (aSEE)** (idea 2): replace MVV-LVA capture scoring with the net material destroyed by the atomic blast, including the capturing piece and any own pieces caught in the explosion.

Only changes that show a **measurable** improvement on the move-order benchmark suite are kept. If a phase does not reduce `nodes`/`child_evals` on the solvable positions (`m24`–`m29`) or make a hard position (`m20`/`m21`) decisive without misclassifying results, it is reverted and the failure is documented in `report2.md`.

## Background

`StaticAtomicScorer` currently mixes three related but distinct ideas in a way that overestimates quiet bishop probes:

- `SCORE_ATOMIC_CHECK` is given when the moved piece attacks a square *adjacent to* the last enemy commoner (`attack_bb & king_attacks(enemy_king_sq)`). On `m19` and `m20` this puts many bishop diagonals (`d6e7`, `d6f8`, etc.) near the top of the move list even though they do not directly attack the commoner.
- `SCORE_BLAST` extends the blast zone by one extra king-attack ring for non-captures, so a quiet move two squares away from the enemy commoner can score as a threat.
- Captures use MVV-LVA: `SCORE_CAPTURE + 10 * victim_value - attacker_value`. This ignores that atomic captures are explosions: a queen capturing a pawn on an empty square loses the queen for a pawn, and a capture into a crowded zone can destroy several pieces.

Both issues are confined to `src/search/ordering.rs`, so they can be fixed and measured without changing the DF-PN loop.

## Implementation tasks

### Phase A: Fix near-commoner heuristics

In `src/search/ordering.rs`:

1. **Replace `SCORE_ATOMIC_CHECK` with a strict kamikaze bonus.**
   - Rename the constant to `SCORE_KAMIKAZE` (or keep `SCORE_ATOMIC_CHECK` if renaming would churn unrelated tests).
   - Apply it when a quiet move lands on a square adjacent to *any* enemy commoner:
     ```text
     (attacks::king_attacks(to) & board.commoners(them)) != EMPTY
     ```
   - Keep the distinction between one and multiple enemy commoners using `state.them_commoners_count`, mirroring `SCORE_THREAT`/`SCORE_THREAT_LAST`:
     - `SCORE_KAMIKAZE_LAST` (e.g., 9_000) when the opponent has one commoner.
     - `SCORE_KAMIKAZE` (e.g., 3_000) otherwise.
   - Remove the old branch that awarded points for `attack_bb & king_attacks(enemy_king_sq)`.

2. **Fix `SCORE_BLAST` for non-captures.**
   - Remove the second ring expansion (`near = blast_zone | union(king_attacks(sq))`).
   - Use only the immediate blast zone:
     ```text
     blast_zone = attacks::king_attacks(to) | Bitboard::square_bb(to)
     ```
   - Since the immediate blast-zone check for non-captures is now the same condition as the kamikaze bonus, **do not double-count**. Either delete the separate `SCORE_BLAST` block and fold a small constant into `SCORE_KAMIKAZE`, or keep `SCORE_BLAST` at `0` for non-captures.

3. **Keep `SCORE_THREAT` for direct commoner attacks.**
   - Continue to reward moves where the piece, after moving, attacks the enemy commoner's square (`attack_bb & board.commoners(them)`).
   - Preserve the `state.them_commoners_count == 1` boost (`SCORE_THREAT_LAST` vs `SCORE_THREAT`).

4. **Sanity check with `static_move_scores`.**
   - `m19` should no longer have a long tail of bishop probes all scoring ~9_000.
   - `m20_white` should move captures and direct rook/knight approach moves above distant bishop probes.

### Phase B: Atomic static exchange evaluation (aSEE) for captures

Still in `src/search/ordering.rs`, replace the MVV-LVA capture branch with a net-blast score.

1. **Preserve the winning-capture special case first.**
   - If the capture (including capture-promotions and en-passant) would remove the opponent's last commoner, keep returning `SCORE_WINNING_CAPTURE`.

2. **Handle promotion captures correctly.**
   - If `m.is_promotion()` and `is_capture`, do **not** return `SCORE_PROMOTION`. The promoted piece is destroyed in the blast, so the capture should be scored by aSEE.
   - If `m.is_promotion()` and `!is_capture`, keep the existing `SCORE_PROMOTION + piece_value(m.promotion_type())` branch.

3. **Add a helper to compute the net material destroyed by a capture blast.**

   ```rust
   fn capture_net_value(board: &Board, m: Move) -> i32 {
       // victim value
       // enemy pieces (non-pawn) in king_attacks(to)
       // own pieces (non-pawn) in king_attacks(to), excluding the origin square
       // own cost = piece_value(moving piece) + own blast losses
       // net = enemy destroyed - own destroyed
   }
   ```

   Details:
   - **Victim**: `board.piece_on(to)` for normal captures; a pawn for en-passant.
   - **Blast radius**: `attacks::king_attacks(to) & !board.pieces_pt(PieceType::Pawn)`. Pawns are immune to the surrounding blast.
   - **Ground zero**: the `to` square is always blasted. Count the victim separately; the moving piece is added as own loss below.
   - **Exclude `from`**: if the origin square is in the blast radius (king move, pawn capture), do not count the moving piece there because it is leaving that square. Count its value once as the moving piece.
   - **Promotion captures**: the moving pawn is consumed (count `piece_value(Pawn)`). The promoted piece is transient; do not count its full value as a lost own piece.
   - Sum piece values over a bitboard using `board.piece_on` and the existing `piece_value` table.

4. **Score captures by net gain.**
   - Use a small base plus a scaled net value so losing suicidal captures fall below forcing non-captures while good trades remain attractive. Start with:
     ```rust
     const SCORE_CAPTURE: i32 = 5_000;
     const CAPTURE_NET_SCALE: i32 = 10;
     // score = SCORE_CAPTURE + net * CAPTURE_NET_SCALE
     ```
   - These constants are intentionally conservative. Tune them during the measurement phase if the benchmark shows captures are still over- or under-prioritized.

5. **Add unit tests for aSEE.**
   - A queen capturing a defended pawn on an empty square scores below a direct commoner threat.
   - A capture that also blasts an enemy rook is scored higher than a capture that only takes a pawn.
   - A capture-promotion that blows up the promoted piece is not scored by `SCORE_PROMOTION`.

### Phase C: Measurement gate

After each phase, run the verification commands below. Decide whether to keep the change:

- **Keep** if on `m24`–`m29` (with `timeout 10`, `runs 3`) at least one of the following improves without regressions elsewhere:
  - Mean `nodes` reduced by ≥10 % averaged over the five positions.
  - Mean `child_evals` reduced by ≥10 % averaged over the five positions.
  - Mean time to first decisive outcome reduced.
- **Keep** if a previously hard position (`m20`, `m21`, `m22`) becomes decisive and does not misclassify.
- **Revert** if any position returns a wrong decisive outcome or if the solvable suite gets slower.

If a phase fails the gate, remove it (or stash it) before the final report.

### Phase D: Regression test updates (only if improvements land)

- If `m20` or `m21` become solvable within 60 s, move them out of `tests/stress.rs` and into `tests/test_move_order.rs` with the correct expected outcome and a documented timeout.
- If `m22` or `m23` move from "60 s stress" to "5 s regression" reliable, update `tests/test_move_order.rs` or the fixture notes.
- Add unit tests in `src/search/ordering.rs` for the new kamikaze and aSEE behavior.
- Update `AGENTS.md` only if a public API or example flag changes (none expected).

## File changes

- `src/search/ordering.rs` (main change)
- `tests/test_move_order.rs` (conditional, if hard positions become reliably solvable)
- `tests/stress.rs` (conditional, if `m20`/`m21` move out)
- `docs/plans/move_order/report2.md` (final deliverable)

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Removing the broad `SCORE_ATOMIC_CHECK`/`SCORE_BLAST` ring drops useful quiet probes. | The new `SCORE_KAMIKAZE` and `SCORE_THREAT` still reward real forcing moves. The measurement gate will detect if `m24`–`m29` get slower. |
| aSEE under-values sacrifices that are tactically winning because the blast removes an own queen. | `SCORE_WINNING_CAPTURE` still promotes captures that kill the last commoner. Other sacrifices are evaluated by search. |
| aSEE is too expensive to run per capture. | The blast zone has at most nine squares; the helper iterates over a tiny bitboard. Profile if necessary. |
| Promotion captures are mis-scored. | Add explicit unit tests for capture-promotions and for `is_promotion() && is_capture` paths. |
| `m20`/`m21` remain unsolved and the stress test still passes; no file move needed. | Document the unchanged status in `report2.md`. |
| A change improves `nodes` on `m24`–`m29` but misclassifies `m20`/`m21`. | The `test_move_order` and `stress` suites must still pass; any wrong result is an immediate revert. |

## Verification

Run after every meaningful edit:

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo doc
```

Run the move-order diagnostics:

```bash
# Before and after each phase:
cargo run --release --example static_move_scores -- --name m19
cargo run --release --example static_move_scores -- --name m20_white
cargo run --release --example static_move_scores -- --name m26_white
cargo run --release --example move_order_debug -- --name m20_white

# Benchmark the full suite:
cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 3
cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5 --runs 3
```

Run the regression/stress tests:

```bash
cargo test --release --test test_move_order
cargo test --release --test stress move_order_hard -- --test-threads=2
```

If any test returns a decisive outcome that differs from the fixture, the current phase is a regression.

## Success criteria

1. `cargo test` and `cargo clippy --all-targets` pass with no new warnings.
2. No wrong decisive outcomes on the move-order suite.
3. At least one of the two ideas shows a measurable improvement on `m24`–`m29` (`nodes`, `child_evals`, or first-outcome time).
4. If `m20`/`m21` become decisive, they are moved to the regression suite and `tests/stress.rs` is updated.
5. `src/search/ordering.rs` has new unit tests for kamikaze detection and aSEE.

## Final task

Write `docs/plans/move_order/report2.md` documenting:

- Which ideas were implemented, reverted, or kept.
- The measured impact on `nodes`/`child_evals`/time for `m19`–`m29` and the default suite.
- Any constants that were tuned (`SCORE_KAMIKAZE`, `SCORE_CAPTURE`, `CAPTURE_NET_SCALE`, etc.).
- Whether `m20`/`m21` became solvable and which tests were updated.
- Unresolved edge cases, missing tests, and the next batch of ideas to evaluate (likely idea 3, node-type-aware ordering, and idea 5, TT-bound-aware initial sort).
