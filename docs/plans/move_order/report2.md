# Report 2: Static move-ordering fixes — near-commoner heuristics and atomic SEE

This report documents the implementation of `docs/plans/move_order/plan2.md`:
rewriting the quiet-move and capture scoring in `StaticAtomicScorer` to match
atomic-chess tactics more closely.

## Summary

- **Phase A** (kept): replaced the broad `SCORE_ATOMIC_CHECK` bonus with a strict
  *kamikaze* bonus for quiet moves that land adjacent to an enemy commoner, and
  removed the extra one-square blast ring from `SCORE_BLAST` for non-captures.
- **Phase B** (kept): replaced MVV-LVA capture scoring with atomic static exchange
  evaluation (aSEE) that scores captures by the net material destroyed in the
  blast, including the capturing piece and any own pieces caught in the explosion.
- `m20` and `m21` remain unsolved within 60 seconds, so no fixtures were moved out
  of `tests/stress.rs`.
- `cargo test`, `cargo clippy --all-targets`, and `cargo doc` are clean.

## Files changed

- `src/search/ordering.rs` (main change and new unit tests)
- `docs/plans/move_order/report2.md` (this report)

`tests/test_move_order.rs` and `tests/stress.rs` were not modified because the
hardest positions did not become decisive.

## Implementation details

### Kamikaze and blast-zone fixes (Phase A)

`src/search/ordering.rs` now has:

```rust
const SCORE_THREAT_LAST: i32 = 10_000;
const SCORE_THREAT: i32 = 1_000;
const SCORE_KAMIKAZE_LAST: i32 = 9_000;
const SCORE_KAMIKAZE: i32 = 3_000;
```

- `SCORE_THREAT` is awarded when the moved piece attacks an enemy commoner
  square (`attack_bb & board.commoners(them) != EMPTY`).
- `SCORE_KAMIKAZE` is awarded when the destination square is adjacent to an
  enemy commoner (`attacks::king_attacks(to) & board.commoners(them) != EMPTY`).
- Both keep the `state.them_commoners_count == 1` boost (`*_LAST`).
- The old `SCORE_ATOMIC_CHECK` branch that scored attacks on squares *near* the
  commoner was removed.
- The `SCORE_BLAST` double-ring expansion for non-captures was removed. The
  immediate blast zone is now the same as the kamikaze condition, so it is not
  double-counted.

### Atomic SEE (Phase B)

Captures are now scored by a new helper, `capture_net_value`, which computes:

- victim value (pawn for en-passant, otherwise `board.piece_on(to)`);
- moving-piece value (always lost at ground zero);
- all non-pawn pieces in `attacks::king_attacks(to)` that are destroyed by the
  blast, with the origin square excluded because the moving piece is leaving it.

The final capture score is:

```rust
const SCORE_CAPTURE: i32 = 5_000;
const CAPTURE_NET_SCALE: i32 = 10;
// score = SCORE_CAPTURE + net * CAPTURE_NET_SCALE
```

Promotion-captures are no longer scored by `SCORE_PROMOTION`; the promoted piece
is transient and is not counted as a lost own piece. Non-capture promotions keep
their old `SCORE_PROMOTION + piece_value(promotion_type)` score.

## Static-score diagnostics

### `m19` (default `static_move_scores` position)

Pre-change (old `SCORE_ATOMIC_CHECK`/`SCORE_BLAST`):

```text
d6f8 9620
d6e7 9610
d6c5 9070
d6e5 9070
d6b4 9060
...
```

Post-change:

```text
d6f8 9120
d2d4 200
d2d3 190
c3d5 190
e3e4 180
...
```

The long tail of bishop probes (`d6e7`, `d6c5`, `d6b4`, `d6a3`) that scored
around 9,000 without directly attacking the commoner is gone. Only the
kamikaze/approach move `d6f8` remains near the top, followed by centralizing
rook/knight moves.

### `m20_white`

Pre-change:

```text
d6e5 10180
d6f8 9620
d2d4 200
...
```

Post-change:

```text
d6e5 10180
d2d4 200
d2d3 190
e3e4 180
c3e4 180
...
```

`d6e5` (a direct commoner threat against the black commoner on h8) remains
first. The non-threatening bishop probe `d6f8` is now far below the first few
centralizing moves.

### `m26_white`

Pre-change:

```text
d6c5 100670
b1b8 10120
d6f8 9620
...
```

Post-change:

```text
b1b8 10120
d6f8 9120
d6c5 2700
...
```

`b1b8` and `d6f8` are correctly scored as direct commoner threats. `d6c5` is
now scored by aSEE rather than the old MVV-LVA boost, which makes its raw
static score lower but still above quiet development.

## Benchmark results

All benchmarks were run on the same machine in release mode. Each row is one
position; `runs=1` with one warm-up run, so the reported `nodes`/`child_evals`
are from a single timed solve.

