//! Win-only front-end child evaluation and move ordering.
//!
//! One child of a DF-PN frame is classified by playing its move and asking the
//! shipped primitives: `Position::outcome()` (terminals, rule50, bare kings),
//! the front-end repetition path (a repeated position is a Draw, never a win),
//! and the front-end's scratch transposition table (solved `Win`/`Loss`
//! outcomes and reusable unsolved proof/disproof bounds).
//!
//! Only path-independent results are stored: terminal `Win`/`Loss` and
//! unsolved `(pn, dn)` bounds. Terminal Draw results are kept in the frame's
//! child table (they are needed for correct minimax) but never stored in the
//! TT, and a repetition-dependent Draw is neither stored nor otherwise
//! remembered. See the module header of [`super`] for the soundness
//! contract.

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};

use super::WinProver;
use crate::position::{Outcome, Position};
use crate::search::ordering::{ScoreContext, nearest_commoner_map};

/// One evaluated child of a frame.
#[derive(Clone, Copy, Debug)]
pub(super) struct ChildInfo {
    pub mv: Move,
    pub pn: u64,
    pub dn: u64,
    /// The child's outcome from *its* side-to-move perspective. Terminal
    /// Draws are included locally even though they are never TT-stored.
    pub outcome: Option<Outcome>,
    /// Proven depth below this child (0 while unsolved).
    pub depth: u32,
}

impl ChildInfo {
    /// A child whose outcome is decided. `child_is_or` is the *child's* node
    /// kind (the parent's `!is_or_node`), which fixes the `pn`/`dn` polarity
    /// relative to the root attacker.
    fn solved(mv: Move, outcome: Outcome, depth: u32, child_is_or: bool) -> Self {
        let (pn, dn) = outcome.pn_dn_for(child_is_or);
        Self {
            mv,
            pn,
            dn,
            outcome: Some(outcome),
            depth,
        }
    }

    /// A non-terminal child carrying TT bounds, or the neutral `(1, 1)`.
    fn unsolved(mv: Move, pn: u64, dn: u64) -> Self {
        Self {
            mv,
            pn,
            dn,
            outcome: None,
            depth: 0,
        }
    }
}

impl WinProver {
    /// Evaluate every legal move of `pos` into `children`, in the scored
    /// order supplied by [`WinProver::sort_moves`].
    ///
    /// The whole sweep is always evaluated: the child-eval budget is checked
    /// at frame-loop heads (and may be crossed by at most this one sweep), so
    /// a partial table can never make an AND node look solved.
    pub(super) fn evaluate_all_children(
        &mut self,
        pos: &mut Position,
        moves: &MoveList,
        is_or_node: bool,
        children: &mut Vec<ChildInfo>,
    ) {
        children.clear();
        children.reserve(moves.len());
        for &mv in moves.as_slice() {
            children.push(self.evaluate_child(pos, mv, is_or_node));
        }
    }

    /// Evaluate one child by playing `mv`, classifying the resulting
    /// position, and undoing the move.
    pub(super) fn evaluate_child(
        &mut self,
        pos: &mut Position,
        mv: Move,
        is_or_node: bool,
    ) -> ChildInfo {
        self.child_evals += 1;
        let child_is_or = !is_or_node;
        pos.do_move(mv);
        let child = self.classify_child(pos, mv, child_is_or);
        pos.undo_move(mv);
        child
    }

    fn classify_child(&self, pos: &Position, mv: Move, child_is_or: bool) -> ChildInfo {
        if let Some(outcome) = pos.outcome() {
            return ChildInfo::solved(mv, outcome, 0, child_is_or);
        }
        if self.path_contains(pos.repetition_key()) {
            // A repetition is a Draw for the side to move. It is recorded in
            // the frame table but can never enter the TT (it depends on the
            // current path).
            return ChildInfo::solved(mv, Outcome::Draw, 0, child_is_or);
        }
        if let Some(entry) = self.tt.probe(pos.hash()) {
            if let Some(outcome) = entry.outcome {
                if outcome != Outcome::Draw {
                    return ChildInfo::solved(mv, outcome, entry.depth, child_is_or);
                }
            } else if entry.pn > 0 && entry.dn > 0 {
                return ChildInfo::unsolved(mv, entry.pn, entry.dn);
            }
        }
        ChildInfo::unsolved(mv, 1, 1)
    }

    /// Order the frame's legal moves by the shipped static scorer (no
    /// history/killer/quiet-group/move-class machinery), moving the TT's
    /// best-move hint to the front when present.
    pub(super) fn sort_moves(
        &self,
        pos: &Position,
        state: &StateInfo,
        moves: &mut MoveList,
        best_from_tt: Move,
        is_or_node: bool,
    ) {
        let board = pos.board();
        let nearest = nearest_commoner_map(board, board.side_to_move().flip());
        let ctx = ScoreContext::build(board, state, nearest);
        let slice = moves.as_mut_slice();
        let mut scored: Vec<(Move, i32)> = slice
            .iter()
            .copied()
            .map(|m| {
                (
                    m,
                    self.scorer.score_with_context(board, m, &ctx, is_or_node),
                )
            })
            .collect();
        scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        if best_from_tt != Move::NONE
            && let Some(idx) = scored.iter().position(|&(m, _)| m == best_from_tt)
        {
            let entry = scored[idx];
            for i in (0..idx).rev() {
                scored[i + 1] = scored[i];
            }
            scored[0] = entry;
        }
        for (i, &(m, _)) in scored.iter().enumerate() {
            slice[i] = m;
        }
    }
}
