# PV Suboptimality Analysis for `6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26`

## 1. Observed behavior

Running the CLI on the position produces:

```text
$ cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"
outcome: win
pv: b1b8 g8h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6
```

The PV is 11 half-moves. Two things are wrong with it:

1. There is a shorter forced win in 7 half-moves.
2. After `1.Rb8+` (`b1b8`), Black's `1...Kh7` (`g8h7`) is the weakest defense. It immediately allows `2.Rg8!` (`b8g8`) and a forced mate in 5 half-moves. The PV should show Black's best defense (`1...Kf7`), not the blunder `1...Kh7`, and White should punish `1...Kh7` with `2.Rg8` instead of the slow `2.Rh8`.

## 2. Verification

A depth-limited search with `max_depth = 7` finds the correct shortest forced win in a few hundred nodes:

```text
$ cargo run --release --example solve_depth_limited -- \
    "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26" 7
outcome: Win nodes: 668
b1b8
g8f7
b8f8
f7g7
d6e5
g7h7
f8h8
```

The final position has Black to move with no legal moves, so it is checkmate. This line is 7 half-moves long and uses Black's most resistant defense (`1...Kf7`).

The three possible Black replies to `1.Rb8+` and their fastest White refutations are:

| Black reply | Child FEN after `1.Rb8+` | Shortest White continuation | Plies from child |
|-------------|----------------------------|----------------------------|-------------------|
| `1...Kf7` (`g8f7`) | `1R6/3p1c2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7C w - - 2 27` | `b8f8 f7g7 d6e5 g7h7 f8h8` | 5 |
| `1...Kg7` (`g8g7`) | `1R6/3p2c1/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7C w - - 2 27` | `b8g8 g7f7 g8g6` | 3 |
| `1...Kh7` (`g8h7`) | `1R6/3p3c/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7C w - - 2 27` | `b8g8 c5c4 g8g6` | 3 |

For example, after `1...Kh7` the 3-plies child win is:

```text
$ cargo run --release --example solve_depth_limited -- \
    "1R6/3p3c/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7C w - - 2 27" 3
outcome: Win nodes: 38
b8g8
c5c4
g8g6
```

After `2.Rg8!` Black has only `2...c5c4`, and `3.Rxg6#` (`g8g6`) explodes the Black king because `g6` is adjacent to `h7`.

Black's **optimal** defense is `1...Kf7`, which holds out for 7 half-moves. `1...Kh7` and `1...Kg7` both lose in 5 half-moves. The solver's PV should therefore start with `b1b8 g8f7 ...`, not `b1b8 g8h7 ...`.

## 3. Root causes

### 3.1 The DF-PN loop exits as soon as any winning child is proven

In `src/search/dfpn/core.rs` the main loop does:

```rust
if let Some(solved) = selection.solved_outcome {
    outcome_to_store = Some(solved);
    ...
    if selection.all_solved {
        break;
    }

    if solved == Outcome::Win {
        break;
    }
    // Draw or Loss with unresolved siblings: keep refining.
}
```

When a node is an OR node (the player to move is trying to win) and `select_children` finds a child that is already a `Win`, the loop stops immediately and stores that child as the `best_move`. It does **not** continue searching the other children to see whether any of them is an even shorter win.

For the reported position this means:

- White's `1.Rb8+` is correctly identified as a winning first move.
- The search then explores the `1...Kh7` defense first (because `b8h8` scores much higher than `b8g8` in the static move ordering) and proves that White wins with the long `b8h8 ... g8g6` line.
- Because `1.Rb8+` is already a proven win, the loop stops. The search never returns to look for the shorter `1...Kf7` defense or the faster `b8g8` continuation after `1...Kh7`.

The same bug appears at every White OR node in the tree. After `1.Rb8+ Kh7`, White's `2.Rh8` is found first and is a win, so `2.Rg8` (which is a 3-plies win) is never discovered. The PV is therefore built from whatever line the move ordering happened to prove first, not the minimax shortest line.

### 3.2 Depth is not used to choose between partially solved children

`src/search/dfpn/selection.rs` already computes `parent_win_depth`/`parent_loss_depth`, but `select_children` only sees the children that happen to be solved at the moment the loop breaks. The transposition table stores the first proven `best_move` and its depth, not the depth-optimal one. Consequently:

- Black's best defensive reply (the one that delays the loss the longest) is not selected.
- White's fastest winning reply (the one with the smallest `1 + child_depth`) is not selected.

The 11-plies PV is a direct consequence of following these non-optimal `best_move` entries.

### 3.3 Static move ordering undervalues atomic restriction moves

`StaticAtomicScorer` (in `src/search/ordering.rs`) computes a threat bonus only when the moved piece attacks the enemy commoner's square after the move:

```rust
if (attack_bb & board.commoners(them)) != Bitboard::EMPTY {
    score += SCORE_THREAT;
}
```

In atomic chess a check is created by attacking a square adjacent to the enemy king, not necessarily the king's square itself. After `1.Rb8+ Kh7`:

