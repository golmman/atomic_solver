//! Transposition table for solver results.

use crate::position::Outcome;
use crate::zobrist;
use atomic_movegen::types::Move;

#[derive(Clone, Copy)]
pub struct TtEntry {
    pub key: u64,
    pub best_move: Move,
    pub outcome: Option<Outcome>,
    pub pn: u64,
    pub dn: u64,
    pub depth: u32,
    pub path_code: u64,
    pub repetition_seen: bool,
    pub valid: bool,
}

impl Default for TtEntry {
    fn default() -> Self {
        Self {
            key: 0,
            best_move: Move::NONE,
            outcome: None,
            pn: 1,
            dn: 1,
            depth: 0,
            path_code: 0,
            repetition_seen: false,
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

    #[allow(clippy::too_many_arguments)]
    pub fn store(
        &mut self,
        key: u64,
        best_move: Move,
        outcome: Option<Outcome>,
        pn: u64,
        dn: u64,
        depth: u32,
        path_code: u64,
        repetition_seen: bool,
    ) {
        let mut pn = pn.min(zobrist::INF);
        let mut dn = dn.min(zobrist::INF);
        if outcome.is_none() && pn == zobrist::INF && dn == zobrist::INF {
            pn = 1;
            dn = 1;
        }
        let idx = self.index(key);
        self.table[idx] = TtEntry {
            key,
            best_move,
            outcome,
            pn,
            dn,
            depth,
            path_code,
            repetition_seen,
            valid: true,
        };
    }
}
