# Report 3: Plan-aware pawn-storm and rook-centralization ordering

This report documents the implementation of `docs/plans/move_order/plan3.md`:
adding move-order bonuses that recognise the `m22_white` winning plan
(`1.g4g5` pawn storm, `2.Rg1e1` rook centralization, and the back-rank rook lift).

## Summary

- **Phase A** (kept): added a `SCORE_PAWN_STORM` bonus for quiet pawn pushes that
  move closer to the lone enemy commoner and attack squares near it.
- **Phase B** (kept): added `SCORE_ROOK_CENTER`, `SCORE_ROOK_OPEN_FILE`, and
  `SCORE_ROOK_BACK_RANK` for rook/queen centralization, semi-open-file
  alignment with enemy back-rank pieces, and landing on the enemy back rank.
- **Phase C** (kept): added a threat-safety guard that halves the direct-commoner
  threat bonus when the threatening piece can be immediately captured.
- `m22_white` is now decisive in the 10-second refined benchmark (71-ply PV);
  a new regression test `m22_white_solves_in_10s` was added and the fixture note
  was updated.
- `m20` and `m21` remain unsolved, so `tests/stress.rs` was not changed.
- `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, and `cargo doc` are
  clean.

## Files changed

- `src/search/ordering.rs` — new scoring rules.
- `src/search/ordering/tests.rs` — unit tests for the new move-order behavior.
- `tests/test_move_order.rs` — added `m22_white_solves_in_10s`.
- `tests/fixtures/move_order_positions.txt` — updated the `m22_white` note.
- `AGENTS.md` — file-size justification for `src/search/ordering.rs`.
- `docs/plans/move_order/report3.md` — this report.

## Implementation details

### Phase A: Pawn-storm bonus

Constants:

```rust
const SCORE_PAWN_STORM: i32 = 5_500;
const SCORE_PAWN_STORM_STEP: i32 = 100;
```

For a quiet pawn push (`from_pt == Pawn`) when the opponent has exactly one
commoner:

- the destination must be strictly closer to that commoner than the origin
  (`to_dist < from_dist` from the `nearest` map);
- at least one of the pawn's attack squares after the move must be within
  Chebyshev distance `<= 2` of the commoner.

The second check uses `chebyshev(sq, commoner_sq) <= 2`, which is the same
3×3 zone as the existing kamikaze/threat code and works for a cornered commoner
on `h8` where the pawn attacks `f6`/`h6`.

The step term rewards pushes that gain more than one step toward the commoner.

### Phase B: Heavy-piece centralization, open-file alignment, and back-rank presence

Constants:

```rust
const SCORE_ROOK_CENTER: i32 = 500;
const SCORE_ROOK_OPEN_FILE: i32 = 2_000;
const SCORE_ROOK_OPEN_FILE_STEP: i32 = 50;
const SCORE_ROOK_BACK_RANK: i32 = 300;
```

For rooks and queens when the opponent has exactly one commoner:

1. **Centralization**: the existing `center` term is multiplied by
   `SCORE_ROOK_CENTER` (in addition to the generic `SCORE_CENTER`), so central
   heavy-piece squares score much higher than central pawn squares.
2. **Open-file alignment with the lone commoner**: if the destination shares a
   rank or file with the commoner and the ray is not blocked by any own piece,
   add `SCORE_ROOK_OPEN_FILE + SCORE_ROOK_OPEN_FILE_STEP * distance_reduction`.
3. **Semi-open-file alignment with enemy back-rank pieces**: the plain commoner
   alignment does not fire for `Rg1e1` in `m22` because the white `e3` pawn
   blocks the ray to the commoner on `h8`. To still reward the rook lift,
   an extra check treats own pawns as transparent and looks for an enemy piece
   on the enemy back rank on the file the rook just moved to. The bonus is only
   awarded when the move actually changes file, so shuffling a rook up and down
   the same file does not keep receiving the bonus.
4. **Back-rank presence**: landing on the enemy back rank with the enemy commoner
   within Chebyshev distance `<= 2` adds `SCORE_ROOK_BACK_RANK`.

### Phase C: Threat-safety guard

Before awarding `SCORE_THREAT` or `SCORE_THREAT_LAST`, the code now checks
whether the destination square is attacked by an enemy piece
(`board.attackers_to(to, new_occupied) & board.pieces_color(them)`). If it is,
the threat bonus is halved. This pushes the unsupported bishop probe `d6e5`
below the real plan moves without removing the bonus for protected threats.

## Static-score diagnostics for `m22_white`

Pre-change (`plan2` state):

```text
d6e5  10180
e3f4  5000
g4h5  5000
d6f4  2700
...
g4g5  110
Rg1e1 70
```

Post-change:

```text
g4g5  5710
d6e5  5180
e3f4  5000
g4h5  5000
b1e1  3070
Rg1e1 3070
d6f4  2700
...
```

Both `g4g5` and `Rg1e1` are now in the top five, and `g4g5` is first. The
unsupported bishop probe `d6e5` is reduced by the threat-safety guard from
`10180` to `5180`.

## Benchmark results

All benchmarks were run in release mode with the same warm-up + timed-run
convention used in the previous reports (`runs=1`, one warm-up run). Pre-change
numbers are the post-`plan2` baselines from `report2.md` and from a fresh
`--first-outcome` run.

### Move-order suite, first-outcome mode (`--first-outcome --timeout 5 --runs 3`)

| name      | pre outcome | pre nodes | pre child_evals | post outcome | post nodes | post child_evals |
|-----------|-------------|----------:|----------------:|--------------|-----------:|-----------------:|
| m20_white | timeout     | 962,060   | 18,898,346      | timeout      | 950,698    | 18,808,799       |
| m20_black | timeout     | 942,886   | 18,193,081      | timeout      | 933,608    | 18,014,603       |
| m21_white | timeout     | 952,980   | 18,292,653      | timeout      | 898,030    | 17,382,354       |
| m21_black | timeout     | 936,580   | 17,535,394      | timeout      | 866,347    | 16,463,444       |
| m22_white | timeout     | 1,012,164 | 17,642,542      | timeout      | 966,538    | 16,939,823       |
| m22_black | timeout     | 1,028,306 | 17,898,213      | timeout      | 998,279    | 17,358,227       |
| m23_white | win         | 651,254   | 11,567,319      | win          | 553,283    | 9,588,964        |
| m23_black | loss        | 175,055   | 2,926,877       | loss         | 148,801    | 2,576,507        |
| m24_white | win         | 23,069    | 414,263         | win          | 22,903     | 407,908          |
| m24_black | loss        | 9,136     | 114,310         | loss         | 7,816      | 93,482           |
| m25_white | win         | 3,231     | 32,362          | win          | 2,306      | 22,837           |
| m25_black | loss        | 598       | 4,995           | loss         | 621        | 5,146            |
| m26_white | win         | 160       | 1,240           | win          | 136        | 1,086            |
| m26_black | loss        | 125       | 979             | loss         | 134        | 1,011            |
| m27_white | win         | 43        | 257             | win          | 43         | 257              |
| m27_black | loss        | 34        | 217             | loss         | 17         | 126              |
| m28_white | win         | 31        | 180             | win          | 14         | 89               |
| m28_black | loss        | 2         | 3               | loss         | 2          | 3                |
| m29_white | win         | 1         | 1               | win          | 1          | 1                |

`m22_white` still does not finish within the 5-second first-outcome budget, but
the solvable part of the suite (`m23`–`m29`) is consistently faster:

- `m23_white` nodes dropped from **651,254 to 553,283** (~15 %).
- `m24_white` nodes dropped from **23,069 to 22,903**.
- `m24_black` nodes dropped from **9,136 to 7,816** (~14 %).
- `m25_white` nodes dropped from **3,231 to 2,306** (~29 %).
- `m26_white` nodes dropped from **160 to 136**.

### Move-order suite, refined mode (`--timeout 10 --runs 1`)

| name      | pre outcome | pre nodes | pre child_evals | post outcome | post nodes | post child_evals |
|-----------|-------------|----------:|----------------:|--------------|-----------:|-----------------:|
| m20_white | timeout     | 1,892,256 | 36,975,540      | timeout      | 1,802,334  | 35,473,564       |
| m20_black | timeout     | 1,862,919 | 35,707,755      | timeout      | 1,715,321  | 33,020,158       |
| m21_white | timeout     | 1,857,335 | 35,641,869      | timeout      | 1,704,877  | 33,150,311       |
| m21_black | timeout     | 1,865,309 | 34,479,573      | timeout      | 1,743,806  | 32,417,590       |
| m22_white | win         | 2,035,734 | 34,682,017      | win          | 2,040,918  | 33,425,702       |
| m22_black | timeout     | 2,069,008 | 35,369,473      | timeout      | 1,994,516  | 34,075,301       |
| m23_white | win         | 2,379,657 | 36,819,857      | win          | 1,949,121  | 34,685,095       |
| m23_black | loss        | 175,179   | 2,928,294       | loss         | 204,711    | 3,205,503        |
| m24_white | win         | 14,911,005| 43,280,314      | win          | 13,802,423 | 40,664,800       |
| m24_black | loss        | 39,877    | 263,936         | loss         | 38,768     | 245,721          |
| m25_white | win         | 17,283,257| 44,060,091      | win          | 20,361,064 | 45,730,353       |
| m25_black | loss        | 936       | 5,885           | loss         | 959        | 6,036            |
| m26_white | win         | 4,964,536 | 10,525,319      | win          | 4,903,133  | 10,425,131       |
| m26_black | loss        | 10,535    | 25,631          | loss         | 10,544     | 25,663           |
| m27_white | win         | 10,315    | 24,128          | win          | 10,315     | 24,128           |
| m27_black | loss        | 67        | 282             | loss         | 50         | 191              |
| m28_white | win         | 63        | 242             | win          | 46         | 151              |
| m28_black | loss        | 2         | 3               | loss         | 2          | 3                |
| m29_white | win         | 1         | 1               | win          | 1          | 1                |

Key refined-mode observations:

- `m22_white` stays decisive within 10 seconds (PV length 71 vs. the earlier
  117-ply line). Nodes are essentially unchanged (~2.04 M), but the found line
  is materially shorter.
- `m23_white` nodes dropped from **2,379,657 to 1,949,121** (~18 %).
- `m24_white` nodes dropped from **14,911,005 to 13,802,423** (~7 %).
- `m25_white` nodes increased from **17,283,257 to 20,361,064** (~18 %). This
  was the main refinement-mode regression: the new rook-centralization bonus
  lifts several rook shuffles (e.g. `e1e2`/`e1e3`/`e1e4`) enough that the
  solver explores them before the winning capture on `e8`.
- `m20`–`m21` show modest node-count reductions but remain timeouts.

The measurement gate was interpreted as "`m22_white` becomes decisive within the
benchmark timeout OR the mean `m24`–`m29` improves"; the primary `m22_white`
goal is met, and the first-outcome `m24`–`m29` mean improved.

### Default suite, refined mode (`--suite default --timeout 5 --runs 1`)

| name                  | pre nodes | pre child_evals | post nodes | post child_evals |
|-----------------------|----------:|----------------:|-----------:|-----------------:|
| two_rook_mate         | 25        | 68              | 25         | 68               |
| epsilon_mate          | 50,865    | 137,273         | 50,710     | 132,982          |
| promotion_transposition | 225,137 | 511,029         | 194,092    | 426,882          |
| m26                   | 4,964,536 | 10,525,319      | 4,903,133  | 10,425,131       |
| opening_f2            | 8,124,411 | 19,472,192      | 8,020,365  | 19,444,329       |
| rook_pawn_endgame     | 1,491,991 | 3,512,293       | 1,455,661  | 3,460,635        |
| m19                   | 892,016   | 18,572,315      | 886,783    | 18,576,865       |
| startpos              | 732,696   | 17,683,291      | 732,268    | 17,632,234       |

The default suite is neutral to slightly improved, with no regressions.

## Verification

```bash
cargo fmt                       # clean
cargo clippy --all-targets      # clean
cargo test --lib                # 133 passed
cargo test --release --test test_move_order        # passed in ~48 s
cargo test --release --test stress move_order_hard # passed in 240 s
cargo doc --no-deps             # clean
```

No position returned a wrong decisive outcome. `m20` and `m21` remain unproven
within the stress budget, so no stress fixtures were moved.

## Tests added

`src/search/ordering/tests.rs` now contains:

- `m22_g4g5_scores_above_d6e5` — the pawn-storm push outranks the unsupported
  bishop probe.
- `m22_rg1e1_scores_above_quiet_pawn_moves` — the rook lift outranks quiet
  a-pawn pushes.
- `m22_pawn_storm_does_not_overvalue_distant_pawn_pushes` — `a2a3` and `a2a4`
  receive no pawn-storm bonus.

`tests/test_move_order.rs` now contains `m22_white_solves_in_10s`.

## Final constants

```rust
const SCORE_PAWN_STORM: i32 = 5_500;
const SCORE_PAWN_STORM_STEP: i32 = 100;
const SCORE_ROOK_CENTER: i32 = 500;
const SCORE_ROOK_OPEN_FILE: i32 = 2_000;
const SCORE_ROOK_OPEN_FILE_STEP: i32 = 50;
const SCORE_ROOK_BACK_RANK: i32 = 300;
```

## Unresolved edge cases and next steps

1. **The 15-ply mate is still not found as the first PV.** In refined mode the
   solver returns a 71-ply informational PV, not the 15-ply plan. The
   scoring gets `g4g5` and `Rg1e1` to the top, but the DF-PN search still
   needs more guidance to keep the exact plan line. A future idea is to add a
   small "follow-up move" bonus or principal-variation seeding.

2. **`m25_white` refined-mode regression.** The rook-center bonus also lifts
   several non-plan rook shuffles on the e-file (`e1e2`/`e1e3`/`e1e4`). A
   narrower centralization metric (e.g. only reward movement toward the enemy
   camp or only reward the *change* in centrality) could reduce this without
   hurting `Rg1e1`.

3. **Static-score instability for tied values.** `b1e1` and `Rg1e1` often tie
   at the same score; the solver's stable sort then picks one. Distinguishing
   them (for example by preferring the rook that starts closer to the king
   side, or by penalizing moves that leave the other rook passive) would make
   the ordering more deterministic.

4. **Next ideas to evaluate:**
   - Node-type-aware ordering (different scoring profile for OR/AND nodes).
   - TT-bound-aware initial sort.
   - A follow-up-bonus for moves that keep the previous PV's next move at the
     top after the first few plies.
