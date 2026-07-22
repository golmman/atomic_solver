# Checkpoint 1: fen1/fen2 timing discrepancy

## Context

Two positions differing by one halfmove (`c6c5`):

```text
fen1 = "6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25"
fen2 = "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"
```

`fen1` is Black to move; `fen2` is White to move after `c6c5`.

## Observed behavior

```bash
cargo run --release -- --fen "$fen2" --no-refine-shortest --timeout 60
# outcome: win, instant (7-ply mate)

cargo run --release -- --fen "$fen1" --no-refine-shortest --timeout 60
# timeout after 60 s
```

The same timeout occurs without `--no-refine-shortest`.

## Measurements

Temporary `examples/debug_depth.rs` and `examples/debug_solve.rs` were used to gather the following data (both removed afterwards).

| FEN | call | max_depth | outcome | child_evals | time |
|---|---|---|---|---|---|
| fen2 | `search_depth` | 8 | Win | 2,626 | 0.002 s |
| fen1 | `search_depth` | 8 | Draw | 273,689,724 | 60.0 s |
| fen1 | `search_depth` | 12 | Loss | 10,881 | 0.003 s |
| fen1 | `search_depth` | 16 | Loss | 10,881 | 0.003 s |
| fen1 | `solve()` unbounded | ∞ | Loss | ~1,129 dfpn nodes | fast |

The fen1 PV found by `solve()` is a 12-ply forced loss for Black:

```text
g8g7 b1b8 g7h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6
```

## Root cause

`main.rs` always calls `Search::solve_outcome()`:

- `src/main.rs` lines 148-152
- `src/search/dfpn/mod.rs` lines 156-204

`solve_outcome()` bootstraps with an iterative deepening schedule that doubles `max_depth`:

```text
1, 2, 4, 8, 16, 32, 64, then unbounded
```

For fen2, the forced mate is 7 plies, so `max_depth = 8` succeeds immediately. For fen1, the forced win for White is 12 plies, so `max_depth = 8` is **just below the horizon**. At `max_depth = 8` every leaf is a depth-cutoff `Draw` and DF-PN cannot prove any win/loss; it therefore expands the entire 8-ply frontier (273 M child evaluations) and consumes the full timeout, never reaching `max_depth = 16` where it would finish in milliseconds.

The leaf cutoff is at:

- `src/search/dfpn/core.rs` lines 61-76

## Why unbounded search is fast

With `max_depth = u32::MAX`, `dfpn` reaches the terminal capture `g8g6` and back-propagates a proven `Loss` for Black. DF-PN then prunes siblings using the proof/disproof numbers, and the TT stores solved entries with `remaining_depth = u32::MAX`. The whole search collapses to about 10k child evaluations. The depth-bounded version cannot do any of this because all leaves at `max_depth = 0` are treated as `Draw`, producing `(INF, 0)` bounds that prevent pruning.

## Relevant research and plans

The `docs/plans/dfpn/research*.md` files (epsilon, GHI, parallel) do not directly address this issue. The relevant guidance is in `plan6.md` and `report6.md`, which introduced the iterative-deepening bootstrap and state:

- The bootstrap should be optional/configurable and only used when `refine_shortest` is enabled.
  - `docs/plans/dfpn/plan6.md` line 229
- `solve` should skip the bootstrap when `refine_shortest` is false.
  - `docs/plans/dfpn/plan6.md` lines 440-443
- The `max_depth` cap and doubling schedule should be tuned.
  - `docs/plans/dfpn/plan6.md` line 354
- If the deadline expires during the bootstrap, the best result and PV found so far should be preserved.
  - `docs/plans/dfpn/report6.md` line 58
- Future improvement: keep the transposition table and use widening bounds instead of full clears.
  - `docs/plans/dfpn/report6.md` line 147

## Open questions / next steps

1. Should `main.rs` call `Search::solve()` (unbounded) instead of `solve_outcome()` when `--no-refine-shortest` is given?
2. Should `solve_outcome()` skip bootstrap entirely, start with a larger initial `max_depth`, or use a finer deepening schedule (e.g. `1, 2, 4, 8, 12, 16, 20, ...`)?
3. Should `solve_outcome()` preserve a decisive bootstrap result and its PV if time expires before reaching a larger bound?
4. Should the transposition table be retained across bootstrap iterations instead of being logically cleared?
5. Does the `max_depth == 0` leaf treatment need to change so that bounded searches can propagate useful bounds even when they cannot reach a terminal?
