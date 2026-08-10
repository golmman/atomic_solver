# Plan 3: Plan-aware pawn-storm and rook-centralization ordering

## Start

1. Read `AGENTS.md` for project conventions, file-size limits, and quality gates.
2. Read `docs/plans/move_order/report2.md` and `docs/plans/move_order/ideas.md` for the current state and open ideas.
3. Read `src/search/ordering.rs` (`StaticAtomicScorer`, `score_with_map`, and the scoring constants).
4. Read the diagnostic examples:
   - `examples/static_move_scores.rs`
   - `examples/move_order_debug.rs`
   - `examples/benchmark.rs`
5. Read the benchmark fixtures:
   - `tests/fixtures/move_order_positions.txt`
   - `tests/stress.rs`
   - `tests/test_move_order.rs`
6. Record a pre-change baseline:
   ```bash
   cargo run --release --example static_move_scores -- --name m22_white
   cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5 --runs 3
   cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 3
   ```

## Goal

Improve `StaticAtomicScorer` in `src/search/ordering.rs` so that the solver finds
`m22_white`'s short winning plan earlier. The current scorer prefers captures and
direct commoner threats, so the first decisive line found by `Search::solve` is a
117-ply win through `g4h5`. The actual `m22` plan starts with the quiet pawn
push `1.g4g5` followed by rook centralization (`2.Rg1e1`), but these moves are
scored far below `d6e5`, `e3f4`, and `g4h5`.

Only changes that pass the measurement gate below are kept. If a phase fails the
gate, it is reverted before the report is written.

## Background

`m22_white` is a `Win` from the move-order benchmark suite:

```text
4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22
```

The intended winning line is a 15-ply mate:

```text
1.  g4g5    h8g8
2.  Rg1e1   a5a4
3.  e3f4    a4a3
4.  Rxe8    c6c5
5.  Rb1b8   Kf7
6.  Rb8g8   Ke6
7.  Rg8f8   c5c4
8.  Rf8f6#
```

This line uses three motifs that the current scorer does not value:

1. **Pawn storm** — `g4g5` advances the g-pawn toward the h8 commoner and makes
the squares `f6` and `h6` inhospitable.
2. **Rook centralization** — `Rg1e1` prepares `Rxe8` and opens the e-file.
3. **Back-rank rook lift** — `Rb1b8`, `Rb8g8`, `Rg8f8` confine the commoner and
deliver mate.

The current `StaticAtomicScorer` constants in `src/search/ordering.rs` are:

- `SCORE_WINNING_CAPTURE` = 100_000_000
- `SCORE_PROMOTION` = 1_000_000
- `SCORE_CAPTURE` = 5_000
- `SCORE_THREAT_LAST` = 10_000
- `SCORE_THREAT` = 1_000
- `SCORE_KAMIKAZE_LAST` = 9_000
- `SCORE_KAMIKAZE` = 3_000
- `SCORE_APPROACH` = 100
- `SCORE_CENTER` = 50

For `m22_white` this produces:

```text
d6e5  10180   (unsupported bishop probe toward h8)
e3f4  5000    (pawn capture)
g4h5  5000    (pawn capture)
d6f4  2700    (bishop capture)
...
g4g5  110     (the key pawn storm)
Rg1e1 70      (the key rook centralization)
```

The 117-ply first PV is therefore not surprising: the solver finds a long win
through one of the high-scoring captures before it ever tries the quiet plan.

## Implementation tasks

### Phase A: Pawn-storm / passed-pawn bonus

In `src/search/ordering.rs`:

1. Add constants:
   ```rust
   const SCORE_PAWN_STORM: i32 = 2_000;
   const SCORE_PAWN_STORM_STEP: i32 = 100;
   ```
