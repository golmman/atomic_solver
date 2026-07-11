//! Transposition table for solver results.

use atomic_movegen::types::Move;
use crate::position::Outcome;

#[derive(Clone, Copy)]
pub struct TtEntry {
    pub key: u64,
    pub best_move: Move,
    pub outcome: Option<Outcome>,
    pub generation: u32,
    pub depth: u32,
    pub valid: bool,
}

impl Default for TtEntry {
    fn default() -> Self {
        Self {
            key: 0,
            best_move: Move::NONE,
            outcome: None,
            generation: 0,
            depth: 0,
            valid: false,
        }
    }
}

pub struct TranspositionTable {
    table: Vec<TtEntry>,
    mask: usize,
}

impl TranspositionTable {
    pub fn with_mb(mb: usize) -> Self {
        let bytes = mb.saturating_mul(1024 * 1024);
        let size = (bytes / std::mem::size_of::<TtEntry>()).next_power_of_two();
        let size = size.max(16);
        Self {
            table: vec![TtEntry::default(); size],
            mask: size - 1,
        }
    }

    #[inline]
    fn index(&self, key: u64) -> usize {
        (key as usize) & self.mask
    }

    pub fn probe(&self, key: u64) -> Option<&TtEntry> {
        let e = &self.table[self.index(key)];
        if e.valid && e.key == key {
            Some(e)
        } else {
            None
        }
    }

    pub fn store(
        &mut self,
        key: u64,
        best_move: Move,
        outcome: Option<Outcome>,
        generation: u32,
        depth: u32,
    ) {
        let idx = self.index(key);
        self.table[idx] = TtEntry {
            key,
            best_move,
            outcome,
            generation,
            depth,
            valid: true,
        };
    }
}
