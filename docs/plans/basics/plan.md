# Implementation Plan: First Atomic-Chess Solver

**Goal:** Build a pure atomic-chess solver from scratch, leveraging the existing `atomic-movegen` crate for move generation and position representation. The solver determines the exact game-theoretic outcome of a position (win / loss / draw) and, when the outcome is a forced win or loss, produces a principal variation.

**Scope of this first set:** `atomic-movegen` integration, depth-first proof-number search (DF-PN) with iterative deepening, FEN input with a default starting position, and a simple CLI output.

**Assumptions made for this plan (because they were not specified explicitly):**
- The solver is built as a Rust binary crate in this repository.
- `atomic-movegen` is used as a crates.io dependency (version `1.0.0` is the current release).
- The solver runs until it resolves the position to an exact `win`, `loss`, or `draw`.
- Principal variations are printed in UCI long-algebraic notation (`e2e4`, `e7e8q`, etc.).
- "Depth" is the depth parameter of the completed iterative-deepening iteration, and a new progress line is printed after each completed iteration.

---

## 1. Repository and crate setup

1. Add `atomic-movegen = "1.0.0"` to `Cargo.toml`.
2. Add any other small dependencies we decide we need (e.g. `clap` for CLI or none at all — keep it minimal).
3. Call `atomic_movegen::attacks::init()` once at program startup. `atomic-movegen` needs this before any move generation is done.
4. Keep the existing `Cargo.toml` `edition = "2024"` unless the dependency turns out to require an older edition.

---

## 2. `Position` wrapper around `atomic_movegen::Board`

The `Board` type from `atomic-movegen` is the source of truth for pieces, moves, and FEN. However, it does not expose everything we need for a solver, so we introduce a small `Position` wrapper.

1. Create `src/position.rs` (or a `position` module) containing a `Position` struct.
2. `Position` holds:
   - `board: atomic_movegen::board::Board`
   - `rule50: u8` (half-move clock since the last pawn move or capture; `Board` keeps this internally but does not expose it directly)
   - `zobrist: u64` (incremental hash for the transposition and repetition tables)
3. Implement `Position::from_fen(fen: &str) -> Result<Self, ...>`:
   - Parse the FEN with `Board::from_fen`.
   - Extract `rule50` from the same FEN string (tokens 5 and 6).
4. Implement `do_move(m: Move)` and `undo_move(m: Move)`:
   - Use `Board::do_move` and `Board::undo_move` with a `StateInfo`.
   - Update `rule50` manually: `0` if the move is a pawn move or capture, otherwise `previous + 1`.
   - Update `zobrist` incrementally (or recompute from the board after the move; recompute is simpler for the first version).
5. Provide helpers:
   - `legal_moves(&mut self, moves: &mut MoveList)` calls `generate_legal`.
   - `side_to_move()`, `commoners()`, `fen()`, `hash()`, `rule50()`.

---

## 3. Zobrist hashing

`atomic-movegen` does not expose a board hash, so we implement our own.

1. Create `src/zobrist.rs`.
2. Use a simple deterministic pseudo-random generator (e.g. splitmix64) seeded with a fixed value to generate a table of 64-bit keys.
3. Key tables needed:
   - one key per `(color, piece_type, square)` for the 12 colored piece types (2 colors × 6 piece types × 64 squares)
   - one key for side to move
   - four keys for castling rights (one per right)
   - one key per en-passant file (or per square)
   - one key per `rule50` value (optional; include it to make `rule50` part of the position key, which avoids subtle transposition-table bugs with the 50-move rule)
4. `Position::hash()`:
   - Iterate over all occupied squares (or all 64 squares) using `board.piece_on()` and `board.occupied()`.
   - XOR the appropriate piece key, side-to-move key, castling keys, en-passant key, and `rule50` key.
5. Initialize the Zobrist table lazily (e.g. with `std::sync::OnceLock` or `lazy_static` once the code uses a static) and reuse it for every position.

---

## 4. Terminal and draw detection

Atomic chess in this variant ends when a side has no commoners. There is no check/mate in the usual sense.

1. Create `src/rules.rs` or keep the logic inside `Position`/`search`.
2. Terminal outcome for a `Position`:
   - `Outcome::Win` for the side to move: the opponent has no commoners (the previous move blew them up).
   - `Outcome::Loss` for the side to move: this side has no commoners.
   - `Outcome::Draw` if `rule50 >= 100` (50-move rule) or if the position has no legal moves and the side to move still has commoners (stalemate).