2. In `score_with_map`, after the kamikaze block and before the
   approach/center block, add a pawn-specific bonus:
   - The moving piece is a pawn.
   - The opponent has exactly one commoner (`state.them_commoners_count == 1`).
   - The destination is strictly closer to the lone commoner than the origin
     (`to_dist < from_dist` from the `nearest` map).
   - The pawn, after moving, attacks a square in the 3x3 zone around that
     commoner:
     ```rust
     attacks::king_attacks(commoner_sq) & attacks::pawn_attacks(us, to) != EMPTY
     ```
   Pseudocode:
   ```rust
   if from_pt == PieceType::Pawn && state.them_commoners_count == 1 {
       let from_dist = nearest[from as usize];
       let to_dist = nearest[to as usize];
       if to_dist < from_dist {
           let commoner_sq = board.commoners(them).pop_lsb();
           let attacks = attacks::pawn_attacks(us, to);
           if (attacks::king_attacks(commoner_sq) & attacks) != EMPTY {
               score += SCORE_PAWN_STORM
                       + i32::from(from_dist - to_dist) * SCORE_PAWN_STORM_STEP;
           }
       }
   }
   ```
   The exact bitboard helper for extracting the single commoner square may be
   `pop_lsb`, `bitscan`, or iteration; use whatever is already available in the
   `atomic_movegen` API.
3. Tune so that `g4g5` for `m22_white` rises into the top 5 while quiet a-pawn
   pushes (`a2a3`, `a2a4`) stay near the bottom. Start with the constants
   above and adjust by ±500 / ±50 based on `static_move_scores` output.

### Phase B: Heavy-piece centralization / open-file bonus

1. Add constants:
   ```rust
   const SCORE_ROOK_CENTER: i32 = 500;
   const SCORE_ROOK_OPEN_FILE: i32 = 400;
   const SCORE_ROOK_OPEN_FILE_STEP: i32 = 50;
   const SCORE_ROOK_BACK_RANK: i32 = 300;
   ```
2. In `score_with_map`, add a piece-type-specific block for rooks and queens
   after the pawn-storm block:
   - **Centralization**: multiply the existing `SCORE_CENTER` effect for rooks
     and queens. A rook/queen on a central square is much more valuable than a
     pawn on the same square. Add `SCORE_ROOK_CENTER * center` for `Rook` and
     `Queen` moves, where `center` is computed the same way as in the existing
     code.
   - **File/rank alignment with the lone commoner**: if `from_pt` is `Rook` or
     `Queen`, the destination shares a file or rank with the lone enemy commoner,
     and the ray between them is not blocked by any own piece, add
     `SCORE_ROOK_OPEN_FILE + SCORE_ROOK_OPEN_FILE_STEP * distance_reduction`.
     Use `attacks::rook_attacks(to, board.occupied())` and mask with the
     commoner square and the relevant file/rank bitboards. Exclude the `from`
     square because the piece is vacating it.
   - **Back-rank presence**: if a rook or queen lands on the enemy back rank
     (rank 8 for White, rank 1 for Black) and the enemy commoner is on or near
     that rank, add `SCORE_ROOK_BACK_RANK`.
3. Keep the logic cheap: at most one ray scan per move, and only for rooks/queens.

### Phase C: Threat-safety guard (optional)

If `d6e5` and similar unsupported bishop probes remain above the real plan
moves, guard the direct commoner threat bonus:

- Before awarding `SCORE_THREAT` or `SCORE_THREAT_LAST`, check whether the
  threatening piece on `to` can be immediately captured by an enemy piece.
- If it can, reduce the threat bonus by half.
- Use `board.attackers(to, them)` or `board.attacks_to` from `atomic_movegen` if
  it exists; otherwise defer this guard to a later plan.

This pushes `d6e5` (met by `…Rxe5`) below `g4g5` and `Rg1e1` without removing
the bonus for protected threats.

### Phase D: Measurement gate

After each phase run:

1. Diagnostics:
   ```bash
   cargo run --release --example static_move_scores -- --name m22_white
   cargo run --release --example move_order_debug -- --name m22_white --solve
   ```
   `g4g5` and `Rg1e1` should be in the top 5 static scores.
2. First-outcome benchmark:
   ```bash
   cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5 --runs 3
   ```
   Primary metric: mean `nodes` and mean time to first decisive outcome on
   `m22_white`. A shorter first PV is a secondary positive signal.
