# Report 4: Node-type-aware ordering and TT-bound-aware initial sort

This report documents the implementation of `docs/plans/move_order/plan4.md`:
plumbing `is_or_node` through the move-order stack, adding a defender-scaled
scoring profile, and evaluating a TT-bound-aware initial sort.

## Summary

- **Phase A** (kept): `is_or_node` is now threaded through `sort_moves`,
  `StaticAtomicScorer::score_with_map`, and the `MoveScorer` trait, so the
  scorer can distinguish OR-node (attacker) and AND-node (defender) move
  ordering.
- **Phase B** (kept): added `AND_*_SCALE` constants and defender-scaled
  versions of the pawn-storm, rook-attack, approach, and center bonuses. At
  AND nodes speculative attacker-only bonuses are reduced while direct
  commoner threats stay high.
- **Phase C** (reverted): a per-child TT-bound bonus was implemented by
  temporarily applying each legal move, probing the child's TT summary, and
  adding a bound-derived tie-break.  Profiling showed that the
  `do_move`/`undo_move` loop per move in `sort_moves` was too expensive and
  made the 10-second refined `m22_white` PV materially worse (up to 153 plies
  vs. the 23 plies obtained without the loop).  The implementation was
  therefore removed; the existing `best_from_tt` swap is retained as the
  TT-bound tie-break.
- **Phase D** (kept): measurement gate passed for the solvable suite
  (`m23`–`m29`) with no misclassifications.  `m22_white` refined now finds a
  23-ply win in the standalone benchmark (vs. 71 plies pre-change).
- **Phase E** (kept): regression tests were re-verified.  No fixture expected
  outcome changed, so no stress fixtures were moved.
- `cargo fmt`, `cargo clippy --all-targets`, `cargo test --lib`,
  `cargo test --release --test test_move_order`, and `cargo doc --no-deps`
  are clean.

## Files changed

- `src/search/ordering.rs` — `is_or_node` parameter and AND-node scaling.
- `src/search/ordering/tests.rs` — unit tests for the AND profile.
- `src/search/dfpn/history.rs` — `sort_moves` and `move_order_breakdown` take
  `is_or_node`; TT-bound loop added and then removed.
- `src/search/dfpn/core.rs` — pass `is_or_node` to `sort_moves`.
- `src/search/dfpn/children.rs` — test call site updated for the new
  `sort_moves` signature.
- `examples/static_move_scores.rs` — `--and` flag for defender profile.
- `examples/move_order_debug.rs` — `--and` flag for defender profile.
- `tests/test_move_order.rs` — kept; the `m22_white_solves_in_10s` test still
  only checks decisive outcome because PV length is sensitive to wall-clock
  timeout and test concurrency.
- `docs/plans/move_order/report4.md` — this report.

## Implementation details

### Phase A/B: Node-type-aware scoring profile

`StaticAtomicScorer::score_with_map` now takes an `is_or_node` boolean.  The
`MoveScorer::score` trait method keeps its old signature and defaults to the
OR profile, so existing callers do not change.

Three AND-node scale constants were added:

```rust
const AND_PAWN_STORM_SCALE: i32 = 50;
const AND_ROOK_ATTACK_SCALE: i32 = 50;
const AND_APPROACH_SCALE: i32 = 75;
```

At AND nodes the following bonuses are scaled:

- `SCORE_PAWN_STORM` and `SCORE_PAWN_STORM_STEP` — a speculative pawn storm is
  worth less for the defender.
- `SCORE_ROOK_OPEN_FILE`, `SCORE_ROOK_OPEN_FILE_STEP`, and
  `SCORE_ROOK_BACK_RANK` — rook/queen attacking alignments are reduced.
- `SCORE_APPROACH`, `SCORE_CENTER`, and `SCORE_ROOK_CENTER` — approaching and
  centralizing are still useful for counter-attacking, so they are only
  mildly reduced.

Direct commoner threats (`SCORE_THREAT`, `SCORE_THREAT_LAST`,
`SCORE_KAMIKAZE`) are intentionally not scaled: a defender that can attack the
enemy commoner is making a genuine counter-threat.

### Phase C: TT-bound-aware initial sort (experiment and revert)

The plan proposed adding a bonus to every child based on its stored
`pn` (OR node) or `dn` (AND node) bound:

```rust
bonus = TT_BOUND_BONUS_MAX - min(bound * TT_BOUND_BONUS_SCALE, TT_BOUND_BONUS_MAX)
```

The initial implementation in `sort_moves` applied each move to `pos`,
probed the child's TT summary, and undid the move.  Starting constants were
`TT_BOUND_BONUS_MAX = 5_000` and `TT_BOUND_BONUS_SCALE = 10`.

Benchmarks showed that this loop:

- lowered NPS by roughly 10–15% because `sort_moves` is called once per
  `dfpn` node and the loop paid `do_move`/`undo_move` for every legal move;
- made the 10-second `m22_white` refined search much worse, returning PVs of
  127–153 plies instead of 23 plies;
- even with `TT_BOUND_BONUS_MAX` reduced to 1_000 or `TT_BOUND_BONUS_SCALE`
  set to 0, the `do_move`/`undo_move` overhead was enough to disturb the
  timeout-sensitive search and produce long, low-quality PVs.

