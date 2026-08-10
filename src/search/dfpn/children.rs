//! DF-PN child evaluation.
//!
//! This file is larger than 10 KiB because `ChildInfo` construction, terminal
//! detection, TT reuse, and proof-tree event emission for every child all share
//! the same `Position` move/undo sequence.

#![allow(clippy::similar_names)]

use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::proof_event::{NodeProven, ProofEvent};

use super::{INF, Search};

pub struct ChildInfo {
    pub mv: Move,
    pub pn: u64,
    pub dn: u64,
    pub outcome: Option<Outcome>,
    pub depth: u32,
    pub repetition_seen: bool,
    pub explored: bool,
}

#[derive(Clone, Copy)]
pub struct ChildSelection {
    pub best_child: (Move, u64, u64),
    pub second_child: (u64, u64),
    pub best_child_index: Option<usize>,
    pub pn: u64,
    pub dn: u64,
    pub depth: u32,
    pub best_move: Move,
    pub solved_outcome: Option<Outcome>,
    pub repetition_seen: bool,
}

impl Search {
    /// Evaluate every legal move and build a fresh `ChildInfo` table.
    ///
    /// A single child `Loss` is enough to prove the parent is a win for the
    /// side to move, so we can stop evaluating the remaining children once a
    /// winning child is found. A Loss parent requires all children to be solved.
    pub(super) fn evaluate_all_children(
        &mut self,
        pos: &mut Position,
        moves: &MoveList,
        max_depth: u32,
        is_or_node: bool,
    ) -> Vec<ChildInfo> {
        let mut children = Vec::with_capacity(moves.len());
        for i in 0..moves.len() {
            let mv = moves[i];
            let info = self.evaluate_child(pos, mv, max_depth, is_or_node);
            let decisive = info.outcome == Some(Outcome::Loss);
            children.push(info);
            if decisive {
                for j in (i + 1)..moves.len() {
                    children.push(ChildInfo {
                        mv: moves[j],
                        pn: INF,
                        dn: 0,
                        outcome: None,
                        depth: 0,
                        repetition_seen: false,
                        explored: false,
                    });
                }
                break;
            }
        }
        children
    }

    pub(super) fn evaluate_child(
        &mut self,
        pos: &mut Position,
        mv: Move,
        max_depth: u32,
        is_or_node: bool,
    ) -> ChildInfo {
        self.child_evals += 1;
        pos.do_move(mv);
        let child_key = pos.hash();
        let child_rep_key = pos.repetition_key();
        let child_is_or = !is_or_node;

        let info = if let Some(outcome) = pos.outcome() {
            let (pn, dn) = outcome.pn_dn_for(child_is_or);
            ChildInfo {
                mv,
                pn,
                dn,
                outcome: Some(outcome),
                depth: 0,
                repetition_seen: false,
                explored: false,
            }
        } else if self.path_contains(child_rep_key) {
            let (pn, dn) = Outcome::Draw.pn_dn_for(child_is_or);
            ChildInfo {
                mv,
                pn,
                dn,
                outcome: Some(Outcome::Draw),
                depth: 0,
                repetition_seen: true,
                explored: false,
            }
        } else {
            let child_max_depth = max_depth.saturating_sub(1);
            if let Some(resolved) = self.try_use_tt(pos, child_key, child_max_depth) {
                let (pn, dn) = resolved.outcome.pn_dn_for(child_is_or);
                ChildInfo {
                    mv,
                    pn,
                    dn,
                    outcome: Some(resolved.outcome),
                    depth: resolved.depth,
                    repetition_seen: false,
                    explored: false,
                }
            } else if let Some(summary) = self.tt.probe_summary(child_key) {
                // Only reuse unsolved bounds when they are non-degenerate.  A
                // previous work-bounded search may have stored a candidate
                // terminal-like bound (pn == 0 or dn == 0) without an outcome,
                // and propagating such values can trick the parent search into
                // treating an unproven node as solved.  Fall back to neutral
                // (1, 1) in those cases.
                let use_as_unsolved = summary.outcome.is_none()
                    && summary.pn > 0
                    && summary.dn > 0
                    && summary.remaining_depth != u32::MAX
                    && summary.remaining_depth <= child_max_depth;
                let (pn, dn) = if use_as_unsolved {
                    (summary.pn, summary.dn)
                } else {
                    (1, 1)
                };
                ChildInfo {
                    mv,
                    pn,
                    dn,
                    outcome: None,
                    depth: 0,
                    repetition_seen: false,
                    explored: false,
                }
            } else {
                ChildInfo {
                    mv,
                    pn: 1,
                    dn: 1,
                    outcome: None,
                    depth: 0,
                    repetition_seen: false,
                    explored: false,
                }
            }
        };

        if let Some(sender) = &self.proof_event_sender
            && let Some(outcome) = info.outcome
            && outcome != Outcome::Draw
        {
            let mut path = self.move_stack.clone();
            path.push(mv);
            let _ = sender.send(ProofEvent::NodeProven(NodeProven::new(
                path,
                pos.hash(),
                outcome,
                info.depth,
            )));
        }

        pos.undo_move(mv);
        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;
    use atomic_movegen::types::Square;

    #[test]
    fn evaluate_child_terminal_win_for_parent() {
        // White rook can capture the black commoner on e8.
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        let mv = Move::make_move(Square::E1, Square::E8);
        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);

        assert_eq!(
            info.outcome,
            Some(Outcome::Loss),
            "capturing the commoner ends the game"
        );
        assert_eq!(
            info.pn, 0,
            "terminal winning child has pn 0 at the AND side"
        );
        assert_eq!(
            info.dn, INF,
            "terminal winning child has dn INF at the AND side"
        );
    }