3. Refined benchmark:
   ```bash
   cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 3
   ```
   Check that `m24`–`m29` do not regress in `nodes`/`child_evals` and that
   `m22_white` becomes decisive or faster.
4. Regression tests:
   ```bash
   cargo test --release --test test_move_order
   cargo test --release --test stress move_order_hard -- --test-threads=2
   ```

Keep a phase if:
- It causes no wrong decisive outcomes on the suite.
- It reduces mean `nodes`/`child_evals`/first-outcome time on `m24`–`m29` by at
  least 5% averaged, **or**
- It makes `m22_white` decisive within the benchmark timeout.

Revert a phase immediately if it misclassifies a position, slows the solvable
suite, or fails `cargo test`.

### Phase E: Regression test updates (only if improvements land)

- If `m22_white` becomes reliably decisive in ≤10 seconds in the benchmark,
  update the fixture note in `tests/fixtures/move_order_positions.txt` and add a
  regression test in `tests/test_move_order.rs` with the documented timeout.
- If `m20_white` or `m21_white` become decisive, move them out of
  `tests/stress.rs` and into `tests/test_move_order.rs`.
- Add unit tests in `src/search/ordering.rs` covering the new behaviors, e.g.:
  - `m22_g4g5_scores_above_d6e5`
  - `m22_Rg1e1_scores_above_quiet_pawn_moves`

### Phase F: Final report

Write `docs/plans/move_order/report3.md` documenting:
- Which phases were implemented, reverted, or kept.
- Measured impact on `nodes`/`child_evals`/time for the move-order suite and
  the default suite.
- Final constant values.
- Problems encountered and unresolved edge cases.
- Next idea to evaluate (likely node-type-aware ordering or TT-bound-aware sort).

## File changes

- `src/search/ordering.rs` (main change)
- `tests/test_move_order.rs` (conditional)
- `tests/stress.rs` (conditional)
- `tests/fixtures/move_order_positions.txt` (conditional note update)
- `docs/plans/move_order/report3.md` (final deliverable)

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Pawn-storm bonus overvalues quiet a-pawn pushes. | Restrict to pawns whose destination or attack squares are within the 3x3 zone of the lone enemy commoner. Verify with `static_move_scores` on `m19`, `m22`, `m26`. |
| Rook-alignment bonus overvalues rooks on closed files. | Only award alignment when the ray from `to` to the commoner is not blocked by an own piece. Keep the bonus below true winning captures. |
| New scoring lifts distant plan moves above real captures. | Start constants below `SCORE_CAPTURE` (base 5_000) and tune until `g4g5`/`Rg1e1` beat `d6e5` but not `e3f4`/`g4h5` unless the benchmark proves otherwise. |
| Missing bitboard helpers in `atomic_movegen`. | Use existing `attacks::king_attacks`, `attacks::pawn_attacks`, `attacks::rook_attacks`, `Board::occupied`, and `Board::commoners`. Wrap alignment in a small helper. |
| Measurement noise from proof-tree worker. | Use `examples/benchmark` (no proof tree) and `--outcome-only` for `atomic_solver` diagnostics. |

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
cargo run --release --example static_move_scores -- --name m23_white
cargo run --release --example static_move_scores -- --name m26_white
cargo run --release --example move_order_debug -- --name m22_white --solve
cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5 --runs 3
cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 3
```

Regression and stress:

```bash
cargo test --release --test test_move_order
cargo test --release --test stress move_order_hard -- --test-threads=2
```

If any test returns a decisive outcome that does not match the fixture, the
current phase is a regression.

## Success criteria

1. `cargo test`, `cargo clippy --all-targets`, and `cargo doc` pass with no new
   warnings.
2. No wrong decisive outcomes on the move-order suite.
3. `g4g5` and `Rg1e1` appear in the top 5 static scores for `m22_white`.
4. Mean `nodes`/`child_evals`/first-outcome time on `m24`–`m29` does not
   regress; ideally improves by ≥5% or `m22_white` becomes decisive within the
   benchmark timeout.
5. If `m22` (or `m20`/`m21`) becomes reliably decisive, update the regression
   suite and fixture notes.
6. New unit tests cover the new pawn-storm and rook-alignment behavior.
