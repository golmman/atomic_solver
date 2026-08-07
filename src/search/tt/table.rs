//! Transposition table storage and lookup.

use crate::position::Outcome;
use crate::zobrist::INF;
use atomic_movegen::types::Move;

use super::entry::{TtEntry, TtSummary};

pub struct TranspositionTable {
    table: Vec<[TtEntry; 2]>,
    mask: usize,
    current_generation: u32,
}

impl TranspositionTable {
    #[cfg(test)]
    pub(crate) fn bucket_count(&self) -> usize {
        self.table.len()
    }

    /// Construct a table with the given number of buckets (rounded up to the
    /// next power of two). This is `pub(crate)` so unit tests can force
    /// collisions and eviction deterministically.
    #[cfg(test)]
    pub(crate) fn with_capacity(buckets: usize) -> Self {
        let buckets = buckets.next_power_of_two().max(1);
        Self {
            table: vec![[TtEntry::default(); 2]; buckets],
            mask: buckets - 1,
            current_generation: 1,
        }
    }

    pub fn with_mb(mb: usize) -> Self {
        let bytes = mb.saturating_mul(1024 * 1024);
        let entries = (bytes / std::mem::size_of::<TtEntry>()).next_power_of_two();
        let entries = entries.max(32);
        let buckets = entries.max(2) / 2;
        Self {
            table: vec![[TtEntry::default(); 2]; buckets],
            mask: buckets - 1,
            current_generation: 1,
        }
    }

    #[inline]
    fn index(&self, key: u64) -> usize {
        (key as usize) & self.mask
    }

    pub fn probe(&self, key: u64) -> Option<&TtEntry> {
        self.table[self.index(key)]
            .iter()
            .find(|e| e.valid && e.key == key && e.generation == self.current_generation)
    }

    /// Return a small copy of the base fields for `key`.
    pub fn probe_summary(&self, key: u64) -> Option<TtSummary> {
        self.probe(key).map(|e| TtSummary {
            best_move: e.best_move,
            best_child: e.best_child,
            work: e.work,
            outcome: e.outcome,
            pn: e.pn,
            dn: e.dn,
            depth: e.depth,
            remaining_depth: e.remaining_depth,
        })
    }

    /// Return the best move for `key`.
    ///
    /// Returns the stored best move for solved entries (it may be `Move::NONE`
    /// for terminal positions) and for unsolved entries that already have a
    /// preferred move.
    pub fn probe_best_move(&self, key: u64) -> Option<Move> {
        let entry = self.probe(key)?;
        if entry.outcome.is_some() || entry.best_move != Move::NONE {
            Some(entry.best_move)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        for bucket in &mut self.table {
            *bucket = [TtEntry::default(); 2];
        }
        self.current_generation = 1;
    }

    /// Mark every existing table entry as belonging to an older generation.
    pub fn new_generation(&mut self) {
        self.current_generation = self.current_generation.wrapping_add(1);
        if self.current_generation == 0 {
            self.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn store(
        &mut self,
        key: u64,
        best_move: Move,
        best_child: u8,
        work: u64,
        outcome: Option<Outcome>,
        pn: u64,
        dn: u64,
        depth: u32,
        remaining_depth: u32,
    ) {
        let mut pn = pn.min(INF);
        let mut dn = dn.min(INF);
        if outcome.is_none() && pn == INF && dn == INF {
            pn = 1;
            dn = 1;
        }

        let idx = self.index(key);
        let mut existing = false;

        {
            let bucket = &mut self.table[idx];
            for slot in bucket.iter_mut() {
                if slot.valid && slot.key == key && slot.generation == self.current_generation {
                    existing = true;
                    slot.generation = self.current_generation;
                    slot.work = slot.work.max(work);
                    if let Some(o) = outcome {
                        // Solved, path-independent result: overwrite the base entry.
                        slot.best_move = best_move;
                        slot.best_child = best_child;
                        slot.outcome = Some(o);
                        slot.pn = pn;
                        slot.dn = dn;
                        slot.depth = depth;
                        slot.remaining_depth = remaining_depth;
                    } else if slot.outcome.is_none() {
                        // Unsolved node: update base bounds.
                        slot.best_move = best_move;
                        slot.best_child = best_child;
                        slot.outcome = None;
                        slot.pn = pn;
                        slot.dn = dn;
                        slot.depth = depth;
                        slot.remaining_depth = remaining_depth;
                    } else {
                        // Do not overwrite a solved base entry with unsolved bounds.
                    }
                    break;
                }
            }
        }

        if existing {
            return;
        }

        let new = TtEntry {
            key,
            valid: true,
            generation: self.current_generation,
            best_move,
            best_child,
            work,
            outcome,
            pn,
            dn,
            depth,
            remaining_depth,
        };

        self.insert_new(idx, new);
    }

    /// Return a distribution of stored `best_child` values among live entries.
    ///
    /// `u8::MAX` (the "unknown" sentinel) is excluded. This is useful for
    /// debugging proof-tree and GHI path-code usage.
    pub fn best_child_counts(&self) -> Vec<(u8, usize)> {
        let mut counts = std::collections::HashMap::new();
        for bucket in &self.table {
            for entry in bucket {
                if entry.valid
                    && entry.generation == self.current_generation
                    && entry.best_child != u8::MAX
                {
                    *counts.entry(entry.best_child).or_insert(0) += 1;
                }
            }
        }
        let mut v: Vec<_> = counts.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v
    }

    /// Return aggregate statistics about the current transposition table contents.
    ///
    /// Tuple fields are: `(buckets, live_entries, solved_entries, unsolved_entries, generation)`.
    pub fn stats(&self) -> (usize, usize, usize, usize, u32) {
        let mut live = 0;
        let mut solved = 0;
        for bucket in &self.table {
            for entry in bucket {
                if entry.valid && entry.generation == self.current_generation {
                    live += 1;
                    if entry.outcome.is_some() {
                        solved += 1;
                    }
                }
            }
        }
        let unsolved = live - solved;
        (
            self.table.len(),
            live,
            solved,
            unsolved,
            self.current_generation,
        )
    }

    /// Place a new entry into `idx`, preferring the two most valuable entries.
    fn insert_new(&mut self, idx: usize, new: TtEntry) {
        let current_generation = self.current_generation;
        let score = |e: &TtEntry| {
            let live = e.valid && e.generation == current_generation;
            let solved = e.outcome.is_some();
            (live as u8, solved as u8, e.work, e.generation)
        };

        let bucket = &mut self.table[idx];
        if !bucket[0].valid || bucket[0].generation != current_generation {
            bucket[0] = new;
        } else if !bucket[1].valid || bucket[1].generation != current_generation {
            bucket[1] = new;
        } else {
            let evict = if score(&bucket[0]) < score(&bucket[1]) {
                0
            } else {
                1
            };
            bucket[evict] = new;
        }
    }
}
