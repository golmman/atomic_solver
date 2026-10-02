//! Transposition table for solver results.

mod entry;
mod shard;
mod table;

#[cfg(test)]
mod tests;

pub use entry::{EntryResult, TtEntry};
pub use table::TranspositionTable;
