//! Transposition table for solver results.

use crate::position::Outcome;
use crate::zobrist;
use atomic_movegen::types::Move;

const MAX_TWINS: usize = 2;

#[derive(Clone, Copy, Debug)]
pub struct TwinEntry {
    pub path_code: u64,
    pub outcome: Option<Outcome>, // None means empty
    pub best_move: Move,
    pub depth: u32,
}

impl Default for TwinEntry {
    fn default() -> Self {
        Self {
            path_code: 0,
            outcome: None,
            best_move: Move::NONE,
            depth: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TtEntry {
    pub key: u64,
    pub valid: bool,

    // Base entry: bounds for unsolved nodes, or path-independent solved results.
    pub best_move: Move,
    pub outcome: Option<Outcome>,
    pub pn: u64,
    pub dn: u64,
    pub depth: u32,
    pub repetition_seen: bool,

    // Twin entries: path-dependent solved results.
    pub twins: [TwinEntry; MAX_TWINS],
}

impl Default for TtEntry {
    fn default() -> Self {
        Self {
            key: 0,
            valid: false,
            best_move: Move::NONE,
            outcome: None,
            pn: 1,
            dn: 1,
            depth: 0,
            repetition_seen: false,
            twins: [TwinEntry::default(); MAX_TWINS],
        }
    }
}

impl TtEntry {
    /// Find a cached result for `path_code` that matches `expected`.
    /// Returns the path-independent base entry if available, or a matching twin.
    pub fn find_result_for_path(&self, path_code: u64, expected: Outcome) -> Option<EntryResult> {
        if self.outcome == Some(expected) && !self.repetition_seen {
            return Some(EntryResult {
                best_move: self.best_move,
                depth: self.depth,
            });
        }
        for twin in self.twins.iter() {
            if twin.outcome == Some(expected) && twin.path_code == path_code {
                return Some(EntryResult {
                    best_move: twin.best_move,
                    depth: twin.depth,
                });
            }
        }
        None
    }

    /// Return the best move and outcome for `path_code`, preferring a
    /// path-independent base result.
    pub fn best_result_for_path(&self, path_code: u64) -> Option<(Move, Option<Outcome>, u32)> {
        if self.outcome.is_some() && !self.repetition_seen {
            return Some((self.best_move, self.outcome, self.depth));
        }
        for twin in self.twins.iter() {
            if twin.outcome.is_some() && twin.path_code == path_code {
                return Some((twin.best_move, twin.outcome, twin.depth));
            }
        }
        None
    }

    fn store_twin(&mut self, path_code: u64, outcome: Outcome, best_move: Move, depth: u32) {
        // Replace an existing twin for the same path, or use the first empty
        // or oldest slot.
        let mut empty_or_old = 0;
        for (i, twin) in self.twins.iter_mut().enumerate() {
            if twin.outcome.is_some() && twin.path_code == path_code {
                twin.outcome = Some(outcome);
                twin.best_move = best_move;
                twin.depth = depth;
                return;
            }
            if twin.outcome.is_none() {
                empty_or_old = i;
            }
        }
        self.twins[empty_or_old] = TwinEntry {
            path_code,
            outcome: Some(outcome),
            best_move,
            depth,
        };
    }

    fn clear_twins(&mut self) {
        self.twins = [TwinEntry::default(); MAX_TWINS];
    }

    fn reinit_base_for_twin(&mut self) {
        self.best_move = Move::NONE;
        self.outcome = None;
        self.pn = 1;
        self.dn = 1;
        self.depth = 0;
        self.repetition_seen = true;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EntryResult {
    pub best_move: Move,
    pub depth: u32,
}

pub struct TranspositionTable {
    table: Vec<[TtEntry; 2]>,
    mask: usize,
}

impl TranspositionTable {
    pub fn with_mb(mb: usize) -> Self {
        let bytes = mb.saturating_mul(1024 * 1024);
        let entries = (bytes / std::mem::size_of::<TtEntry>()).next_power_of_two();
        let entries = entries.max(32);
        let buckets = entries.max(2) / 2;
        Self {
            table: vec![[TtEntry::default(); 2]; buckets],
            mask: buckets - 1,
        }
    }

    #[inline]
    fn index(&self, key: u64) -> usize {
        (key as usize) & self.mask
    }

    pub fn probe(&self, key: u64) -> Option<&TtEntry> {
        self.table[self.index(key)]
            .iter()
            .find(|&&e| e.valid && e.key == key)
    }

    pub fn clear(&mut self) {
        for bucket in &mut self.table {
            *bucket = [TtEntry::default(); 2];
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
        let bucket = &mut self.table[idx];

        for slot in bucket.iter_mut() {
            if slot.valid && slot.key == key {
                if let Some(o) = outcome {
                    if repetition_seen {
                        // Path-dependent result: keep as a twin and reset the base.
                        slot.store_twin(path_code, o, best_move, depth);
                        slot.reinit_base_for_twin();
                    } else {
                        // Path-independent result: store in the base entry and clear twins.
                        slot.best_move = best_move;
                        slot.outcome = Some(o);
                        slot.pn = pn;
                        slot.dn = dn;
                        slot.depth = depth;
                        slot.repetition_seen = false;
                        slot.clear_twins();
                    }
                } else {
                    // Unsolved node: update base bounds and keep existing twins.
                    slot.best_move = best_move;
                    slot.outcome = None;
                    slot.pn = pn;
                    slot.dn = dn;
                    slot.depth = depth;
                    slot.repetition_seen = repetition_seen;
                }
                return;
            }
        }

        // No exact match: create a new primary entry, keeping the old primary in
        // the secondary slot to reduce collisions.
        let old = bucket[0];
        let mut new = TtEntry {
            key,
            valid: true,
            best_move,
            outcome,
            pn,
            dn,
            depth,
            repetition_seen,
            twins: [TwinEntry::default(); MAX_TWINS],
        };

        if let Some(o) = outcome {
            if repetition_seen {
                new.reinit_base_for_twin();
                new.store_twin(path_code, o, best_move, depth);
            } else {
                new.clear_twins();
            }
        }

        bucket[0] = new;
        if old.valid && old.key != key {
            bucket[1] = old;
        }
    }

    pub fn store_twin(
        &mut self,
        key: u64,
        path_code: u64,
        outcome: Outcome,
        best_move: Move,
        depth: u32,
    ) {
        let idx = self.index(key);
        let bucket = &mut self.table[idx];

        for slot in bucket.iter_mut() {
            if slot.valid && slot.key == key {
                slot.store_twin(path_code, outcome, best_move, depth);
                slot.reinit_base_for_twin();
                return;
            }
        }

        let old = bucket[0];
        let mut new = TtEntry {
            key,
            valid: true,
            best_move: Move::NONE,
            outcome: None,
            pn: 1,
            dn: 1,
            depth: 0,
            repetition_seen: true,
            twins: [TwinEntry::default(); MAX_TWINS],
        };
        new.store_twin(path_code, outcome, best_move, depth);
        bucket[0] = new;
        if old.valid && old.key != key {
            bucket[1] = old;
        }
    }
}
