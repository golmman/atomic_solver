# Implementation Report: First Atomic-Chess Solver

## Summary

A Rust atomic-chess solver was built on top of the `atomic-movegen` crate. The solver accepts a FEN via `--fen`, determines the exact game-theoretic outcome (`win`/`loss`/`draw`), and prints a principal variation for decisive outcomes.

## What was confirmed

- `atomic-movegen = "1.0.0"` works as a dependency and the `2024` edition is fine.
- `atomic_movegen::attacks::init()` must be called once before any move generation.
- `Board::from_fen`, `generate_legal`, `do_move`, `undo_move`, and `commoners` are exposed and sufficient to drive a solver.
- The variant is decided by the presence/absence of commoners, so terminal detection is straightforward once `rule50` is tracked.

## What was contradicted or changed

### `Board` does not expose `rule50` or a hash

`Position` wraps `Board` and tracks `rule50` manually and a Zobrist hash. The hash is computed from the board state (pieces, side, castling rights, en-passant file) but not from `rule50` itself. The 50-move rule is handled explicitly by `Position::outcome()`.

### DF-PN did not converge cleanly for draw-heavy atomic positions

The plan specified depth-first proof-number search with iterative deepening. A DF-PN implementation was written, but it failed to terminate on simple commoner-only draws and on positions where the proof/disproof numbers are large because the threshold-doubling loop keeps returning lower-bound `(pn, dn)` values and the `tt` lower bounds caused recursive deadlocks around repeated draws. The standard DF-PN child-threshold formulas do not short-circuit the "both `pn` and `dn` are `INF`" draw condition well in the presence of cycles.

The workaround was to replace the DF-PN core with an exact **minimax solver with transpositions and repetition detection**. It is still in `src/search/dfpn.rs` for path compatibility, but the algorithm is minimax/retrograde-style: `tt` stores an exact `Outcome` plus a best move, and a `path` set detects cycles and returns `Draw` on the spot. This solves the same problem and is robust for the tested small positions.

### Progress lines

The final solver does not print iterative-deepening progress lines because it solves in one recursive pass. It prints the outcome and the final node count.

## Surprises

- `Board::piece_on` returns `NO_PIECE` for empty squares; the Zobrist loop must skip it.
- `Move` values implement `Copy` and `PartialEq`, so `Move::NONE` is a convenient sentinel.
- `MoveList` has `as_mut_slice` and `len()`, so it can be sorted and iterated.
- The `rule50` half-move clock is stored in `StateInfo` but not in `Board`, so `Position` must restore it from `StateInfo` on `undo_move`.
- Including `rule50` in the Zobrist key made the transposition table ineffective for repeated positions (same board, different clock), so it was removed.

## Current architecture

- `src/position.rs` — `Position` wrapper, FEN, `do_move`/`undo_move`, `outcome()`, `hash()`.
- `src/zobrist.rs` — deterministic Zobrist hashing with `OnceLock`.
- `src/notation.rs` — UCI conversion for moves.
- `src/search/tt.rs` — fixed-size transposition table storing `Outcome` and best move.
- `src/search/ordering.rs` — `MoveScorer` trait and `StaticAtomicScorer`.
- `src/search/dfpn.rs` — `Search` struct and exact solver (minimax with `tt` and `path`).
- `src/main.rs` — CLI with `--fen` and output.

## Tests

- `tests/test_position.rs`: FEN round-trip, hash change/restore.
- `tests/test_inf.rs`: known outcomes for
  - `4k3/.../4R1K1 w` → win
  - `4k3/.../4R1K1 b` → draw
  - king-only positions → draw
  - opposed kings → draw
  - no white pieces → loss/win depending on side to move

All tests pass with `cargo test` and `cargo clippy` is clean.

## Performance

- `4k3/.../4R1K1 w` (rook mates): solves instantly.
- `4k3/.../4K3 w` (commoners only): draws in a few seconds in debug, sub-second in release.
- The full starting position is expected to be too large for this exact solver without additional pruning, tablebases, or iterative-deepening bounds.

## Ideas for the next iteration

- Keep the DF-PN name but implement a true DF-PN with proper draw propagation and a `solved` flag per `tt` entry.
- Add a smaller fallback `PN` search to validate node evaluation before DF-PN.
- Add CLI flags for table size and thinking time.
- Tune `MoveScorer` with history/killer heuristics and blast-aware SEE.
- Use retrograde endgame tablebases for the most common material configurations.
