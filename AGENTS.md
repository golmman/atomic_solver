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
- `src/main.rs` is the CLI entry point; it accepts `--fen <FEN>` and prints
  the outcome plus a PV when the result is decisive.
- `examples/` contains example binaries for exploring solver behavior.
- `tests/` contains integration/regression tests.

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
- Unit tests go in a `#[cfg(test)] mod tests` at the bottom of each module.
  Integration/regression tests go under `tests/`.
- The most important quality attributes for this library are, from most to
  least important:
  - correctness, performance, maintainability, testability, consistency
- Only use reading `git` commands, never writing ones (no `git add`,
  `git rm`, `git commit`, etc.).
- ignore all `prompt.md` files
