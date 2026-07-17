//! Transposition table for solver results.

use crate::position::Outcome;
use crate::zobrist;
use atomic_movegen::types::Move;

const MAX_TWINS: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct TwinEntry {
    pub path_code: u64,
    pub path_length: u32,
    pub outcome: Option<Outcome>, // None means empty
    pub best_move: Move,
    pub depth: u32,
}

impl Default for TwinEntry {
    fn default() -> Self {
        Self {
            path_code: 0,
            path_length: 0,
            outcome: None,
            best_move: Move::NONE,
            depth: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TwinAction {
    Inserted,
    Updated,
    Evicted,
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

    fn store_twin(
        &mut self,
        path_code: u64,
        path_length: u32,
        outcome: Outcome,
        best_move: Move,
        depth: u32,
    ) -> TwinAction {
        // Update an existing twin for the same path.
        for twin in self.twins.iter_mut() {
            if twin.outcome.is_some() && twin.path_code == path_code {
                twin.path_length = path_length;
                twin.outcome = Some(outcome);
                twin.best_move = best_move;
                twin.depth = depth;
                return TwinAction::Updated;
            }
        }

        // Use the first empty slot, or evict slot 0 if all are full.
        let mut empty_or_old = 0;
        let mut found_empty = false;
        for (i, twin) in self.twins.iter_mut().enumerate() {
            if twin.outcome.is_none() {
                empty_or_old = i;
                found_empty = true;
            }
        }

        self.twins[empty_or_old] = TwinEntry {
            path_code,
            path_length,
            outcome: Some(outcome),
            best_move,
            depth,
        };

        if found_empty {
            TwinAction::Inserted
        } else {
            TwinAction::Evicted
        }
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

    fn live_twin_count(&self) -> u8 {
        self.twins
            .iter()
            .filter(|t| t.outcome.is_some())
            .count()
            .min(255) as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryResult {
    pub best_move: Move,
    pub depth: u32,
}

pub struct TranspositionTable {
    table: Vec<[TtEntry; 2]>,
    mask: usize,
    twin_insertions: u64,
    twin_evictions: u64,
    peak_twins: u8,
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
            .find(|&&e| e.valid && e.key == key)
    }

    pub fn clear(&mut self) {
        for bucket in &mut self.table {
            *bucket = [TtEntry::default(); 2];
        }
        self.twin_insertions = 0;
        self.twin_evictions = 0;
        self.peak_twins = 0;
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
        outcome: Option<Outcome>,
        pn: u64,
        dn: u64,
        depth: u32,
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
                if slot.valid && slot.key == key {
                    existing = true;
                    if let Some(o) = outcome {
                        if repetition_seen {
                            twin_action =
                                Some(slot.store_twin(path_code, path_length, o, best_move, depth));
                            live_twins = slot.live_twin_count();
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
            best_move,
            outcome,
            pn,
            dn,
            depth,
            repetition_seen,
            twins: [TwinEntry::default(); MAX_TWINS],
        };

        let new_twin_action = if let Some(o) = outcome {
            if repetition_seen {
                new.reinit_base_for_twin();
                Some(new.store_twin(path_code, path_length, o, best_move, depth))
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

        let bucket = &mut self.table[idx];
        let old = bucket[0];
        bucket[0] = new;
        if old.valid && old.key != key {
            bucket[1] = old;
        }
    }

    pub fn store_twin(
        &mut self,
        key: u64,
        path_code: u64,
        path_length: u32,
        outcome: Outcome,
        best_move: Move,
        depth: u32,
    ) {
        let idx = self.index(key);
        let mut twin_action = None;
        let mut live_twins = 0;

        {
            let bucket = &mut self.table[idx];
            for slot in bucket.iter_mut() {
                if slot.valid && slot.key == key {
                    twin_action =
                        Some(slot.store_twin(path_code, path_length, outcome, best_move, depth));
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
            best_move: Move::NONE,
            outcome: None,
            pn: 1,
            dn: 1,
            depth: 0,
            repetition_seen: true,
            twins: [TwinEntry::default(); MAX_TWINS],
        };
        let action = new.store_twin(path_code, path_length, outcome, best_move, depth);
        self.record_twin_action(action);
        self.peak_twins = self.peak_twins.max(new.live_twin_count());

        let bucket = &mut self.table[idx];
        let old = bucket[0];
        bucket[0] = new;
        if old.valid && old.key != key {
            bucket[1] = old;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tt_entry_size_is_reasonable() {
        // MAX_TWINS was raised to 8; keep the per-entry size bounded so the
        // default 64 MB table still holds a useful number of entries.
        let size = std::mem::size_of::<TtEntry>();
        assert!(size <= 512, "TtEntry size {} exceeds 512 bytes", size);
    }

    #[test]
    fn twin_metrics_track_insertions_and_evictions() {
        let mut tt = TranspositionTable::with_mb(1);
        let key = 12345u64;

        for i in 0..MAX_TWINS as u64 {
            tt.store_twin(key, i, 0, Outcome::Draw, Move::NONE, 0);
        }

        assert_eq!(tt.twin_stats().0, MAX_TWINS as u64);
        assert_eq!(tt.twin_stats().1, 0);

        // One more twin evicts the oldest slot.
        tt.store_twin(key, MAX_TWINS as u64, 0, Outcome::Draw, Move::NONE, 0);
        assert_eq!(tt.twin_stats().0, MAX_TWINS as u64 + 1);
        assert_eq!(tt.twin_stats().1, 1);
    }

    #[test]
    fn clear_resets_twin_stats() {
        let mut tt = TranspositionTable::with_mb(1);
        tt.store_twin(1, 0, 0, Outcome::Draw, Move::NONE, 0);
        tt.clear();
        assert_eq!(tt.twin_stats().0, 0);
        assert_eq!(tt.twin_stats().1, 0);
        assert_eq!(tt.peak_twins(), 0);
    }

    #[test]
    fn peak_twins_tracked() {
        let mut tt = TranspositionTable::with_mb(1);
        let key = 12345u64;

        for i in 0..4 {
            tt.store_twin(key, i, 0, Outcome::Draw, Move::NONE, 0);
        }
        assert_eq!(tt.peak_twins(), 4);

        // A second entry also has four twins; peak stays at 4.
        let key2 = 54321u64;
        for i in 0..2 {
            tt.store_twin(key2, i, 0, Outcome::Draw, Move::NONE, 0);
        }
        assert_eq!(tt.peak_twins(), 4);

        // Filling the first entry to capacity and evicting keeps the peak at 8.
        for i in 4..8 {
            tt.store_twin(key, i, 0, Outcome::Draw, Move::NONE, 0);
        }
        assert_eq!(tt.peak_twins(), 8);
    }

    #[test]
    fn find_and_best_result_for_multiple_paths() {
        let mut tt = TranspositionTable::with_mb(1);
        let key = 123u64;

        // Twins for two different path codes.  Storing a twin reinitialises the
        // base entry, so the table is left with only path-dependent results.
        tt.store_twin(key, 1, 0, Outcome::Loss, Move::NONE, 2);
        tt.store_twin(key, 2, 0, Outcome::Draw, Move::NONE, 3);

        let entry = *tt.probe(key).unwrap();

        // A twin is found only for its own path code and expected outcome.
        assert_eq!(
            entry
                .find_result_for_path(1, Outcome::Loss)
                .map(|r| r.depth),
            Some(2)
        );
        assert!(entry.find_result_for_path(1, Outcome::Win).is_none());

        // best_result_for_path returns the twin for path code 2.
        assert_eq!(
            entry.best_result_for_path(2),
            Some((Move::NONE, Some(Outcome::Draw), 3))
        );

        // The base result has been cleared because path-dependent twins are stored.
        assert!(entry.find_result_for_path(0, Outcome::Win).is_none());

        // Unknown path returns nothing.
        assert!(entry.find_result_for_path(99, Outcome::Loss).is_none());
        assert!(entry.best_result_for_path(99).is_none());
    }
}
