# Plan: Move Ordering for DF-PN+ Solver

## Summary

`plans/dfpn/plan5.md` implemented the full Kishimoto & Müller GHI fix. The white-to-move child `6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28` now finds the shortest win (`g8g7 e6f5 g7g6`) immediately, but the black-to-move root

```text
6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27
```

still returns `Draw` after 60 seconds. The position is a forced loss for Black (`f7e6` leads to a short White win), but the solver expands the `f7e6` child in an order that puts non-winning moves (`d6c5`, `g8g6`, `g8e8`, ...) ahead of the quiet winning moves (`g8g7`/`g8f8`). The `f7e6` child is an AND node (`is_or_node = false`) with `vdn` ties for all unsolved children, so `best_and_second_unsolved` falls back to the static move order.

This plan adds dynamic move ordering: history and killer heuristics, transposition-table best-move ordering for unsolved nodes, and a shallow iterative-deepening bootstrap so that the winning `best_move` is learned before the full, unbounded search. The goal is to make `tests/test_plan5.rs::black_root_report4_fen` pass within 60 seconds and improve overall solver speed on zugzwang-like positions.

## Goal and Scope

### Goal

1. Make `test_plan5::black_root_report4_fen` prove `Outcome::Loss` for the black-to-move FEN within 60 seconds.
2. Improve the general ordering of quiet, positional waiting moves (zugzwang) without weakening the existing ordering on tactical positions.
3. Keep all existing tests passing.

### Non-goals

- Parallel search.
- GHI changes (the base/twin + simulation design from plan5 is kept).
- Epsilon-threshold changes.
- Piece-rule or move-generation changes.

## Affected test positions

Primary regression:

```text
6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27
```

Expected after the fix:

```text
outcome: loss
pv: f7e6 g8g7 e6f5 g7g6
```

(White may also play `g8g7 c5c4 g7d7` or similar; any 3-ply `g8g7`/`g8f8` win is acceptable.)

Additional manual verification positions from `plans/dfpn/prompt.md`:

```text
4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19
4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20
4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/1R4RK b - - 5 20
4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21
4r2k/3p4/2pB2p1/p4p1p/6PP/2N1PP2/P1PP4/1R4RK b - - 0 21
4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22
4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK b - - 0 22
4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23
4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K b - - 2 23
4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24
4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K b - - 0 24
4r1k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R2R2K w - - 0 25
6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25
6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26
1R4k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 1 26
1R6/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 2 27
6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27
6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28
5R2/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 5 28
5R2/3p4/3Bk1p1/6Pp/2p4P/p1N2P2/P1PP4/7K w - - 0 29
8/3p4/3BkRp1/6Pp/2p4P/p1N2P2/P1PP4/7K b - - 1 29
```

## Root cause

1. **`StaticAtomicScorer` does not value zugzwang/waiting moves.**
   - `g8g7` and `g8f8` are quiet rook moves that restrict the Black king's escape squares (especially `e7`). They are not captures, checks, or direct attacks on the king, so they receive only the default `SCORE_CENTER`/`SCORE_APPROACH` bonus.
   - `d6c5` (capture of a pawn), `g8g6` (capture of a pawn), and `g8e8` (check) all score much higher even though they are not the winning moves.

2. **`best_and_second_unsolved` has no dynamic tie-breaker for `is_or_node = false`.**
   - For the `f7e6` child (AND node, white to move), every unsolved child starts with `pn = 1, dn = 1`.
   - `best_and_second_unsolved` picks the child with the smallest `vdn` (which is `dn`). With `dn` ties, it falls through to the static move order produced by `sort_moves`.

3. **`best_from_tt` only uses solved transposition-table moves.**
   - `dfpn` currently checks `best_result_for_path(path_code)`, which returns `None` for unsolved entries.
   - The TT does store a `best_move` for unsolved nodes (from previous searches or iterations), but it is ignored for ordering.

4. **No history or killer heuristic.**
   - Once a good move like `g8g7` is found, the solver has no way to remember it and try it earlier in sibling branches or in a subsequent search.

## Proposed fix

### 1. Use the stored `best_move` for unsolved transposition-table entries

