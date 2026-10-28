// `src/cli.rs` is compiled in two crate roots (this library's test build and
// the binary). Its drift guard pins `STARTPOS_FEN` against
// `Position::STARTPOS_FEN` through the `atomic_solver::` path, which only
// resolves in both roots if the library can name itself.
extern crate self as atomic_solver;

pub mod config;
pub mod notation;
pub mod position;
pub mod proof_event;
pub mod proof_tree;
pub mod reconstruct;
pub mod search;
pub mod tt_snapshot;
pub mod zobrist;

#[cfg(test)]
mod cli;
