//! Transposition table for solver results.

mod entry;
mod table;

#[cfg(test)]
mod tests;

pub use entry::{EntryResult, MAX_TWINS, TtEntry, TwinEntry};
pub use table::TranspositionTable;
