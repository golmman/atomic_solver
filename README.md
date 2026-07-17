# atomic_solver

A pure Rust solver for atomic chess based on a sequential **DF-PN+** search.

## What it does

Given an atomic-chess position, `atomic_solver` determines whether the side to
move can force a **Win**, **Loss**, or **Draw**, and prints a principal
variation (PV) when the result is decisive.

The solver is built on top of [`atomic-movegen`](https://crates.io/crates/atomic-movegen) 2.0.0.

## Quick start

```bash
cargo run -- --fen "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
```

The default CLI search uses a 64 MB transposition table and a 5-second timeout.

## Library usage

```rust
use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
let mut search = Search::new(64);          // TT size in MB
search.set_timeout(10);                    // seconds
search.refine_shortest(true);              // find a shortest decisive PV
let (outcome, pv, nodes) = search.solve(&mut pos);
```

- `Outcome::Win` / `Outcome::Loss` / `Outcome::Draw` are always from the
  perspective of the side to move.
- `Search::search_depth` solves to a fixed maximum depth without refinement.
- `Search::refine_shortest(true)` first bootstraps a decisive result at small
  depth limits, then refines to a shortest PV.

## How it works

- **DF-PN+** proof-number search with iterative deepening at the root when
  shortest-PV refinement is enabled.
- A transposition table with path-independent base entries and path-dependent
  "twin" entries, so repeated positions are handled correctly.
- Deterministic Zobrist hashing keyed by piece layout, side to move,
  castling/en-passant, `rule50`, and a path code.
- Static move ordering (winning captures, promotions, MVV-LVA captures,
  threats, blast proximity, centralization) plus history and killer heuristics.
- Terminal detection: no own commoner = loss, no opponent commoner = win,
  `rule50 >= 100` = draw, only kings left = draw.

## Public API

- `atomic_solver::position::{Position, Outcome}` — FEN parsing, move/undo,
  terminal detection, Zobrist hash.
- `atomic_solver::search::dfpn::Search` — main solver.
- `atomic_solver::search::ordering` — `MoveScorer` trait and
  `StaticAtomicScorer`.
- `atomic_solver::search::tt` — transposition-table types.
- `atomic_solver::notation` — UCI move helpers.
- `atomic_solver::zobrist` — position and path hashing.

## Examples

Run with `cargo run --example <name> -- [args]`:

- `find_winning_child` — try every first move and report one that wins.
- `play_and_solve` — play a given move, then solve the resulting position.
- `solve_depth_limited` — solve with a fixed depth bound.
- `solve_no_refinement` — solve without shortest-PV refinement.
- `static_move_scores` — print static move-ordering scores for a position.

## Development

```bash
cargo fmt
cargo clippy
cargo test
cargo doc
```

Unit tests live in `#[cfg(test)]` modules at the bottom of source files;
integration/regression tests live under `tests/`.

## License

See [LICENSE](LICENSE).
