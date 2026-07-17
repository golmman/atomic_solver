# Correctness Review: `atomic_solver` DF-PN+ Implementation

Date: 2026-07-17  
Scope: `src/{position,zobrist,search/{dfpn,tt,ordering},main}.rs`, with reference to `docs/plans/dfpn/research_epsilon.md`, `research_ghi.md`, and `research_parallel.md`.

## Executive summary

The sequential DF-PN+ solver in `src/search/dfpn.rs` is a faithful implementation of the techniques described in the research notes: it uses the `1 + ε` threshold trick (`research_epsilon.md`), base/twin transposition-table entries and Kawano-style simulation for the Graph-History Interaction problem (`research_ghi.md`), and the standard OR/AND `pn`/`dn` aggregation from the DF-PN literature (`research_parallel.md`). `cargo test` and `cargo clippy --all-targets` both pass.

However, there are two correctness gaps that can produce wrong results:

1. **Terminal detection treats every no-legal-moves position as a draw**, including single-commoner checkmates, which are wins/losses in the check-enforcing atomic-chess rules used by `atomic_movegen`.
2. **The GHI/twin reuse path is not fully sound**: the path-code encoding does not distinguish move type, and the simulation search does not carry the current search prefix, so it may accept a twin whose proof is valid only under a different path.

Several smaller issues (weak PV validation, linear instead of binary shortest-PV search, limited twin capacity) are also noted below.

---

## 1. What is implemented correctly

### 1.1 `1 + ε` threshold trick

`src/search/dfpn.rs` lines 379-401 compute the child thresholds exactly as `research_epsilon.md` describes:

- OR node: `new_th_pn = min(th_pn, ε·ceil(second_child_pn))`, `new_th_dn = th_dn - current_dn + child_dn`.
- AND node: symmetric with `dn`/`pn` swapped.

`epsilon_ceil` (lines 629-635) correctly guards `INF` and rounds up. The default `EPSILON = 0.25` matches the paper's recommended starting value.

### 1.2 OR/AND `pn`/`dn` aggregation and solved-result selection

`select_children` (lines 637-714) and `is_solved_by_children` (lines 801-872) implement the correct minimax selection:

- Win for the side to move if any child is a loss for the next player, picking the shortest such line.
- Draw if all children are solved and at least one is a draw, picking the longest draw.
- Loss if all children are wins for the next player, picking the longest line.

The `pn`/`dn` formulas are the standard ones (OR: `min`/`sum`; AND: `sum`/`min`) and agree with `research_parallel.md` section 2.1.

### 1.3 GHI data structures

`src/search/tt.rs` implements the base/twin split recommended in `research_ghi.md` section 3.1:

- `TtEntry` has a base `outcome`/`pn`/`dn` plus up to two `TwinEntry` records.
- Path-dependent solved results are stored as twins and the base is re-initialized to `(1, 1)` (`reinit_base_for_twin`, lines 123-130), matching `research_ghi.md` section 4.1.
- Path codes are maintained as a Zobrist XOR of per-move keys (`src/zobrist.rs` lines 58-75), as in `research_ghi.md` section 3.2.
- The position hash includes the halfmove clock (`zobrist::hash`), which is the cheap path-dependence fix recommended in `research_ghi.md` section 7.

### 1.4 Move ordering and heuristics

`StaticAtomicScorer` ( `src/search/ordering.rs` ) uses domain-appropriate features (winning captures, promotions, threats, blast threats, centralization) and the `Search` struct adds history and killer tables. These are ordering-only and do not affect correctness.

### 1.5 CLI

`src/main.rs` correctly parses `--fen`, runs the solver with shortest-PV refinement enabled, and prints the outcome plus a UCI PV.

---

## 2. Significant correctness issues

### 2.1 No-legal-move terminal detection misclassifies checkmate as draw

`Position::outcome` (`src/position.rs` lines 89-105) only handles:

- `rule50 >= 100` → Draw
- side to move has no commoners → Loss
- opponent has no commoners → Win
- only two pieces on the board → Draw

It does **not** consider the case where the side to move has no legal moves.

In `dfpn` (`src/search/dfpn.rs` lines 259-272) an empty move list is unconditionally treated as `Outcome::Draw`:

```rust
if moves.is_empty() {
    self.path.remove(&key);
    let (pn, dn) = Outcome::Draw.pn_dn_for(is_or_node);
    self.tt.store(
        key,
        Move::NONE,
        Some(Outcome::Draw),
        pn,
        dn,
        0,
        self.path_code,
        false,
    );
    return Outcome::Draw;
}
```

This is inherited from `atomic_movegen::Board::outcome`, which also treats all no-legal-move positions as stalemate draws. But `atomic_movegen` *does* enforce check: a lone commoner may not move into attack, and `generate_legal` can return an empty move list because the last commoner is in check with no escape. In the check-enforcing atomic-chess rules used by the move generator, that is a checkmate and should be `Outcome::Loss` for the side to move, not `Outcome::Draw`.

