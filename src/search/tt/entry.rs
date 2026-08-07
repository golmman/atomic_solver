//! Transposition-table entry layout.

use crate::position::Outcome;
use atomic_movegen::types::Move;

/// A small, read-only summary of a transposition-table entry.
///
/// This contains only the fields used by the search hot path so that probes
/// do not have to copy the full `TtEntry`.
#[derive(Clone, Copy, Debug)]
pub struct TtSummary {
    pub best_move: Move,
    pub best_child: u8,
    pub work: u64,
    pub outcome: Option<Outcome>,
    pub pn: u64,
    pub dn: u64,
    pub depth: u32,
    pub remaining_depth: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct TtEntry {
    pub(crate) key: u64,
    pub(crate) valid: bool,
    pub(crate) generation: u32,

    pub(crate) best_move: Move,
    pub(crate) best_child: u8, // u8::MAX means "unknown / unset"
    pub(crate) work: u64,      // cumulative child_evals spent under this subtree
    pub(crate) outcome: Option<Outcome>,
    pub(crate) pn: u64,
    pub(crate) dn: u64,
    pub(crate) depth: u32,
    pub(crate) remaining_depth: u32,
}

impl Default for TtEntry {
    fn default() -> Self {
        Self {
            key: 0,
            valid: false,
            generation: 0,
            best_move: Move::NONE,
            best_child: u8::MAX,
            work: 0,
            outcome: None,
            pn: 1,
            dn: 1,
            depth: 0,
            remaining_depth: 0,
        }
    }
}

impl TtEntry {
    /// Return the cached result for `expected` if the base entry stores one.
    #[must_use]
    pub fn result_for(&self, expected: Outcome) -> Option<EntryResult> {
        if self.outcome == Some(expected) {
            Some(EntryResult {
                best_move: self.best_move,
                depth: self.depth,
            })
        } else {
            None
        }
    }

    /// Return the cached result for `expected` only if its stored depth matches
    /// `remaining`. Used to extract PVs whose length is known from a bounded
    /// solve.
    #[must_use]
    pub fn result_for_depth(&self, expected: Outcome, remaining: u32) -> Option<EntryResult> {
        if self.outcome == Some(expected) && self.depth == remaining {
            Some(EntryResult {
                best_move: self.best_move,
                depth: self.depth,
            })
        } else {
            None
        }
    }

    /// Return the best move, outcome, and depth stored in the base entry.
    ///
    /// This is used by PV extraction when no expected outcome is supplied yet.
    #[must_use]
    pub fn best_result(&self) -> Option<(Move, Outcome, u32)> {
        self.outcome.map(|o| (self.best_move, o, self.depth))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryResult {
    pub best_move: Move,
    pub depth: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Outcome;
    use atomic_movegen::types::{Move, Square};

    #[test]
    fn result_for_matches_expected_outcome() {
        let entry = TtEntry {
            best_move: Move::make_move(Square::E2, Square::E4),
            outcome: Some(Outcome::Win),
            depth: 5,
            ..TtEntry::default()
        };

        assert!(entry.result_for(Outcome::Win).is_some());
        assert_eq!(entry.result_for(Outcome::Win).unwrap().depth, 5);
        assert!(entry.result_for(Outcome::Loss).is_none());
    }

    #[test]
    fn result_for_depth_requires_exact_depth() {
        let entry = TtEntry {
            best_move: Move::make_move(Square::E2, Square::E4),
            outcome: Some(Outcome::Win),
            depth: 5,
            ..TtEntry::default()
        };

        assert!(entry.result_for_depth(Outcome::Win, 5).is_some());
        assert!(entry.result_for_depth(Outcome::Win, 3).is_none());
        assert!(entry.result_for_depth(Outcome::Loss, 5).is_none());
    }

    #[test]
    fn best_result_returns_outcome_move_and_depth() {
        let mv = Move::make_move(Square::E2, Square::E4);
        let entry = TtEntry {
            best_move: mv,
            outcome: Some(Outcome::Win),
            depth: 7,
            ..TtEntry::default()
        };

        assert_eq!(entry.best_result(), Some((mv, Outcome::Win, 7)));

        let empty = TtEntry::default();
        assert_eq!(empty.best_result(), None);
    }
}
