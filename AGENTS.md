# AGENTS.md

## Goal

A pure solver for atomic chess in Rust.

## Architecture

- `src/lib.rs` re-exports `notation`, `position`, `proof_tree`, `search`, and
  `zobrist`.
- `src/position.rs` wraps `atomic_movegen::board::Board` and tracks the
  `Outcome` (Win/Loss/Draw from the side-to-move perspective), undo state,
  and Zobrist hashing.
- `src/search/dfpn/` implements the sequential DF-PN+ solver with iterative
  bounded shortest-PV refinement, history/killer heuristics, and a 5-second
  default timeout. `dfpn` emits `NodeProven` events for every node it proves
  or disproves; the returned PV is the shortest decisive line found before the
  timeout or a bounded-search failure.
- `src/search/tt/` holds the transposition table with path-independent base
  entries. Repetition-dependent results are not cached, following the
  first-player-loss GHI shortcut.
- `src/search/ordering.rs` provides the `MoveScorer` trait and the
  `StaticAtomicScorer`.
- `src/proof_tree/mod.rs` provides a `Move`-based in-memory proof tree and a
  background worker that collects `NodeProven` events from the search,
  maintains the tree, enforces a memory budget, and serializes the full proven
  subtree to a compact binary adjacency dump (`src/proof_tree/binary.rs`).
  External tools can import the binary dump into PostgreSQL.
- `src/zobrist.rs` generates deterministic Zobrist keys for positions,
  including the halfmove clock for transposition-table lookup.
- `src/notation.rs` provides UCI move helpers.
- `src/main.rs` is the CLI entry point. It accepts `--fen <FEN>` (default
  standard start position), `--tt-size <MB>` (default 64), `--epsilon <VALUE>`
  (default 0.125), `--timeout <SECONDS>` (default 5), `--first-outcome`
  (stop after the first decisive line without iterative shortest-PV refinement),
  `--outcome-only` (disables the pre-exit hook and stdin reader), `--pt-size <MB>`
  (default 256, max in-memory proof-tree size), `--dump-path <FILE>`
  (default `proof_tree.bin`, binary dump of the full proven subtree), plus
  `-h`/`--help`. Unknown options exit with an error. It prints the outcome and
  a PV when the result is decisive and, by default, logs proof-tree statistics,
  the returned PV, its validity, and writes the binary dump before exit.
- `examples/` contains example binaries for exploring solver behavior.
- `tests/` contains integration/regression tests.

## Examples

`examples/common.rs` provides shared helpers for the example binaries; it is
not itself a runnable example.

The runnable examples are:

- `benchmark` — Reproducible benchmark harness over a fixed suite of positions.
  Supports `--runs`, `--timeout`, `--epsilon`, `--first-outcome`, and an
  optional positional name filter. Prints a table with outcome, nodes, child
  evaluations, mean/min/max time, and PV length.
- `find_winning_child` — Enumerates every legal first move, solves the resulting
  child with a short timeout, and reports the first move that is winning for
  the root side (a child `Loss`).
- `play_and_solve` — Plays a user-specified move and then solves the resulting
  position. Useful for inspecting a particular line.
- `solve_depth_limited` — Runs `Search::search_depth` with a fixed
  `max_depth` and no iterative-deepening bootstrap.
- `static_move_scores` — Prints the `StaticAtomicScorer` values for all legal
  moves, sorted from highest to lowest.
- `verify_ppv` — Verifies that a supplied UCI move list is a Proof Principal
  Variation for a given FEN.

## Output priorities

When the solver must trade off result quality against time or implementation
complexity, prefer them in this order:

1. **Decisive outcome** for deep positions (roughly 30 full moves / 60 plies or
   more).
2. **Shortest decisive PV** returned by `Search::solve` by iterative bounded
   refinement.
3. **Proof tree dump** (`proof_tree.bin`) that records every node proven or
   disproven during the search.

`Search::solve` returns the first decisive line quickly, then uses the
remaining time budget to iteratively shorten it. Use `Search::first_outcome_only`
(or the CLI `--first-outcome` flag) to skip refinement when only a decisive
outcome is needed.

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
- The most important quality attributes for this project are (highest priority first):
  - correctness
  - performance
  - maintainability
  - testability
  - consistency
- Only use reading `git` commands, never writing ones (no `git add`,
  `git rm`, `git commit`, etc.).
- `docs/plans/` contains prompts, implementation plans and reports
  - ignore all `prompt.md` files
  - implementation plans can be found via `find . -type f -name 'plan*.md'`
  - implementation reports can be found via `find . -type f -name 'report*.md'`
  - the final task of an implementation plan is creating the corresponding implementation report
    - the report should include additional tools/examples used, problems encountered, unresolved parts, missing tests, next steps

## Conversational Guidelines

- You are not just a simple developer but a consultant for the user
- Push back if the users ideas or tasks are not sound or need clarification
- Feel free to ask questions where decisions are needed
  - explain the trade-offs for decision options