Extend the `best_from_tt` lookup in `dfpn` so that it also returns `entry.best_move` when the entry is unsolved but has a non-`NONE` `best_move` and a positive `depth`. This is safe because the move is only used for ordering; the GHI-safe result lookup in `try_use_tt` is unchanged.

```rust
let best_from_tt = self
    .tt
    .probe(key)
    .and_then(|e| {
        e.best_result_for_path(self.path_code)
            .map(|(mv, ..)| mv)
            .or_else(|| {
                if e.best_move != Move::NONE && e.depth > 0 {
                    Some(e.best_move)
                } else {
                    None
                }
            })
    })
    .unwrap_or(Move::NONE);
```

### 2. Add a history heuristic

Add a `history` table to `Search`:

```rust
const HISTORY_MAX: i32 = 10_000;
const HISTORY_BONUS: i32 = 100;

history: [[[i32; 64]; 64]; 2],
```

Indexed by `[side as usize][from as usize][to as usize]`. In `sort_moves` add `history[side][from][to]` to the static score.

When `dfpn` stores a solved result (`outcome_to_store` is `Win` or `Loss`) with a non-`NONE` `best_move`, increment the history entry for the side that played it:

```rust
if let Some(outcome) = outcome_to_store
    && best_move != Move::NONE
    && (outcome == Outcome::Win || outcome == Outcome::Loss)
{
    let us = pos.side_to_move();
    let from = best_move.from_sq() as usize;
    let to = best_move.to_sq() as usize;
    let entry = &mut self.history[us as usize][from][to];
    *entry = (*entry + HISTORY_BONUS).min(HISTORY_MAX);
}
```

To avoid stale history dominating the score, age all entries periodically (e.g., after every `HISTORY_AGE_INTERVAL = 10_000` nodes, divide every entry by 2 with saturating arithmetic). This is a standard technique and keeps the table small.

### 3. Add killer moves

Add a killer move table to `Search`:

```rust
const KILLER_SLOTS: usize = 2;
const MAX_KILLER_DEPTH: usize = 256;

killers: [[Move; KILLER_SLOTS]; MAX_KILLER_DEPTH],
```

In `sort_moves`, give a large bonus to a move that matches one of the killer slots for the current `path_stack.len()`:

```rust
const SCORE_KILLER: i32 = 50_000;
```

`SCORE_KILLER` should be below `SCORE_WINNING_CAPTURE` and `SCORE_PROMOTION` but above `SCORE_CAPTURE` and `SCORE_THREAT_LAST`, so tactical moves still come first but quiet winning positional moves jump ahead of random moves.

When a node is solved with `best_move != Move::NONE`, update the killer table at the current depth:

```rust
if best_move != Move::NONE {
    let depth = self.path_stack.len();
    if depth < MAX_KILLER_DEPTH {
        let slot = &mut self.killers[depth];
        if best_move != slot[0] {
            slot[1] = slot[0];
            slot[0] = best_move;
        }
    }
}
```

### 4. Update `sort_moves` to use the combined score

```rust
fn sort_moves(&self, pos: &Position, moves: &mut MoveList, best_from_tt: Move) {
    let mut state = atomic_movegen::board::StateInfo::new();
    pos.board.populate_state(&mut state);

    let us = pos.side_to_move() as usize;
    let depth = self.path_stack.len();

    let slice = moves.as_mut_slice();
    if best_from_tt != Move::NONE
        && let Some(idx) = slice.iter().position(|&m| m == best_from_tt)
    {
        slice.swap(0, idx);
    }

    slice.sort_by(|&a, &b| {
        let sa = self.scorer.score(&pos.board, a, &state)
            + self.history[us][a.from_sq() as usize][a.to_sq() as usize]
            + self.killer_bonus(a, depth);
        let sb = self.scorer.score(&pos.board, b, &state)
            + self.history[us][b.from_sq() as usize][b.to_sq() as usize]
            + self.killer_bonus(b, depth);
        sb.cmp(&sa)
    });
}

fn killer_bonus(&self, m: Move, depth: usize) -> i32 {
    if depth >= MAX_KILLER_DEPTH {
        return 0;
    }
    if self.killers[depth].iter().any(|&k| k == m) {
        SCORE_KILLER
    } else {
        0
    }
}
```