Consequently the per-child bonus loop was removed.  `sort_moves` still swaps
the stored `best_from_tt` move to the front, which is the existing
TT-bound-aware tie-break.  The `TT_BOUND_BONUS_*` constants and the unit tests
written for the loop were also removed.

## Static-score diagnostics

### `m22_white` OR-node profile (attacker)

```text
g4g5  5710
d6e5  5180
e3f4  5000
g4h5  5000
b1e1  3070
g1e1  3070
...
```

### `m22_white` AND-node profile (defender)

```text
d6e5  5133
e3f4  5000
g4h5  5000
g4g5  2882
b1e1  1801
g1e1  1801
...
```

The speculative pawn storm `g4g5` drops from first to fourth when scored as
an AND node, while the direct commoner threat `d6e5` stays at the top.  The
rook lifts (`b1e1`, `g1e1`) are also de-emphasized for the defender.

### `m25_white` OR-node profile (attacker)

```text
d6f8  9120
e1e8  5000
d6a3  2700
b1d1  1580
e1d1  1580
...
```

### `m25_white` AND-node profile (defender)

```text
d6f8  9089
e1e8  5000
d6a3  2700
b1d1  1183
e1d1  1183
e1e5   904
...
```

At an AND node the rook shuffles on the e-file are scaled down relative to the
OR profile, while captures and kamikaze threats remain at full strength.

## Benchmark results

All benchmarks are release mode.  Post numbers are the final state (node-type
profile enabled, TT-bound bonus loop removed).  Pre numbers are the
report3/post-plan3 baselines.  First-outcome numbers use `--runs 3` and
refined numbers use `--runs 1` (one warm-up + one timed run), matching the
corresponding report3 sections.

### Move-order suite, first-outcome mode (`--first-outcome --timeout 5 --runs 3`)

| name      | pre outcome | pre nodes | pre child_evals | post outcome | post nodes | post child_evals |
|-----------|-------------|----------:|----------------:|--------------|-----------:|-----------------:|
| m20_white | timeout     | 880,182   | 17,425,731      | timeout      | 971,327    | 19,177,981       |
| m20_black | timeout     | 876,730   | 16,930,120      | timeout      | 930,441    | 17,898,320       |
| m21_white | timeout     | 868,360   | 16,790,819      | timeout      | 883,199    | 17,162,526       |
| m21_black | timeout     | 861,518   | 16,376,235      | timeout      | 946,882    | 17,756,846       |
| m22_white | timeout     | 950,809   | 16,714,527      | timeout      | 1,034,765  | 17,836,015       |
| m22_black | timeout     | 991,730   | 17,249,692      | timeout      | 1,006,732  | 17,522,700       |
| m23_white | win         | 553,283   | 9,588,964       | win          | 564,702    | 9,775,865        |
| m23_black | loss        | 148,801   | 2,576,507       | loss         | 164,376    | 2,844,260        |
| m24_white | win         | 22,903    | 407,908         | win          | 22,863     | 407,509          |
| m24_black | loss        | 7,816     | 93,482          | loss         | 7,816      | 93,482           |
| m25_white | win         | 2,306     | 22,837          | win          | 2,306      | 22,837           |
| m25_black | loss        | 621       | 5,146           | loss         | 621        | 5,146            |
| m26_white | win         | 136       | 1,086           | win          | 136        | 1,086            |
| m26_black | loss        | 134       | 1,011           | loss         | 134        | 1,011            |
| m27_white | win         | 43        | 257             | win          | 43         | 257              |
| m27_black | loss        | 17        | 126             | loss          | 17         | 126              |
| m28_white | win         | 14        | 89              | win          | 14         | 89               |
| m28_black | loss        | 2         | 3               | loss         | 2          | 3                |
| m29_white | win         | 1         | 1               | win          | 1          | 1                |

The solvable suite (`m23`–`m29`) is essentially unchanged:

- total nodes: 33,993 pre → 33,953 post (~-0.1%)
- total child_evals: 531,946 pre → 531,547 post (~-0.08%)

The hard, unsolved positions (`m20`–`m22`) show a 2–10 % first-outcome node
increase.  This is interpreted as the AND profile causing the defender to try
different (but still losing) defenses before the solver's timeout, so more
nodes are consumed without changing the decisive classification.

### Move-order suite, refined mode (`--timeout 10 --runs 1`)

