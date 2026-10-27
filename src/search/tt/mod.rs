//! Transposition table for solver results.

mod alloc;
mod entry;
mod table;

#[cfg(test)]
mod tests;

pub use entry::{EntryResult, TtEntry};
pub use table::TranspositionTable;
