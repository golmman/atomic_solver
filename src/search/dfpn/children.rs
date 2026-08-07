//! DF-PN child evaluation and selection.

#![allow(clippy::similar_names)]

use atomic_movegen::types::{Move, MoveList};

use crate::notation::move_to_uci;
use crate::position::{Outcome, Position};
use crate::proof_tree::{NodeProven, ProofMessage};

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

        if self.proof_tree_sender.is_some()
            && let Some(outcome) = info.outcome
            && outcome != Outcome::Draw
        {
            let uci = move_to_uci(mv);
            let path = format!("{}.{}", self.proof_path, uci);
            if let Some(sender) = &self.proof_tree_sender {
                let _ = sender.send(ProofMessage::NodeProven(NodeProven {
                    path,
                    mv,
                    outcome,
                    depth: info.depth,
                }));
            }
        }

        pos.undo_move(mv);
        info
    }

    /// Compute the parent's proof/disproof numbers and pick the best/second
    /// unsolved child from a cached `ChildInfo` table.
    ///
    /// `previous_best_move` and `previous_best_child` are hints from the
    /// transposition table. If the stored child is still valid and still the
    /// most-proving child, it is reused without recomputing the full argmin.
    pub(super) fn select_from_children(
        &self,
        children: &[ChildInfo],
        is_or_node: bool,
        previous_best_move: Option<Move>,
        previous_best_child: Option<u8>,
    ) -> ChildSelection {
        let solved = Self::is_solved_by_children(children, is_or_node);

        // A single winning child is enough to prove a Win. For Loss and Draw
        // we need all children to be solved.
        if let Some(selection) = Self::select_child_with_early_exit(children, solved) {
            return selection;
        }

        // Reuse the previous best child if it is still valid and still the best.
        if let Some(prev_mv) = previous_best_move
            && let Some(idx) = previous_best_child
                .filter(|&c| (c as usize) < children.len())
                .map(|c| c as usize)
                .filter(|&i| children[i].mv == prev_mv)
                .or_else(|| children.iter().position(|c| c.mv == prev_mv))
            && children[idx].outcome.is_none()
            && !children[idx].explored
        {
            let is_still_best = if is_or_node {
                children
                    .iter()
                    .filter(|c| c.outcome.is_none())
                    .all(|c| c.pn >= children[idx].pn)
            } else {
                children
                    .iter()
                    .filter(|c| c.outcome.is_none())
                    .all(|c| c.dn >= children[idx].dn)
            };
            if is_still_best {
                return Self::selection_for_child(children, is_or_node, idx);
            }
        }

        // Compute proof/disproof numbers from all children.
        let (mut pn, mut dn) = if is_or_node { (INF, 0) } else { (0, INF) };
        if is_or_node {
            for c in children {
                pn = std::cmp::min(pn, c.pn);
                dn = std::cmp::min(INF, dn.saturating_add(c.dn));
            }
        } else {
            for c in children {
                pn = std::cmp::min(INF, pn.saturating_add(c.pn));
                dn = std::cmp::min(dn, c.dn);
            }
        }

        let (best_idx, second_idx) = Self::best_and_second_unsolved(children, is_or_node);
        let best = best_idx.map(|i| &children[i]);
        let second = second_idx.map(|i| &children[i]);

        let best_child = best.map_or((Move::NONE, INF, INF), |b| (b.mv, b.pn, b.dn));
        let second_child = second.map_or((INF, INF), |s| (s.pn, s.dn));

        let best_move = solved.as_ref().map_or_else(
            || best_idx.map_or(Move::NONE, |i| children[i].mv),
            |(_, _, mv, _, _)| *mv,
        );

        let depth = solved.as_ref().map_or(0, |(_, d, _, _, _)| *d);

        // Win and Loss cannot depend on a repetition. Draw may, so use the
        // selected draw child's flag.
        let repetition_seen = if let Some((outcome, _, _, _, idx)) = solved {
            matches!(outcome, Outcome::Draw) && children[idx].repetition_seen
        } else {
            false
        };

        ChildSelection {
            best_child,
            second_child,
            best_child_index: best_idx,
            pn,
            dn,
            depth,
            best_move,
            solved_outcome: solved.map(|(o, _, _, _, _)| o),
            repetition_seen,
        }
    }

    fn selection_for_child(children: &[ChildInfo], is_or_node: bool, idx: usize) -> ChildSelection {
        let (mut pn, mut dn) = if is_or_node { (INF, 0) } else { (0, INF) };
        for c in children {
            if is_or_node {
                pn = std::cmp::min(pn, c.pn);
                dn = std::cmp::min(INF, dn.saturating_add(c.dn));
            } else {
                pn = std::cmp::min(INF, pn.saturating_add(c.pn));
                dn = std::cmp::min(dn, c.dn);
            }
        }

        let second_idx = Self::second_best_unsolved_excluding(children, is_or_node, idx);
        let best = &children[idx];
        let second = second_idx.map(|i| &children[i]);

        ChildSelection {
            best_child: (best.mv, best.pn, best.dn),
            second_child: second.map_or((INF, INF), |s| (s.pn, s.dn)),
            best_child_index: Some(idx),
            pn,
            dn,
            depth: 0,
            best_move: best.mv,
            solved_outcome: None,
            repetition_seen: false,
        }
    }

    fn second_best_unsolved_excluding(
        children: &[ChildInfo],
        is_or_node: bool,
        exclude: usize,
    ) -> Option<usize> {
        let mut best: Option<usize> = None;
        for i in 0..children.len() {
            if i == exclude || children[i].outcome.is_some() || children[i].explored {
                continue;
            }
            let cmp_c = if is_or_node {
                children[i].pn
            } else {
                children[i].dn
            };
            match best {
                None => best = Some(i),
                Some(b) => {
                    let cmp_best = if is_or_node {
                        children[b].pn
                    } else {
                        children[b].dn
                    };
                    if cmp_c < cmp_best {
                        best = Some(i);
                    }
                }
            }
        }
        best
    }
}