### 5. Iterative deepening bootstrap to prime `best_move`

If history and killers alone are not enough, add an `initial_depth` bootstrap phase to `solve` before the unbounded `dfpn` or `solve_refined`:

1. Run `dfpn` with `max_depth = 1`.
2. If the result is `Draw`, clear the transposition table and double `max_depth` (`2, 4, 8, ...`).
3. Stop as soon as the result is `Win` or `Loss`.
4. Once an initial decisive result is found, run `solve_refined` as before to find the shortest PV.

Because each `dfpn` call is depth-bounded, the `max_depth = 4` search will solve the `f7e6` child (`g8g7` is a 3-ply win) and store `g8g7` as the `best_move` in the TT. The subsequent unbounded search will then use `best_from_tt` to order `g8g7` first and prove the loss for Black quickly.

This bootstrap should be optional/configurable and only used when `refine_shortest` is enabled, to avoid overhead on trivial positions.

## Data structure changes

### `src/search/dfpn.rs`

Add to `Search`:

```rust
const HISTORY_MAX: i32 = 10_000;
const HISTORY_BONUS: i32 = 100;
const HISTORY_AGE_INTERVAL: u64 = 10_000;
const SCORE_KILLER: i32 = 50_000;
const KILLER_SLOTS: usize = 2;
const MAX_KILLER_DEPTH: usize = 256;

history: [[[i32; 64]; 64]; 2],
killers: [[Move; KILLER_SLOTS]; MAX_KILLER_DEPTH],
history_age_counter: u64,
```

Initialize in `Search::new`:

```rust
history: [[[0; 64]; 64]; 2],
killers: [[Move::NONE; KILLER_SLOTS]; MAX_KILLER_DEPTH],
history_age_counter: 0,
```

Add methods:

```rust
fn update_history(&mut self, m: Move, side: Color);
fn update_killers(&mut self, m: Move);
fn maybe_age_history(&mut self);
fn killer_bonus(&self, m: Move, depth: usize) -> i32;
```

### `src/search/tt.rs`

No structure changes. The existing `best_move` field is already available for unsolved entries. Optionally add a helper `best_move_for_path(path_code)` that returns the solved or unsolved best move for the current path, or reuse `best_result_for_path` with a fallback as shown above.

## Algorithm changes

### `dfpn` lookup

Change `best_from_tt` to also use unsolved `best_move` entries:

```rust
let best_from_tt = self
    .tt
    .probe(key)
    .and_then(|e| {
        e.best_result_for_path(self.path_code)
            .map(|(mv, ..)| mv)
            .or_else(|| {
                if e.best_move != Move::NONE && e.depth > 0 {
                    Some(e.best_move)
                } else {
                    None
                }
            })
    })
    .unwrap_or(Move::NONE);
```

### `sort_moves`

Replace the static-only scoring with the combined static + history + killer score.

### `dfpn` result storage

After the `tt.store` call at the end of `dfpn`, update history and killers if the node was solved with a non-`NONE` best move:

```rust
self.tt.store(...);

if let Some(outcome) = outcome_to_store
    && outcome != Outcome::Draw
    && best_move != Move::NONE
{
    self.update_history(best_move, pos.side_to_move());
    self.update_killers(best_move);
}

self.maybe_age_history();
```

### `solve` iterative deepening bootstrap

If `refine_shortest` is enabled, change the flow:

```rust
fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
    self.nodes = 0;
    self.start = Instant::now();
    self.deadline = self.start + self.timeout;
    self.path.clear();
    self.path_stack.clear();
    self.path_code = 0;
    self.last_pv.clear();

    if self.refine_shortest {
        // Bootstrap: find any decisive result with a small depth budget,
        // doubling the budget until the position is solved.
        let mut max_depth = 1u32;
        while max_depth <= 64 {
            self.tt.clear();
            let outcome = self.dfpn(pos, INF, INF, max_depth, true);
            if outcome != Outcome::Draw || self.time_exceeded() {
                break;
            }
            max_depth = max_depth.saturating_mul(2);
        }

        // Now refine to the shortest win/loss.
        self.solve_refined(pos)
    } else {
        let outcome = self.dfpn(pos, INF, INF, u32::MAX, true);
        let pv = self.extract_pv(pos);
        (outcome, pv, self.nodes)
    }
}
```

