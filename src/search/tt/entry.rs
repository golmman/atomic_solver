//! Transposition-table entry layout and path-dependent twin entries.

use crate::position::Outcome;
use atomic_movegen::types::Move;

pub const MAX_TWINS: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct TwinEntry {
    pub(crate) path_code: u64,
    pub(crate) path_length: u32,
    pub(crate) outcome: Option<Outcome>, // None means empty
    pub(crate) best_move: Move,
    pub(crate) depth: u32,
    pub(crate) remaining_depth: u32,
}

impl Default for TwinEntry {
    fn default() -> Self {
        Self {
            path_code: 0,
            path_length: 0,
            outcome: None,
            best_move: Move::NONE,
            depth: 0,
            remaining_depth: 0,
        }
    }
}

/// A small, read-only summary of a base transposition-table entry.
///
/// This contains only the fields used by the search hot path so that probes
/// do not have to copy the full `TtEntry` (including its twin slots).
#[derive(Clone, Copy, Debug)]
pub struct TtSummary {
    pub best_move: Move,
    pub outcome: Option<Outcome>,
    pub pn: u64,
    pub dn: u64,
    pub depth: u32,
    pub remaining_depth: u32,
    pub repetition_seen: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TwinAction {
    Inserted,
    Updated,
    Evicted,
}

#[derive(Clone, Copy, Debug)]
pub struct TtEntry {
    pub(crate) key: u64,
    pub(crate) valid: bool,
    pub(crate) generation: u32,

    // Base entry: bounds for unsolved nodes, or path-independent solved results.
    pub(crate) best_move: Move,
    pub(crate) outcome: Option<Outcome>,
    pub(crate) pn: u64,
    pub(crate) dn: u64,
    pub(crate) depth: u32,
    pub(crate) remaining_depth: u32,
    pub(crate) repetition_seen: bool,

    // Twin entries: path-dependent solved results.
    pub(crate) twins: [TwinEntry; MAX_TWINS],
}

impl Default for TtEntry {
    fn default() -> Self {
        Self {
            key: 0,
            valid: false,
            generation: 0,
            best_move: Move::NONE,
            outcome: None,
            pn: 1,
            dn: 1,
            depth: 0,
            remaining_depth: 0,
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

    pub(super) fn store_twin(
        &mut self,
        path_code: u64,
        path_length: u32,
        outcome: Outcome,
        best_move: Move,
        depth: u32,
        remaining_depth: u32,
    ) -> TwinAction {
        // Update an existing twin for the same path.
        for twin in self.twins.iter_mut() {
            if twin.outcome.is_some() && twin.path_code == path_code {
                twin.path_length = path_length;
                twin.outcome = Some(outcome);
                twin.best_move = best_move;
                twin.depth = depth;
                twin.remaining_depth = remaining_depth;
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
            remaining_depth,
        };

        if found_empty {
            TwinAction::Inserted
        } else {
            TwinAction::Evicted
        }
    }

    pub(super) fn clear_twins(&mut self) {
        self.twins = [TwinEntry::default(); MAX_TWINS];
    }

    pub(super) fn reinit_base_for_twin(&mut self) {
        self.best_move = Move::NONE;
        self.outcome = None;
        self.pn = 1;
        self.dn = 1;
        self.depth = 0;
        self.remaining_depth = 0;
        self.repetition_seen = true;
    }

    pub(super) fn live_twin_count(&self) -> u8 {
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