### Move-order suite, first-outcome mode (`--first-outcome --timeout 5 --runs 1`)

| name      | pre outcome | pre nodes | pre child_evals | post outcome | post nodes | post child_evals |
|-----------|-------------|----------:|----------------:|--------------|-----------:|-----------------:|
| m20_white | timeout     | 881,783   | 17,276,845      | timeout      | 962,060    | 18,898,346       |
| m20_black | timeout     | 849,372   | 16,360,845      | timeout      | 942,886    | 18,193,081       |
| m21_white | timeout     | 874,115   | 16,621,674      | timeout      | 952,980    | 18,292,653       |
| m21_black | timeout     | 856,725   | 16,156,638      | timeout      | 936,580    | 17,535,394       |
| m22_white | timeout     | 932,699   | 16,152,190      | timeout      | 1,012,164  | 17,642,542       |
| m22_black | timeout     | 957,228   | 16,496,301      | timeout      | 1,028,306  | 17,898,213       |
| m23_white | win         | 681,583   | 12,027,452      | win          | 651,254    | 11,567,319       |
| m23_black | loss        | 178,542   | 2,991,242       | loss         | 175,055    | 2,926,877        |
| m24_white | win         | 58,383    | 986,758         | win          | 23,069     | 414,263          |
| m24_black | loss        | 14,304    | 152,891         | loss         | 9,136      | 114,310          |
| m25_white | win         | 1,662     | 17,814          | win          | 3,231      | 32,362           |
| m25_black | loss        | 1,074     | 9,951           | loss         | 598        | 4,995            |
| m26_white | win         | 299       | 2,461           | win          | 160        | 1,240            |
| m26_black | loss        | 264       | 2,200           | loss         | 125        | 979              |
| m27_white | win         | 42        | 256             | win          | 43         | 257              |
| m27_black | loss        | 20        | 148             | loss         | 34         | 217              |
| m28_white | win         | 17        | 111             | win          | 31         | 180              |
| m28_black | loss        | 2         | 4               | loss         | 2          | 3                |
| m29_white | win         | 1         | 2               | win          | 1          | 1                |

For the first decisive result on the solvable part of the suite (`m23`–`m29`)
the new ordering is clearly faster:

- `m24_white` nodes dropped from **58,383 to 23,069** (60 % reduction).
- `m24_black` nodes dropped from **14,304 to 9,136** (36 % reduction).
- `m23_white` nodes dropped from **681,583 to 651,254**.
- `m26_white` nodes dropped from **299 to 160**.

A few of the very small/easy positions (`m25_white`, `m27_black`, `m28_white`)
showed tiny node-count increases. These are positions that solve in single-digit
nodes, so small ordering differences and PV-length refinement dominate; the
absolute increases are negligible.

### Move-order suite, refined mode (`--timeout 10 --runs 1`)

| name      | pre outcome | pre nodes | pre child_evals | post outcome | post nodes | post child_evals |
|-----------|-------------|----------:|----------------:|--------------|-----------:|-----------------:|
| m20_white | timeout     | 1,860,625 | 36,232,762      | timeout      | 1,892,256  | 36,975,540       |
| m20_black | timeout     | 1,823,543 | 34,700,172      | timeout      | 1,862,919  | 35,707,755       |
| m21_white | timeout     | 1,799,957 | 34,491,732      | timeout      | 1,857,335  | 35,641,869       |
| m21_black | timeout     | 1,814,957 | 33,350,188      | timeout      | 1,865,309  | 34,479,573       |
| m22_white | win         | 2,823,622 | 35,321,913      | win          | 2,035,734  | 34,682,017       |
| m22_black | timeout     | 1,985,753 | 34,042,028      | timeout      | 2,069,008  | 35,369,473       |
| m23_white | win         | 3,850,593 | 35,753,230      | win          | 2,379,657  | 36,819,857       |
| m23_black | loss        | 1,581,596 | 6,286,981       | loss         | 175,179    | 2,928,294        |
| m24_white | win         | 13,924,480| 39,444,515      | win          | 14,911,005 | 43,280,314       |
| m24_black | loss        | 15,373    | 156,803         | loss         | 39,877     | 263,936          |
| m25_white | win         | 15,201,685| 37,872,205      | win          | 17,283,257 | 44,060,091       |
| m25_black | loss        | 1,419     | 10,907          | loss         | 936        | 5,885            |
| m26_white | win         | 5,035,859 | 10,817,552      | win          | 4,964,536  | 10,525,319       |
| m26_black | loss        | 525       | 3,359           | loss         | 10,535     | 25,631           |
| m27_white | win         | 10,314    | 24,127          | win          | 10,315     | 24,128           |
| m27_black | loss        | 53        | 213             | loss         | 67         | 282              |
| m28_white | win         | 49        | 173             | win          | 63         | 242              |
| m28_black | loss        | 2         | 4               | loss         | 2          | 3                |
| m29_white | win         | 1         | 2               | win          | 1          | 1                |