The exact `max_depth` cap (`64`) and doubling schedule can be tuned. The bootstrap is only needed for positions where the unbounded search would time out.

## File changes

### `src/search/dfpn.rs`

- Add `history`, `killers`, and `history_age_counter` to `Search`.
- Add `update_history`, `update_killers`, `maybe_age_history`, `killer_bonus`, `time_exceeded` helpers.
- Update `sort_moves` to combine static, history, and killer scores.
- Update `best_from_tt` lookup to use unsolved `best_move`.
- Update `dfpn` to call `update_history`/`update_killers` after storing a solved result.
- Update `solve` to optionally run an iterative-deepening bootstrap before `solve_refined`.

### `src/search/ordering.rs`

No changes unless `SCORE_KILLER` is placed here for consistency. Keep `StaticAtomicScorer` unchanged so it continues to provide a stable static baseline.

### `src/search/tt.rs`

No changes required, but add `TtEntry::best_move_for_path(path_code)` if the fallback logic is to be shared.

### `tests/test_plan5.rs`

- Remove `#[ignore]` from `black_root_report4_fen`.
- Optionally keep `set_timeout(60)` to make the 60-second budget explicit.

### `tests/test_plan6.rs` (new)

Add a new integration test that runs the positions in `plans/dfpn/prompt.md` and asserts the expected outcome within the timeout. For the black-to-move regression, assert `Outcome::Loss` and a PV that starts with `f7e6`.

## Testing and verification

### Existing tests

Run the full suite:

```bash
cargo fmt
cargo clippy --all-targets
cargo test
cargo doc
```

Ensure the following still pass:

- `tests/test_inf.rs`
- `tests/test_plan2.rs`
- `tests/test_plan3.rs`
- `tests/test_plan4.rs`
- `tests/test_plan5.rs`
- `src/search/dfpn.rs` unit tests

### Re-enable regression test

After removing `#[ignore]` from `test_plan5::black_root_report4_fen`:

```bash
cargo test --release --test test_plan5
```

Expected:

```text
test black_root_report4_fen ... ok
test white_child_f7e6_short_win ... ok
```

### Manual verification

```bash
cargo run --release -- --fen "6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27"
```

Expected:

```text
outcome: loss
pv: f7e6 g8g7 e6f5 g7g6
```

Also verify the prompt.md benchmark positions finish within 60 seconds and return sensible outcomes.

## Risks and mitigations

- **History table size**: `[[[i32; 64]; 64]; 2]` is 32 KiB (2 * 64 * 64 * 4 bytes), negligible. A fixed-size table with saturating arithmetic and periodic aging avoids unbounded growth.
- **Killer table size**: `[[Move; 2]; 256]` is 256 * 2 * 4 bytes = 2 KiB, also negligible.
- **Correctness**: History and killer bonuses are only used for move ordering. They cannot change the game-theoretic result; they only affect the order in which `dfpn` explores children. If a bonus is misleading, `dfpn` will still explore siblings and find the correct result.
- **GHI safety**: Using unsolved `best_move` for ordering is safe because the result is still verified by `try_use_tt` and the base/twin mechanism. A stale `best_move` from a different path may be tried first, but if it fails the solver falls back to other moves.
- **Iterative deepening overhead**: The bootstrap adds a few extra shallow searches. For positions that already solve with the unbounded search, the overhead is one extra `dfpn` with a small `max_depth`. The `solve` function can be changed to skip the bootstrap if `refine_shortest` is false or if the position is solved trivially.
- **History aging frequency**: Aging every 10,000 nodes may be too often or too rare. Measure and tune.

## Summary

1. Use unsolved `best_move` from the transposition table for ordering.
2. Add a history heuristic with saturating increments and periodic aging.
3. Add a killer-move table with two slots per ply.
4. Update `sort_moves` to combine static, history, and killer scores.
5. Update `dfpn` to record history/killer bonuses when a node is solved.
6. Optionally add a shallow iterative-deepening bootstrap before `solve_refined` to prime `best_move` for hard zugzwang positions.
7. Re-enable `test_plan5::black_root_report4_fen` and verify all existing tests still pass.