- `b8h8` places the rook on `h8`, from where it attacks `h7` (the king). It gets `SCORE_THREAT_LAST` and a very high score (`10650`).
- `b8g8` places the rook on `g8`, from where it attacks `f8` and `h8`, both adjacent to the king on `h7`. It is in fact a much stronger move, but it only gets the generic `SCORE_BLAST`/`SCORE_CENTER` bonuses (`650`).

Because `b8h8` is ordered first, the solver proves the slower line first and exits.

### 3.4 `refine_shortest` binary search gets stuck

When `Search::solve` is called with `refine_shortest(true)`, it:

1. Runs an unbounded `dfpn` that returns the 11-plies PV and stores a depth of `11` in the transposition table.
2. Binary-searches between `1` and `11`.

The midpoint is `6`. Proving that the position is **not** a win in 6 half-moves requires an exhaustive depth-bounded search. In this position that takes about 5 seconds, which consumes the entire default timeout. The search never reaches `max_depth = 7`, where a short win is found almost instantly. With the 5-second timeout the binary search therefore fails and falls back to the 11-plies unbounded PV.

This can be seen with instrumentation of `solve_refined`:

```text
[refine] unbounded outcome=Win best_depth=11 nodes=26871
[refine] probe depth=6 outcome=Draw nodes=944802 lo=1 hi=11
[refine] validate lo=1 outcome=Draw ...
```

The depth-6 probe uses ~944k nodes and the timeout is exceeded before `max_depth = 7` is tried.

## 4. Proposed solution

### 4.1 Make the search depth-aware, not just the TT

The current code already stores a `depth` in `ChildInfo`, `ChildSelection`, and `TtEntry`, but the `dfpn` loop does not use it to keep looking for shorter wins. Change the loop in `core.rs` so that when `refine_shortest` is enabled and a `Win` is found at an OR node, the search keeps a running best move/depth and continues while there are still unsolved children that could improve the known shortest win. A practical rule is:

- Maintain `best_win_depth` and `best_win_move` for the current OR node.
- When `select_children` returns a solved `Win` child with depth `d`, if `d < best_win_depth` update the best move.
- Stop only when `best_win_depth` can no longer be improved (all children are solved, or the remaining unsolved children have a lower-bound depth greater than the current best).

For AND nodes the symmetric rule applies: keep the child with the **largest** loss depth, because that is the defense that delays the loss longest.

`is_solved_by_children` already has the right selection arithmetic (`min` for winning children, `max` for losing children); the fix is to allow it to run to completion rather than letting `core.rs` break on the first `Win`.

### 4.2 Replace binary refinement with iterative deepening

Instead of the current `solve_refined` strategy:

1. Keep the existing bootstrap that doubles `max_depth` (`1, 2, 4, 8, ...`). It already finds a winning depth quickly (in this case `8`).
2. Once a decisive result is found at depth `D`, try `D-1, D-2, ...` downward, **reusing** the transposition table and history/killer tables between probes.
3. The first failing depth establishes the lower bound; the last succeeding depth is the minimal winning depth.
4. If the lower-bound proof cannot be completed within the timeout, return the shortest winning PV found so far instead of falling back to an unbounded 11-plies PV.

This avoids the expensive `max_depth = 6` exhaustive draw probe being the very first refinement step. With a small amount of state reuse the `max_depth = 7` probe, which finds the win in a few hundred nodes, would run well before the timeout.

### 4.3 Improve `StaticAtomicScorer` for atomic checks

Add a bonus for moves that, after the move, attack a square adjacent to the opponent's last commoner, not just the commoner square. In the reported position this makes `b8g8` score comparable to `b8h8` and lets the solver discover the 3-plies `b8g8 ... g8g6` continuations.

The existing `SCORE_BLAST` and `SCORE_THREAT` bonuses in `src/search/ordering.rs` can be extended, or a new `SCORE_NEAR_COMMONER` can be introduced that is awarded when `attack_bb` intersects `attacks::king_attacks(enemy_king_sq)`.

### 4.4 Make `extract_pv` follow depth-optimal entries

Once the transposition table stores the minimax depth for each entry, `extract_pv_internal` should naturally follow the correct `best_move` because the stored `best_move` will be the depth-optimal one. As a safeguard, `extract_pv` can prefer a TT twin whose stored `depth` matches the expected minimax depth for the current path.

### 4.5 Regression tests

Add tests for this position and the key child positions. For example:

- `6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26` should return `Outcome::Win` and a PV of length `7` starting with `b1b8 g8f7`.
- The child `1R6/3p3c/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7C w - - 2 27` (after `1.Rb8+ Kh7`) should be solved in 3 plies with `b8g8 c5c4 g8g6`.

## 5. Relation to existing work

This is the same class of bug discussed in `docs/plans/dfpn/report_pv_issue.md` and targeted by `docs/plans/dfpn/plan6.md`. Plan 6 added history/killer heuristics and an iterative-deepening bootstrap, which are visible in `src/search/dfpn/history.rs` and `src/search/dfpn/mod.rs`. Those changes improve move ordering and help the bootstrap find a decisive result quickly, but they do not fix the underlying problem: the `dfpn` loop still stops at the first proven winning child, and `solve_refined` still binary-searches from an unbounded upper bound. The additional changes described in this report are needed to close that gap.
