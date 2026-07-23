//! DF-PN child evaluation and selection.

#![allow(clippy::similar_names)]

use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::zobrist;

use super::{INF, Search};

pub struct ChildInfo {
    pub mv: Move,
    pub pn: u64,
    pub dn: u64,
    pub outcome: Option<Outcome>,
    pub depth: u32,
    pub repetition_seen: bool,
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
    pub all_solved: bool,
    pub repetition_seen: bool,
}

impl Search {
    /// Evaluate every legal move and build a fresh `ChildInfo` table.
    ///
    /// When a child proves a win for the side to move and `refine_shortest` is
    /// disabled, the remaining siblings are not evaluated. Their entries are
    /// filled with neutral bounds so they do not affect the parent's numbers.
    pub(super) fn evaluate_all_children(
        &mut self,
        pos: &mut Position,
        moves: &MoveList,
        max_depth: u32,
        is_or_node: bool,
        refine_shortest: bool,
    ) -> Vec<ChildInfo> {
        let mut children = Vec::with_capacity(moves.len());
        for i in 0..moves.len() {
            let mv = moves[i];
            let info = self.evaluate_child(pos, mv, max_depth, is_or_node);
            let early_win = info.outcome == Some(Outcome::Loss);
            children.push(info);
            if early_win && !refine_shortest {
                for j in (i + 1)..moves.len() {
                    children.push(ChildInfo {
                        mv: moves[j],
                        pn: INF,
                        dn: 0,
                        outcome: None,
                        depth: 0,
                        repetition_seen: false,
                    });
                }
                break;
            }
        }
        children
    }

    /// Compute the parent's proof/disproof numbers and pick the best/second
    /// unsolved child from a cached `ChildInfo` table.
    ///
    /// `previous_best_move` and `previous_best_child` are hints from the
    /// transposition table. If the stored child is still valid and still the
    /// most-proving child, it is reused without recomputing the full argmin.
    pub(super) fn select_from_children(
        children: &[ChildInfo],
        is_or_node: bool,
        refine_shortest: bool,
        previous_best_move: Option<Move>,
        previous_best_child: Option<u8>,
    ) -> ChildSelection {
        let solved = Self::is_solved_by_children(children, is_or_node);

        // If a win is already proven and we do not need the shortest PV,
        // there is no reason to order or expand the remaining siblings.
        if let Some(selection) =
            Self::select_child_with_early_exit(children, solved, refine_shortest)
        {
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

        let mut pn;
        let mut dn;
        if is_or_node {
            pn = INF;
            dn = 0;
            for c in children {
                pn = std::cmp::min(pn, c.pn);
                dn = std::cmp::min(INF, dn.saturating_add(c.dn));
            }
        } else {
            pn = 0;
            dn = INF;
            for c in children {
                pn = std::cmp::min(INF, pn.saturating_add(c.pn));
                dn = std::cmp::min(dn, c.dn);
            }
        }

        // Choose the child to expand from the unsolved children only.
        let (best_idx, second_idx) = Self::best_and_second_unsolved(children, is_or_node);
        let best = best_idx.map(|i| &children[i]);
        let second = second_idx.map(|i| &children[i]);

        let best_child = best.map_or((Move::NONE, INF, INF), |b| (b.mv, b.pn, b.dn));
        let second_child = second.map_or((INF, INF), |s| (s.pn, s.dn));

        let best_move = if let Some((_, _, mv, _, _)) = solved {
            mv
        } else {
            best_idx.map_or(Move::NONE, |i| children[i].mv)
        };

        let depth = solved.map_or(0, |(_, d, _, _, _)| d);
        let all_solved = solved.is_some_and(|(_, _, _, all, _)| all);

        let repetition_seen = if let Some((outcome, _, _, _, idx)) = solved {
            match outcome {
                Outcome::Win | Outcome::Draw => children[idx].repetition_seen,
                Outcome::Loss => children.iter().any(|c| c.repetition_seen),
            }
        } else {
            children.iter().any(|c| c.repetition_seen)
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
            all_solved,
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
            all_solved: false,
            repetition_seen: children.iter().any(|c| c.repetition_seen),
        }
    }

    fn second_best_unsolved_excluding(
        children: &[ChildInfo],
        is_or_node: bool,
        exclude: usize,
    ) -> Option<usize> {
        let mut best: Option<usize> = None;
        for i in 0..children.len() {
            if i == exclude || children[i].outcome.is_some() {
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
        let child_path_code = self.path_code ^ zobrist::path_random(mv, self.path_stack.len());

        let info = if let Some(outcome) = pos.outcome() {
            let (pn, dn) = outcome.pn_dn_for(child_is_or);
            ChildInfo {
                mv,
                pn,
                dn,
                outcome: Some(outcome),
                depth: 0,
                repetition_seen: false,
            }
        } else if self.path_stack.contains(&child_rep_key) {
            let (pn, dn) = Outcome::Draw.pn_dn_for(child_is_or);
            ChildInfo {
                mv,
                pn,
                dn,
                outcome: Some(Outcome::Draw),
                depth: 0,
                repetition_seen: true,
            }
        } else {
            let child_max_depth = max_depth.saturating_sub(1);
            let child_path_length = self.path_stack.len() as u32;
            if let Some(resolved) = self.try_use_tt(
                pos,
                child_key,
                child_max_depth,
                child_path_code,
                child_path_length,
            ) {
                let (pn, dn) = resolved.outcome.pn_dn_for(child_is_or);
                ChildInfo {
                    mv,
                    pn,
                    dn,
                    outcome: Some(resolved.outcome),
                    depth: resolved.depth,
                    repetition_seen: resolved.repetition_seen,
                }
            } else if let Some(summary) = self.tt.probe_summary(child_key) {
                // Only reuse unsolved bounds from a search that was bounded by at
                // most `child_max_depth` plies.  Bounds from a deeper or work-
                // bounded search may be inconsistent for this shallower probe, so
                // start with the neutral (1, 1) values instead.
                let use_as_unsolved = summary.outcome.is_none()
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
                    repetition_seen: summary.repetition_seen,
                }
            } else {
                ChildInfo {
                    mv,
                    pn: 1,
                    dn: 1,
                    outcome: None,
                    depth: 0,
                    repetition_seen: false,
                }
            }
        };

        pos.undo_move(mv);
        info
    }
}
