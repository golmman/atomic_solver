# atomic_solver

A pure Rust solver for atomic chess based on a sequential **DF-PN+** search.

## What it does

Given an atomic-chess position, `atomic_solver` determines whether the side to
move can force a **Win**, **Loss**, or **Draw**, and prints a principal
variation (PV) when the result is decisive.

The solver is built on top of [`atomic-movegen`](https://crates.io/crates/atomic-movegen) 2.1.0.

## Quick start

```bash
cargo run -- --fen "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
```

The default CLI search uses a 64 MB transposition table and a 5-second timeout. Use `--tt-size <MB>` to change the table size.

## Library usage

```rust
use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
let mut search = Search::new(64);          // TT size in MB
search.set_timeout(10);                    // seconds
let (outcome, pv, nodes) = search.solve(&mut pos);
```

- `Outcome::Win` / `Outcome::Loss` / `Outcome::Draw` are always from the
  perspective of the side to move.
- `Search::search_depth` solves to a fixed maximum depth without refinement.
- `Search::solve` iteratively refines the PV to find the shortest decisive
  line within the configured timeout. Call `search.first_outcome_only(true)`
  (or use `--first-outcome` in the CLI) to stop after the first decisive line.

## Features and techniques

### Core solving algorithm

- **DF-PN+** — Depth-First Proof-Number search with epsilon-inflated
  thresholds (`1 + epsilon`, default `epsilon = 0.125`) to reduce re-searches.
- **OR/AND tree search** — Alternating proof/disproof number aggregation:
  OR nodes use `min(pn)` / sum `dn`; AND nodes use sum `pn` / `min(dn)`.
- **Iterative bounded solving** — `solve()` first finds any decisive outcome and
  then repeatedly runs `dfpn` with `max_depth = current_pv_len - 2` to find a
  shorter decisive line. Each probe is work-bounded, reusing the transposition
  table and heuristics across chunks, and the search terminates when a shorter
  line cannot be found or the timeout is reached.
- **Proof-tree emission** — `dfpn` emits `ProofEvent` nodes (now carrying a
  Zobrist hash) for every node it proves or disproves. A background proof-tree
  worker consumes these events, accumulates the tree across the whole run, and
  runs a `finalize()` pass that copies fully expanded canonical subtrees onto
  unexpanded transpositions, producing an authoritative proven subtree.

### Transposition table and repetition

- **2-slot bucketed hash table** indexed by Zobrist key, with work-weighted,
  generation-aware replacement.
- **Path-independent base entries** for normal solved/unsolved results.
- **Path-dependent "twin" entries** (up to 8 per position) to handle repeated
  positions and Graph-History Interaction (GHI) correctly.
- **Kawano-style simulation** — verifies a twin found on a different path can
  be reused on the current path before it is stored as a new twin.

### Move ordering and heuristics

- **Static atomic scoring** — domain-specific move ordering that prioritizes:
  1. Winning captures (blast removes the opponent's last commoner).
  2. Promotions.
  3. MVV-LVA captures.
  4. Threats on / near the opponent's last commoner.
  5. Blast-threat proximity to enemy commoners.
  6. Moves that approach the nearest enemy commoner.
  7. Centralization.
- **History heuristic** — `from`/`to` table with additive bonuses, a cap, and
  periodic halving (aging).
- **Killer moves** — up to 2 killer moves per ply added to the static score.
- **TT best-move ordering** — the move stored in the transposition table is
  placed first in the move list.

### Principal variation handling

- **PV extraction** — walks the transposition table from the root following best
  moves, including path-dependent twin entries.
- **PV validation** — validates that an extracted line reaches the expected
  terminal outcome.
- **Iterative shortest-PV refinement** — repeatedly searches with a tighter
  depth bound to find progressively shorter decisive lines.

### Terminal detection

`Position::outcome` checks, in priority order:
- Own commoner extinct → `Loss`.
- Opponent commoner extinct → `Win`.
- No legal moves and in check → `Loss`.
- No legal moves and not in check → `Draw`.
- `rule50 >= 100` → `Draw`.
- Only two pieces left → `Draw`.

### Hashing

- **Zobrist position key** — combines the board hash with a key for `rule50`.
- **Repetition key** — board-only, ignoring `rule50`.
- **Path code** — order-sensitive XOR of `(move, depth)` random keys, used to
  distinguish twins reached by different move orders.

## Public API

- `atomic_solver::position::{Position, Outcome}` — FEN parsing, move/undo,
  terminal detection, Zobrist hash.
- `atomic_solver::search::dfpn::Search` — main solver.
- `atomic_solver::search::ordering` — `MoveScorer` trait and
  `StaticAtomicScorer`.
- `atomic_solver::search::tt` — transposition-table types.
- `atomic_solver::proof_event::{ProofEvent, NodeProven}` — search-to-worker
  event protocol; `NodeProven` includes the position Zobrist hash.
- `atomic_solver::proof_tree` — in-memory proof tree, worker handle, and
  `finalize()` pass for producing an authoritative proven subtree.
- `atomic_solver::notation` — UCI move helpers.
- `atomic_solver::zobrist` — position and path hashing.

## Examples

Run with `cargo run --example <name> -- [args]`:

- `benchmark` — reproducible benchmark over a fixed suite of positions. Supports
  `--suite default|move-order|decisive|all`, `--first-outcome`, and `--runs N`.
- `chunk_growth` — explore work-chunk growth settings.
- `find_winning_child` — try every first move and report one that wins.
- `inspect_pt` — dump a binary proof tree to human-readable JSON.
- `list_legal` — list all legal UCI moves and the terminal outcome for a FEN.
- `move_order_debug` — print static, history, killer and total move-ordering scores.
  Use `--name <case>` to inspect a move-order benchmark position.
- `play_and_solve` — play a given move, then solve the resulting position.
- `replay` — replay a UCI line from a FEN and solve the resulting position.
- `solve_depth_limited` — solve with a fixed depth bound.
- `static_move_scores` — print static move-ordering scores for a position.
  Use `--name <case>` to inspect a move-order benchmark position.
- `twin_stats` — report transposition-table statistics for GHI-sensitive positions.
- `verify_ppv` — verify that a supplied UCI move list is a PPV for a FEN.

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
