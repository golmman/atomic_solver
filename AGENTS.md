
# AGENTS.md

## Goal

A pure solver for atomic chess in Rust.

## Conventions

- Follow standard Rust 2024 edition idioms.
- Use `cargo clippy`, `cargo fmt`, `cargo test` and `cargo doc` to ensure correctness and code quality.
- Avoid `unsafe` by default; prefer safe Rust. If `unsafe` is needed for a measurable performance win, document it clearly and guard it appropriately.
- Name public API types and functions clearly; prefer full words over abbreviations.
- Example binaries go under `examples/`.
- Tests go in a `#[cfg(test)] mod tests` at the bottom of each module.
- The most important quality attributes for this library are in order from most to least important:
  - correctness, performance, maintainability, testability, consistency
- only use reading `git` commands, never writing ones (no `git add`, `git rm`, `git commit`, etc.)