3. Repetition handling:
   - Maintain a path set (`HashSet<u64>` or a counter) of hashes along the current search path.
   - If a position hash is already in the path, it is a draw by repetition (the side to move can repeat).
   - Do not store / look up these repetition nodes in the transposition table, because their value is path-dependent.
4. Note: `generate_legal` from `atomic-movegen` already returns an empty list when a side has no commoners, so the terminal detection is mainly a correctness wrapper.

---

## 5. CLI and FEN input

1. Create `src/main.rs` (or `src/cli.rs`):
   - Accept a `--fen` argument; default to the standard starting position.
   - Keep it minimal: use `std::env::args` or `clap`.
2. Parse the FEN, build a `Position`, and start the solver.
3. On exit, print `Outcome` plus the simple requested metrics.

---

## 6. Move representation and principal variation

1. Create `src/notation.rs` or small helpers.
2. Convert a `Move` to UCI string:
   - Use `atomic_movegen::types::sq_str` for `from_sq` and `to_sq`.
   - If the move is a promotion, append the promotion piece letter (`q`/`r`/`b`/`n`).
3. Store the `best_move` in each transposition-table entry to reconstruct the principal variation after the search.
4. When the search finishes, walk the best moves from the root to produce a PV line.

---

## 7. Basic proof-number search (PN) — optional validation step

Before going to DF-PN, implement a small proof-number search to make sure the move generation, terminal detection, and node-evaluation logic are correct.

1. Build an explicit in-memory tree:
   - `Node { outcome, proof, disproof, children, best_child }`.
   - OR nodes for the side to move, AND nodes for the opponent.
2. Expand the most-proving node until the root is solved.
3. Use this to verify a few known forced mates and a couple of simple positions.
4. This step is not strictly required for the final product, but it is a low-risk way to catch bugs before the recursive DF-PN implementation.

---

## 8. Depth-first proof-number search (DF-PN)

This is the main search algorithm.

1. Create `src/search/dfpn.rs`.
2. Define `PN`/`DN` values:
   - Use `u64` with a large `INF` value (e.g. `1 << 60`).
   - `0` means proven.
   - `INF` means unproven / impossible.
3. Terminal values:
   - `Win`  → `(pn = 0, dn = INF)`
   - `Loss` → `(pn = INF, dn = 0)`
   - `Draw` → `(pn = INF, dn = INF)`
4. Recursive `MID` (memory-efficient iterative deepening) function:
   - Generate legal moves.
   - For each child, look up / compute its `(pn, dn)`.
   - Compute the node's `(pn, dn)` from the children:
     - `pn = min(child.dn)` (side to move chooses the best chance to win)
     - `dn = sum(child.pn)` (opponent must refute all replies)
   - Select the most-proving child (minimum `dn` for OR nodes).
   - Compute the child thresholds using the standard DF-PN formulas:
     - `phi_1 = min_child.dn`, `phi_2 = second_min_child.dn`
     - `delta = sum(child.pn)`
     - `child_phi_threshold = parent_delta_threshold - (delta - phi_1)`
     - `child_delta_threshold = min(parent_phi_threshold, phi_2 + 1)`
   - Recurse into the child with those thresholds.
   - Update the child and the parent values.
   - Stop when `pn >= threshold.pn` or `dn >= threshold.dn`.
5. Iterative deepening loop at the root:
   - Start with a small depth parameter (e.g. `threshold = (1, 1)`).
   - Repeatedly call `MID(root, threshold)` and increase the threshold (e.g. double it) for the next iteration.
   - The loop stops once the root is solved (`pn = 0` or `dn = 0` or `pn = INF` and `dn = INF` for a draw).
   - After each completed iteration, print a progress line containing the iteration's depth parameter, the node count so far, and the current nodes-per-second.
   - Keep track of `depth` (the depth parameter of the just-completed iteration) and `nodes` for the output.