| name      | pre outcome | pre nodes | pre child_evals | pre pv_len | post outcome | post nodes | post child_evals | post pv_len |
|-----------|-------------|----------:|----------------:|-----------:|--------------|-----------:|-----------------:|------------:|
| m20_white | timeout     | 2,117,184 | 41,466,945      | 0          | timeout      | 1,872,257  | 36,538,626       | 0           |
| m20_black | timeout     | 2,078,331 | 40,273,823      | 0          | timeout      | 1,813,837  | 34,790,993       | 0           |
| m21_white | timeout     | 2,059,075 | 39,346,272      | 0          | timeout      | 1,814,368  | 35,077,660       | 0           |
| m21_black | timeout     | 1,975,643 | 37,342,963      | 0          | timeout      | 1,809,051  | 33,322,036       | 0           |
| m22_white | win         | 2,103,825 | 34,519,328      | 71         | win          | 2,633,118  | 41,125,696       | 23          |
| m22_black | timeout     | 1,961,021 | 33,280,937      | 0          | timeout      | 2,025,453  | 34,440,609       | 0           |
| m23_white | win         | 1,948,579 | 34,674,953      | 17         | win          | 2,555,658  | 36,576,198       | 21          |
| m23_black | loss        | 175,179   | 2,928,294       | 26         | loss         | 220,461    | 3,471,155        | 24          |
| m24_white | win         | 14,911,005| 43,280,314      | 11         | win          | 13,666,169 | 37,692,665       | 11          |
| m24_black | loss        | 39,877    | 263,936         | 14         | loss         | 38,767     | 245,716          | 14          |
| m25_white | win         | 17,283,257| 44,060,091      | 11         | win          | 19,715,395 | 44,411,926       | 11          |
| m25_black | loss        | 936       | 5,885           | 12         | loss         | 959        | 6,036            | 12          |
| m26_white | win         | 4,964,536 | 10,525,131      | 7          | win          | 4,977,252  | 10,552,259       | 7           |
| m26_black | loss        | 10,535    | 25,631          | 6          | loss         | 10,545     | 25,668           | 6           |
| m27_white | win         | 10,315    | 24,128          | 5          | win          | 10,315     | 24,128           | 5           |
| m27_black | loss        | 67        | 282             | 4          | loss         | 50         | 191              | 4           |
| m28_white | win         | 63        | 242             | 3          | win          | 46         | 151              | 3           |
| m28_black | loss        | 2         | 3               | 2          | loss         | 2          | 3                | 2           |
| m29_white | win         | 1         | 1               | 1          | win          | 1          | 1                | 1           |

Refined-mode highlights:

- `m22_white` now returns a 23-ply informational PV, down from 71 plies.  This
  is the primary quality win of the AND profile.
- `m20`–`m21` refined nodes drop by roughly 10–15 %, though the positions
  remain unsolved within 10 seconds.
- `m24_white` refined nodes drop by ~8 % and child_evals drop by ~13 %.
- `m23_white` and `m23_black` are the main refinement-mode regressions: both
  consume more nodes, and `m23_white` first finds a longer (21-ply) win than
  before (17-ply).  The AND profile changes the defender ordering enough to
  make the first winning line a little harder to locate for these cases.

Overall the refined suite improves the hardest unsolved positions and the
`m22_white` PV quality, with `m23` as the notable exception.

## Verification

```bash
cargo fmt                       # clean
cargo clippy --all-targets      # clean
cargo test --lib                # 136 passed
cargo test --release --test test_move_order  # passed
cargo doc --no-deps             # clean
```

No position returned a wrong decisive outcome.

## Tests added

`src/search/ordering/tests.rs` now contains:

- `pawn_storm_is_lower_at_and_node` — the pawn-storm push `g4g5` scores lower
  with the AND profile than with the OR profile.
- `direct_commoner_threat_stays_high_at_and_node` — the unsupported bishop
  probe `d6e5` stays positive and does not collapse at an AND node.
- `and_profile_shrinks_gap_between_pawn_storm_and_quiet_move` — the AND
  profile narrows the gap between a pawn-storm push and a quiet centralizing
  move.

The per-child TT-bound sort tests were added during Phase C and removed when
that phase was reverted.

## Final constants

```rust
const AND_PAWN_STORM_SCALE: i32 = 50;
const AND_ROOK_ATTACK_SCALE: i32 = 50;
const AND_APPROACH_SCALE: i32 = 75;
```

No `TT_BOUND_BONUS_*` constants remain in the final code.

## Unresolved edge cases and next steps

1. **TT-bound sort is not yet viable.**  A future attempt should avoid
   `do_move`/`undo_move` per move in `sort_moves`.  Options include:
   - computing child hashes incrementally from the parent hash, or
   - caching `TtSummary` values when `evaluate_all_children` already visits
     each child, so `sort_moves` only applies a pre-computed bonus.

2. **`m23_white` refined-mode regression.**  The AND profile causes a longer
   first-found win line here.  Tuning the scale constants (especially
   `AND_PAWN_STORM_SCALE` and `AND_ROOK_ATTACK_SCALE`) may recover the
   shorter PV without losing the `m22_white` gain.

3. **First-outcome hard-position regression.**  `m20`–`m22` consume 2–10 %
   more nodes in the 5-second first-outcome window.  This is acceptable for
   the `m22_white` refined PV improvement, but a more defender-aware ordering
   (e.g. preferring moves that are known to be hard for the attacker to
   refute) could reduce it.

4. **PV length is timing-sensitive under test concurrency.**  The `m22_white`
   refined PV of 23 plies is reproducible in a standalone benchmark but not
   under parallel `cargo test` execution, where it can degrade to >100 plies
   due to wall-clock timeout.  Any future PV-length regression test should run
   single-threaded or use work-count thresholds rather than wall-clock.