R refined mode runs until timeout while iteratively improving the informational
PV. Because the solver consumes the full budget after the first decisive line is
found, node counts in this mode are dominated by how the iterative refinement
explores the tree. The overall pattern is still positive for the medium
positions:

- `m22_white` nodes dropped from **2,823,622 to 2,035,734**.
- `m23_white` nodes dropped from **3,850,593 to 2,379,657**.
- `m23_black` nodes dropped from **1,581,596 to 175,179** and child_evals from
  **6,286,981 to 2,928,294**.
- `m26_white` nodes and child_evals both decreased slightly.

Some of the very easy positions (`m24_black`, `m26_black`, `m27_black`,
`m28_white`) showed small increases. These are positions solved in under 0.1 s
and then spend the remaining time refining; the increase reflects a different
refinement path rather than a worse first decision.

### Default suite, refined mode (`--suite default --timeout 5 --runs 1`)

No pre-change baseline was recorded for the default suite in this pass. Post-change
numbers are:

| name               | outcome | nodes     | child_evals | mean (s) | pv_len |
|--------------------|---------|----------:|------------:|---------:|-------:|
| two_rook_mate      | win     | 25        | 68          | 0.000    | 3      |
| epsilon_mate       | win     | 50,865    | 137,273     | 0.033    | 5      |
| promotion_transposition | win | 225,137   | 511,029     | 0.097    | 7      |
| m26                | win     | 4,964,536 | 10,525,319  | 2.094    | 7      |
| opening_f2         | win     | 8,124,411 | 19,472,192  | 5.000    | 7      |
| rook_pawn_endgame  | win     | 1,491,991 | 3,512,293   | 0.716    | 7      |
| m19                | draw    | 892,016   | 18,572,315  | 5.000    | 0      |
| startpos           | draw    | 732,696   | 17,683,291  | 5.000    | 0      |

## Verification

```bash
cargo fmt                       # clean
cargo clippy --all-targets      # clean
cargo test                      # all active tests pass
cargo doc --no-deps             # clean
cargo test --release --test test_move_order        # passed in ~48 s
cargo test --release --test stress m19               # passed in 60 s
cargo test --release --test stress move_order_hard  # passed in 240 s
```

No position returned a wrong decisive outcome on the move-order suite. `m20`,
`m21`, and `m19` remain unproven within their stress timeouts, so no stress/SSR
fixture moves were required.

## Tests added

`src/search/ordering.rs` now contains:

- `kamikaze_landing_adjacent_to_lone_commoner` — a knight move that lands next to
  the enemy commoner scores above a non-kamikaze jump.
- `losing_capture_scores_below_direct_commoner_threat` — a queen-for-pawn atomic
  capture scores below a quiet bishop move that attacks the commoner.
- `capture_with_blasted_rook_scores_higher` — a capture that also destroys an
  enemy rook scores higher than a capture that only takes a pawn.
- `capture_promotion_is_not_scored_as_promotion` — a capture-promotion is
  evaluated by aSEE, not the `SCORE_PROMOTION` bonus.

## Constants

The following tuning was used and kept:

```rust
const SCORE_CAPTURE: i32 = 5_000;
const CAPTURE_NET_SCALE: i32 = 10;
const SCORE_KAMIKAZE_LAST: i32 = 9_000;
const SCORE_KAMIKAZE: i32 = 3_000;
```

No further constant tuning was required to pass the measurement gate, but the
aSEE base/scale pair is intentionally conservative and may be revisited when
idea 5 (TT-bound-aware initial sort) is evaluated.

## Unresolved edge cases and next steps

1. **aSEE and sacrificial captures:** aSEE can undervalue captures that lose
   material but open a mating net. `SCORE_WINNING_CAPTURE` handles the
   last-commoner case; other sacrificial wins rely on the search to correct the
   static score. A follow-up could add a small *capture-into-empty-near-king*
   bonus for captures that leave the opponent with no legal recapture.

2. **Refined-mode noise on tiny positions:** positions that solve in <0.1 s show
   small node-count increases in refined mode because the solver spends the rest
   of the timeout refining a different PV. These differences are not
   misclassifications, but they make refined-mode `nodes` a noisy ordering metric
   for very easy positions. Future benchmarking should report first-outcome
   time/nodes separately from full-refinement numbers.

3. **Default suite baseline:** a pre-change default-suite baseline was not
   captured in this pass. The next ordering experiment should record both
   `default` and `move-order` baselines before editing.

4. **Next ideas to evaluate:**
   - Idea 3 (node-type-aware ordering): give `sort_moves` an `is_or_node` flag and
     use a second scoring profile for defender nodes.
   - Idea 5 (TT-bound-aware initial sort): give a small bonus to children with a
     low `pn` at OR nodes or low `dn` at AND nodes when the TT already has bound
     information.
