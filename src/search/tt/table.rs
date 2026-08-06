//! Transposition table storage, lookup, and twin accounting.

use crate::position::Outcome;
use crate::zobrist;
use atomic_movegen::types::Move;

use super::entry::{MAX_TWINS, TtEntry, TtSummary, TwinAction, TwinEntry};

pub struct TranspositionTable {
    table: Vec<[TtEntry; 2]>,
    mask: usize,
    current_generation: u32,
    twin_insertions: u64,
    twin_evictions: u64,
    peak_twins: u8,
}

impl TranspositionTable {
    #[cfg(test)]
    pub(crate) fn bucket_count(&self) -> usize {
        self.table.len()
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
            twin_insertions: 0,
            twin_evictions: 0,
            peak_twins: 0,
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
    ///
    /// This does not copy the twin array.
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
            repetition_seen: e.repetition_seen,
        })
    }

    /// Return the best move to use for `key` on `path_code`.
    ///
    /// Prefers a path-independent solved base result, then a twin for `path_code`,
    /// then the unsolved base `best_move`.  This avoids copying the full entry.
    pub fn probe_best_move(&self, key: u64, path_code: u64) -> Option<Move> {
        let entry = self.probe(key)?;
        if entry.outcome.is_some() && !entry.repetition_seen {
            return Some(entry.best_move);
        }
        for twin in entry.twins.iter() {
            if twin.outcome.is_some() && twin.path_code == path_code {
                return Some(twin.best_move);
            }
        }
        if entry.best_move != Move::NONE && entry.outcome.is_none() {
            return Some(entry.best_move);
        }
        None
    }

    pub fn clear(&mut self) {
        for bucket in &mut self.table {
            *bucket = [TtEntry::default(); 2];
        }
        self.current_generation = 1;
        self.twin_insertions = 0;
        self.twin_evictions = 0;
        self.peak_twins = 0;
    }

    /// Mark every existing table entry as belonging to an older generation.
    ///
    /// This is logically equivalent to clearing the table without zeroing any
    /// buckets.  On the extremely rare `u32` wrap, the table is physically
    /// cleared and the generation counter is reset.
    pub fn new_generation(&mut self) {
        self.current_generation = self.current_generation.wrapping_add(1);
        if self.current_generation == 0 {
            self.clear();
        }
    }

    pub fn twin_stats(&self) -> (u64, u64) {
        (self.twin_insertions, self.twin_evictions)
    }

    /// Maximum number of live twins observed in any single entry so far.
    pub fn peak_twins(&self) -> u8 {
        self.peak_twins
    }

    #[inline]
    fn record_twin_action(&mut self, action: TwinAction) {
        match action {
            TwinAction::Inserted => self.twin_insertions += 1,
            TwinAction::Evicted => {
                self.twin_insertions += 1;
                self.twin_evictions += 1;
            }
            TwinAction::Updated => {}
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
        path_code: u64,
        path_length: u32,
        repetition_seen: bool,
    ) {
        let mut pn = pn.min(zobrist::INF);
        let mut dn = dn.min(zobrist::INF);
        if outcome.is_none() && pn == zobrist::INF && dn == zobrist::INF {
            pn = 1;
            dn = 1;
        }

        let idx = self.index(key);
        let mut twin_action = None;
        let mut existing = false;
        let mut live_twins = 0;

        {
            let bucket = &mut self.table[idx];
            for slot in bucket.iter_mut() {
                if slot.valid && slot.key == key && slot.generation == self.current_generation {
                    existing = true;
                    slot.generation = self.current_generation;
                    slot.work = slot.work.max(work);
                    if let Some(o) = outcome {
                        if repetition_seen {
                            twin_action = Some(slot.store_twin(
                                path_code,
                                path_length,
                                o,
                                best_move,
                                depth,
                                remaining_depth,
                            ));
                            live_twins = slot.live_twin_count();
                            slot.reinit_base_for_twin();
                        } else {
                            // Path-independent result: store in the base entry and clear twins.
                            slot.best_move = best_move;
                            slot.best_child = best_child;
                            slot.outcome = Some(o);
                            slot.pn = pn;
                            slot.dn = dn;
                            slot.depth = depth;
                            slot.remaining_depth = remaining_depth;
                            slot.repetition_seen = false;
                            slot.clear_twins();
                        }
                    } else if slot.outcome.is_none() {
                        // Unsolved node: update base bounds and keep existing twins.
                        slot.best_move = best_move;
                        slot.best_child = best_child;
                        slot.outcome = None;
                        slot.pn = pn;
                        slot.dn = dn;
                        slot.depth = depth;
                        slot.remaining_depth = remaining_depth;
                        slot.repetition_seen = repetition_seen;
                    } else {
                        // Do not overwrite a solved base entry with unsolved bounds;
                        // the solved result is still valid and will be preferred by
                        // proof-tree reconstruction.  `work` was already updated above.
                    }
                    break;
                }
            }
        }

        if let Some(action) = twin_action {
            self.record_twin_action(action);
            self.peak_twins = self.peak_twins.max(live_twins);
        }

        if existing {
            return;
        }

        let mut new = TtEntry {
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
            repetition_seen,
            twins: [TwinEntry::default(); MAX_TWINS],
        };

        let new_twin_action = if let Some(o) = outcome {
            if repetition_seen {
                new.reinit_base_for_twin();
                Some(new.store_twin(path_code, path_length, o, best_move, depth, remaining_depth))
            } else {
                new.clear_twins();
                None
            }
        } else {
            None
        };

        if let Some(action) = new_twin_action {
            self.record_twin_action(action);
            self.peak_twins = self.peak_twins.max(new.live_twin_count());
        }

        self.insert_new(idx, new);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn store_twin(
        &mut self,
        key: u64,
        path_code: u64,
        path_length: u32,
        outcome: Outcome,
        best_move: Move,
        depth: u32,
        remaining_depth: u32,
        work: u64,
    ) {
        let idx = self.index(key);
        let mut twin_action = None;
        let mut live_twins = 0;

        {
            let bucket = &mut self.table[idx];
            for slot in bucket.iter_mut() {
                if slot.valid && slot.key == key && slot.generation == self.current_generation {
                    slot.work = slot.work.max(work);
                    twin_action = Some(slot.store_twin(
                        path_code,
                        path_length,
                        outcome,
                        best_move,
                        depth,
                        remaining_depth,
                    ));
                    slot.generation = self.current_generation;
                    live_twins = slot.live_twin_count();
                    slot.reinit_base_for_twin();
                    break;
                }
            }
        }

        if let Some(action) = twin_action {
            self.record_twin_action(action);
            self.peak_twins = self.peak_twins.max(live_twins);
            return;
        }

        let mut new = TtEntry {
            key,
            valid: true,
            generation: self.current_generation,
            best_move: Move::NONE,
            best_child: u8::MAX,
            work,
            outcome: None,
            pn: 1,
            dn: 1,
            depth: 0,
            remaining_depth: 0,
            repetition_seen: true,
            twins: [TwinEntry::default(); MAX_TWINS],
        };
        let action = new.store_twin(
            path_code,
            path_length,
            outcome,
            best_move,
            depth,
            remaining_depth,
        );
        self.record_twin_action(action);
        self.peak_twins = self.peak_twins.max(new.live_twin_count());

        self.insert_new(idx, new);
    }

    /// Place a new entry into `idx`, preferring the two most valuable entries.
    ///
    /// Empty or stale slots are used first. If both slots are live, the lower-
    /// priority existing entry is evicted. Preference order: live in the current
    /// generation, then solved, then higher `work`, then newer generation.
    fn insert_new(&mut self, idx: usize, new: TtEntry) {
        let current_generation = self.current_generation;
        let score = |e: &TtEntry| {
            let live = e.valid && e.generation == current_generation;
            let solved = e.outcome.is_some() || e.twins.iter().any(|t| t.outcome.is_some());
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