**Reproducible example:**

```text
7K/8/8/8/8/8/1Q6/k7 b - - 0 1
```

White: Kh8, Qb2. Black: Ka1. Black to move. The black commoner on a1 is attacked by the white queen on b2; every black move either leaves the commoner attacked or self-explodes. `generate_legal` returns no moves. The solver reports `Outcome::Draw`; it should be `Outcome::Loss` (white wins by checkmate).

**Fix:** Before returning `Outcome::Draw` for an empty move list, check whether the side to move has at least one commoner and at least one of those commoners is under attack. `Board::populate_state` + `StateInfo::checkers` already provides this information inside `generate_legal`; expose it through `Position` or use it directly in `dfpn`.

### 2.2 GHI twin simulation does not verify the current path

`try_use_tt` (`src/search/dfpn.rs` lines 460-526) follows the research structure: first try a path-independent base entry, then an exact twin for the current path code, then run Kawano simulation for twins from other paths.

The simulation is implemented in `dfpn.rs` lines 528-627. It is called with `twin.path_code`:

```rust
if self.simulate(
    &mut sim_pos,
    twin.path_code,
    outcome,
    twin.best_move,
    &mut sim_path,
    &mut sim_stack,
    &mut sim_nodes,
)
```

`simulate` then recomputes child path codes from the **twin's** path code and probes the TT. The local `sim_path` set is initialized empty and only contains positions from `N` downward. It does **not** include the current search prefix (the contents of `self.path`), and it does not check the main search's `path` HashSet.

Consequences:

- A twin whose proof relied on a repetition with an ancestor in the *twin's* path may validate successfully even though that repetition does not occur along the *current* path.
- A simulation move that reaches a position already on the current search path is a repetition in the real tree, but `sim_path` will not detect it.

`research_ghi.md` section 3.3 says simulation "verifies a twin from another path for the current path." The current code verifies the twin against the twin's own path, not the current path.

**Fix:** Pass the current path prefix into `simulate` (as an initial `sim_path` set or as the starting `path_code` for a bounded fresh search), and run the verification under the current path code. If the twin cannot be reproduced under the current path, it must not be reused.

### 2.3 Path-code encoding does not include move type

`zobrist::path_random` (`src/zobrist.rs` lines 58-65) builds a move index from `from + to*64 + promotion*64*64`. It reads `promotion` from `mv.promotion_type()`.

For a **normal** move, `Move::make_move` leaves the promotion bits as `0`, and `promotion_type()` interprets those `0` bits as the first element of `PROMOTION_PIECES`, which is `PieceType::Queen`. Therefore a normal move and a queen-promotion move with the same `from` and `to` squares get the **same** path key.

The path key also does not encode `MoveType::EnPassant` or `MoveType::Castling` at all. This means two different paths to the same board position can produce identical path codes when they differ only by move type. That can cause `find_result_for_path` to match a twin that was stored for a different kind of move sequence, potentially reusing a path-dependent result incorrectly.

**Fix:** Include the move type in the path key, and use a distinct sentinel (e.g., `0`) for non-promotion moves so that normal moves do not share keys with queen promotions.

---

## 3. Other correctness and robustness issues

### 3.1 `simulate` accepts an empty move list as valid for `Outcome::Loss`

In `simulate`, the `Outcome::Loss` branch (lines 591-620) expands all legal children:

```rust
Outcome::Loss => {
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    let mut ok = true;
    for i in 0..moves.len() { ... }
    ok
}
```

If `moves` is empty, the loop body never runs and `ok` remains `true`. For a node that is supposed to be a `Loss` for the side to move, an empty move list should be either a draw (stalemate) or a terminal loss (no commoners), but `simulate` does not re-check `pos.outcome()` in this branch. It does check `pos.outcome()` at the top, but only when called; a position with `outcome() == None` and no legal moves would pass through as a `Loss`. This is a defensive correctness gap.

**Fix:** After generating legal moves, if `moves.is_empty()` return `pos.outcome().is_some_and(|o| o == expected)` or `false` for `Outcome::Loss` (unless the position is genuinely terminal with `Outcome::Loss`).

### 3.2 `solve_refined` is linear, not binary

The comment at `solve_refined` (`src/search/dfpn.rs` lines 134-175) says it does a "binary search the smallest depth bound", but the code is a linear `for mid in 1..best_depth` loop. The final result is still correct, but the comment and the implementation disagree, and the loop can be expensive for deep wins.

### 3.3 PV validation is weak

`validate_pv` (`src/search/dfpn.rs` lines 192-198) replays the PV and checks only that the final position is terminal:

```rust
fn validate_pv(pv: &[Move], pos: &Position) -> bool {
    let mut current = pos.clone();
    for &m in pv {
        current.do_move(m);
    }
    current.outcome().is_some()
}
```