    #[test]
    fn evaluate_child_repetition_is_draw() {
        let mut search = Search::new(1);
        // Build a non-terminal position whose move leads to a position that is
        // already on the search path by pushing the child's repetition key before
        // evaluating it.
        let mut pos = Position::from_fen("4k3/8/8/4K3/8/8/8/4R3 w - - 0 1").unwrap();

        // Move the white king from e5 to d5; it is quiet and non-terminal.
        let mv = Move::make_move(Square::E5, Square::D5);
        pos.do_move(mv);
        let child_rep_key = pos.repetition_key();
        pos.undo_move(mv);
        search.path_stack.push(child_rep_key);

        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);

        assert_eq!(
            info.outcome,
            Some(Outcome::Draw),
            "repeating position is a draw"
        );
        assert!(info.repetition_seen, "repetition flag should be set");
    }

    #[test]
    fn evaluate_child_uses_solved_tt_entry() {
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
        // g1h1 is a quiet rook move to a non-terminal child.
        let mv = Move::make_move(Square::G1, Square::H1);
        pos.do_move(mv);
        let child_key = pos.hash();
        pos.undo_move(mv);

        // Store a solved Loss for the child position. The remaining-depth guard
        // requires remaining_depth == u32::MAX for an unbounded max_depth.
        search.tt.store(
            child_key,
            Move::NONE,
            u8::MAX,
            0,
            Some(Outcome::Loss),
            INF,
            0,
            1,
            u32::MAX,
        );

        // Force the TT lookup without any legal-move generation at the child.
        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
        assert_eq!(
            info.outcome,
            Some(Outcome::Loss),
            "TT-solved Loss should be reused"
        );
        assert_eq!(info.depth, 1, "TT depth should be preserved");
    }

    #[test]
    fn evaluate_all_children_stops_at_winning_child() {
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);

        // Sort so the winning capture e1e8 is tried first.
        search.sort_moves(&pos, &mut moves, Move::NONE, true);
        let children = search.evaluate_all_children(&mut pos, &moves, u32::MAX, true);

        assert_eq!(children.len(), moves.len());
        assert!(
            children[0].outcome == Some(Outcome::Loss),
            "first child should be the winning capture"
        );

        // Remaining children were filled with dummy unexplored entries.
        for c in &children[1..] {
            assert_eq!(c.pn, INF);
            assert_eq!(c.dn, 0);
            assert_eq!(c.outcome, None);
            assert!(!c.explored);
        }
    }

    #[test]
    fn evaluate_child_unsolved_tt_bounds_used_when_non_degenerate() {
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
        // g1h1 is a quiet rook move to a non-terminal child.
        let mv = Move::make_move(Square::G1, Square::H1);
        pos.do_move(mv);
        let child_key = pos.hash();
        pos.undo_move(mv);

        // Store non-degenerate unsolved bounds with enough remaining depth.
        // max_depth=11 gives child_max_depth=10, so remaining_depth=10 is usable.
        search
            .tt
            .store(child_key, Move::NONE, u8::MAX, 1, None, 5, 5, 0, 10);

        let info = search.evaluate_child(&mut pos, mv, 11, true);
        assert_eq!(
            info.outcome, None,
            "unsolved bounds should not report an outcome"
        );
        assert_eq!(info.pn, 5);
        assert_eq!(info.dn, 5);
    }

    #[test]
    fn evaluate_child_degenerate_tt_bounds_fall_back_to_neutral() {
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
        // g1h1 is a quiet rook move to a non-terminal child.
        let mv = Move::make_move(Square::G1, Square::H1);
        pos.do_move(mv);
        let child_key = pos.hash();
        pos.undo_move(mv);

        // pn == 0 is degenerate and should not be reused as an unsolved bound.
        search
            .tt
            .store(child_key, Move::NONE, u8::MAX, 1, None, 0, 5, 0, 10);

        let info = search.evaluate_child(&mut pos, mv, 11, true);
        assert_eq!(info.outcome, None);
        assert_eq!(info.pn, 1);
        assert_eq!(info.dn, 1);
    }
}