6. Move ordering:
   - Create `src/search/ordering.rs`.
   - Define a `MoveScorer` trait (or a function pointer) so the ordering heuristic can be swapped or combined later without touching the search code. Example:
     ```rust
     pub trait MoveScorer {
         fn score(&self, board: &Board, m: Move, state: &StateInfo) -> i32;
     }
     ```
   - Implement a default `StaticAtomicScorer` that assigns a score based on the principles below.
   - The DF-PN search code sorts the `MoveList` with `scorer` before iterating over children.
   - Keep the scoring values as constants or configuration so they can be tuned without recompiling.

   **Default static heuristic for atomic chess:**

   | Priority | Move type | Score contribution |
   |---|---|---|
   | 1 | Winning capture | Capture that blasts the opponent's *last* commoner. Highest score. |
   | 2 | Promotion | Pawn promotes, especially to queen. |
   | 3 | Capture by MVV-LVA | Most-valuable-victim / least-valuable-aggressor captures. |
   | 4 | Check-like threat | Move that attacks the opponent's last commoner after the move. |
   | 5 | Blast-threaten capture | Any capture whose blast zone is near an enemy commoner. |
   | 6 | Centralizing / attacking moves | Piece moves toward the opponent's commoner or to the center. |
   | 7 | Everything else | Default score `0`. |

   The scoring function uses only the current `Board` and `StateInfo` (no move simulation), so it stays fast and easy to extend. Example scoring logic:
   ```rust
   fn score_move(&self, board: &Board, m: Move, state: &StateInfo) -> i32 {
       // ... scoring logic from the discussion above ...
   }
   ```
   Future extensions can implement `MoveScorer` with dynamic heuristics (history, killer, SEE, blast-aware SEE, etc.) and pass it in.
7. Principal variation extraction:
   - After the root is solved, follow the `best_move` stored in the transposition table from the root to the leaves, stopping at a terminal or a repeated draw.

---

## 9. Transposition table

1. Create `src/search/tt.rs`.
2. Define a `TtEntry`:
   - `key: u64` (full Zobrist key, used to detect collisions)
   - `best_move: Move` (or `Move::NONE` for terminals)
   - `pn: u64`
   - `dn: u64`
   - `depth: u32` or `generation` for replacement policy
3. Implement a fixed-size array-based table (power-of-two size) using `key` modulo size for the index.
4. Use a simple replacement policy:
   - Always replace an empty slot.
   - For collisions, prefer the entry with the larger depth/proof number or a more recent generation.
5. Keep the table size configurable via a CLI flag or a constant (e.g. 64 MB, 256 MB).

---

## 10. Output

During iterative deepening the solver prints one line after each completed iteration:

```
depth: <iteration depth>  nodes: <total nodes searched>  nps: <nodes per second>
```

When the search finishes, the final line contains the result:

```
outcome: win
pv: e2e4 e7e6 d1h5 ...
```

1. `depth`: the depth parameter of the completed iterative-deepening iteration.
2. `nodes`: total positions / nodes evaluated so far.
3. `nps`: `nodes / elapsed_seconds` for the completed iteration.
4. `outcome`: `win`, `loss`, or `draw`.
5. `pv`: the principal variation in UCI notation (only for `win`/`loss` in this first set; can be omitted for draws).

---

## 11. Testing and validation

1. **Perft comparison**: Use the `perft` function from `atomic-movegen` to verify that the move generator and our `do_move`/`undo_move` wrapper produce correct counts.
2. **Known positions**: Build a small `tests/positions` file or inline test cases with known outcomes (e.g. simple one- or two-move commoner blasts).
3. **Self-consistency**: Run the solver on a set of positions and verify that `win`/`loss`/`draw` are reported consistently when the side to move is flipped.
4. **FEN round-trip**: Parse a FEN, make a move, undo it, and compare the FEN strings.
5. **Repetition / 50-move**: Test that repetition and 50-move draws are correctly detected.

---

## 12. Final task: capture learnings in `plans/basics/report.md`

After the implementation is done, write a short report in `plans/basics/report.md` that documents:

- What assumptions were confirmed or contradicted during implementation.
- Problems encountered with `atomic-movegen` integration (e.g. the `attacks::init()` requirement, private fields, missing hash, missing `rule50` getter).
- Surprises about DF-PN behavior in the atomic-chess domain (e.g. cycles, draw handling, node counts, move ordering).
- Workarounds used (e.g. recompute Zobrist from the board, track `rule50` manually, treat path repetitions as draws).
- Performance observations and ideas for the next iteration (transposition table size, move ordering, tablebases, etc.).
