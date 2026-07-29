# AGENTS.md

## Goal

A pure solver for atomic chess in Rust.

## Architecture

- `src/lib.rs` re-exports `notation`, `position`, `search`, and `zobrist`.
- `src/position.rs` wraps `atomic_movegen::board::Board` and tracks the
  `Outcome` (Win/Loss/Draw from the side-to-move perspective), undo state,
  and Zobrist hashing.
- `src/search/dfpn/` implements the sequential DF-PN+ solver with optional
  shortest-PV refinement, history/killer heuristics, and a 5-second default
  timeout.
- `src/search/tt/` holds the transposition table, including path-independent
  base entries and path-dependent "twin" entries for repetition handling.
- `src/search/ordering.rs` provides the `MoveScorer` trait and the
  `StaticAtomicScorer`.
- `src/zobrist.rs` generates deterministic Zobrist keys for positions and
  path-dependent move codes.
- `src/notation.rs` provides UCI move helpers.
- `src/main.rs` is the CLI entry point. It accepts `--fen <FEN>` (default
  standard start position), `--tt-size <MB>` (default 64), `--epsilon <VALUE>`
  (default 0.125), `--timeout <SECONDS>` (default 5), `--no-refine-shortest`
  (refinement is enabled by default), `--outcome-only` (disables the pre-exit
  hook and stdin reader), plus `-h`/`--help`. Unknown options exit with an error.
  It prints the outcome and a PV when the result is decisive.
- `examples/` contains example binaries for exploring solver behavior.
- `tests/` contains integration/regression tests.

## Examples

`examples/common.rs` provides shared helpers for the example binaries; it is
not itself a runnable example.

The runnable examples are:

- `benchmark` — Reproducible benchmark harness over a fixed suite of positions.
  Supports `--runs`, `--timeout`, `--epsilon`, `--refine-shortest`, and an
  optional positional name filter. Prints a table with outcome, nodes, child
  evaluations, mean/min/max time, and PV length.
- `find_winning_child` — Enumerates every legal first move, solves the resulting
  child with a short timeout, and reports the first move that is winning for
  the root side (a child `Loss`).
- `play_and_solve` — Plays a user-specified move and then solves the resulting
  position. Useful for inspecting a particular line.
- `solve_depth_limited` — Runs `Search::search_depth` with a fixed
  `max_depth` and no iterative-deepening bootstrap.
- `solve_no_refinement` — Solves a position with the full staged solver but
  with shortest-PV refinement disabled.
- `static_move_scores` — Prints the `StaticAtomicScorer` values for all legal
  moves, sorted from highest to lowest.
- `twin_stats` — Solves GHI-sensitive positions and reports twin-table
  insertion, eviction, and peak-live-twin statistics.
- `verify_ppv` — Verifies that a supplied UCI move list is a Proof Principal
  Variation for a given FEN.

## Output priorities

When the solver must trade off result quality against time or implementation
complexity, prefer them in this order:

1. **Decisive outcome** for deep positions (roughly 30 full moves / 60 plies or
   more).
2. **Proof Principal Variation (PPV)** once the outcome is known.
3. **Shortest PPV (SPPV)** refinement only when time and correctness allow.

This means `solve_outcome` may use the majority of the time budget,
`find_ppv` should return a valid PPV if possible, and `refine_sppv` is the
lowest-priority stage. `--no-refine-shortest` is a normal, well-supported
mode.

## Conventions

- Follow standard Rust 2024 edition idioms.
- Use `cargo clippy`, `cargo fmt`, `cargo test`, and `cargo doc` to ensure
  correctness and code quality.
- Avoid `unsafe` by default; prefer safe Rust. If `unsafe` is needed for a
  measurable performance win, document it clearly and guard it appropriately.
- Name public API types and functions clearly; prefer full words over
  abbreviations. Existing public modules use domain-standard abbreviations
  such as `dfpn`, `tt`, and `zobrist`; prefer full words for new public API
  unless the abbreviation is domain-standard.
- Example binaries go under `examples/`.
- Keep source files under ~10 KB. Files larger than 10 KB must include a short
  documented justification in the file header or in `AGENTS.md`. Files larger
  than ~20 KB should normally be split into submodules.
  - this limit does not hold for `docs/`
- Unit tests go in a `#[cfg(test)] mod tests` at the bottom of each module.
  Integration/regression tests go under `tests/`.
- The most important quality attributes for this library are, from most to
  least important:
  - correctness, performance, maintainability, testability, consistency
- Only use reading `git` commands, never writing ones (no `git add`,
  `git rm`, `git commit`, etc.).
- `docs/plans/` contains prompts, implementation plans and reports
  - ignore all `prompt.md` files
  - implementation plans can be found via `find . -type f -name 'plan*.md'`
  - implementation reports can be found via `find . -type f -name 'report*.md'`
  - the final task of an implementation plan is creating the corresponding implementation report
    - the report should include additional tools/examples used, problems encountered, open ends, next steps