It does not verify that each move is legal, that the outcome matches the reported result, or that the PV length is consistent with the reported depth. An illegal move would be silently played by `Board::do_move` and could lead to a spurious terminal.

### 3.4 `outcome_from_pn_dn` misclassifies draws

`outcome_from_pn_dn` (`src/search/dfpn.rs` lines 1106-1114) maps `(INF, 0)` to `Outcome::Loss`:

```rust
} else if pn == INF && dn == 0 {
    Some(Outcome::Loss)
```

`Outcome::Draw` also maps to `(INF, 0)` in `to_pn_dn`. The function is not used in the main search loop, but it is exported and would misclassify a draw as a loss. Any future caller should be aware of this mapping.

### 3.5 `Position::outcome` ordering and assumptions

- `Position::outcome` checks `rule50 >= 100` before `commoners(us).is_empty()`. A position with `rule50 >= 100` and no own commoners is reported as a draw, even though the side to move has already lost its last commoner. Such positions cannot arise legally, but the ordering is not defensive.
- The `occupied().count() == 2` draw heuristic is safe for legal positions: if it is reached, both remaining pieces must be commoners (a side without a commoner is caught earlier). It only misbehaves on malformed FENs that reach it while one side lacks a commoner, but those are ruled out by the earlier checks.

### 3.6 Twin capacity may be too small

`MAX_TWINS = 2` (`src/search/tt.rs` line 7) and the replacement policy overwrites the oldest twin. In a repetition-heavy search graph a single position can be reached by many path codes. Evicting a twin does not cause an incorrect result — the base is `(1, 1)` and the node is re-searched — but it can cause repeated re-solving and, in pathological cases, interact with the simulation bug above to evict the only twin that would have correctly matched a path.

---

## 4. Research alignment notes

- `research_epsilon.md` is followed well; the only missing convenience is a `set_epsilon` method/runtime flag.
- `research_ghi.md` base/twin structure and base re-initialization are followed. The two main deviations are the path-code encoding (section 3.2) and the simulation not carrying the current path (section 3.3). The recommendation to initialize root thresholds to `(1, 1)` is **not** followed, but the solver does not store thresholds in the TT (only `pn`/`dn` bounds and outcomes), so this does not appear to be a correctness problem.
- `research_parallel.md` describes a parallel/multi-agent design; this implementation is purely sequential and does not use `T(n,c)` or `Mark`/`Unmark`. That is fine for a single-threaded solver and does not affect correctness.

---

## 5. Test status

- `cargo test` passes all 49 non-ignored tests.
- `cargo clippy --all-targets` is clean.
- The existing tests cover simple mates, rule-50 draws, king extinction, and the plan6 regression suite. They do not cover:
  - single-commoner checkmate vs stalemate,
  - transposition/repetition reuse through twins,
  - promotion vs normal-move path-code collisions,
  - `simulate` behavior across incompatible paths.

A temporary test with the FEN `7K/8/8/8/8/8/1Q6/k7 b - - 0 1` demonstrated the no-legal-move checkmate issue (solver returned `Draw`, expected `Loss`).

---

## 6. Recommendations

1. **Fix terminal detection for no-legal-moves.** When `generate_legal` returns an empty list, distinguish checkmate (last commoner under attack) from stalemate and return `Outcome::Loss` in the checkmate case.
2. **Fix the path-code encoding.** Include `move_type` and use a non-promotion sentinel so that normal moves, promotions, en-passant, and castling do not share keys.
3. **Harden GHI simulation.** Carry the current search prefix into `simulate` and verify the twin under the current path code; treat empty move lists and repetitions with the current prefix correctly.
4. **Increase twin capacity** or switch to a dynamic list/Vec for `TwinEntry` in heavily cyclic positions.
5. **Clarify or fix `solve_refined`:** either implement true binary search or update the comment to reflect the linear scan.
6. **Strengthen `validate_pv`** to check move legality and that the final outcome matches the reported result.
7. **Add regression tests** for:
   - checkmate by line piece with lone king (`7K/8/8/8/8/8/1Q6/k7 b - - 0 1` and similar),
   - stalemate with no commoner under attack,
   - transpositions with different move types/promotions,
   - positions with repeated states that stress the twin mechanism.
8. **Expose `epsilon` at runtime** as suggested by `research_epsilon.md` if tuning is intended.

---

## 7. Conclusion

The implementation is a solid sequential DF-PN+ solver that matches the research notes for the `1 + ε` trick, OR/AND `pn`/`dn` propagation, and the base/twin transposition-table design. The main risks are in **terminal detection** (misclassifying checkmate as draw) and **GHI twin reuse** (simulation not using the current path and path-code collisions from missing move-type encoding). Fixing these two areas, especially the terminal detection, is the highest priority for making the solver fully correct on atomic-chess endgames.
