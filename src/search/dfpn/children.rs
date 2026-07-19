//! DF-PN child evaluation and selection.

use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::zobrist;

use super::{INF, Search};

pub struct ChildInfo {
    pub mv: Move,
    pub pn: u64,
    pub dn: u64,
    pub vpn: u64,
    pub vdn: u64,
    pub outcome: Option<Outcome>,
    pub depth: u32,
    pub repetition_seen: bool,
}

pub struct ChildSelection {
    pub best_child: (Move, u64, u64, u64, u64),
    pub second_child: (u64, u64),
    pub pn: u64,
    pub dn: u64,
    pub depth: u32,
    pub best_move: Move,
    pub solved_outcome: Option<Outcome>,
    pub all_solved: bool,
    pub repetition_seen: bool,
}

impl Search {
    pub(super) fn select_children(
        &mut self,
        pos: &mut Position,
        moves: &MoveList,
        max_depth: u32,
        is_or_node: bool,
    ) -> ChildSelection {
        let mut children = Vec::with_capacity(moves.len());
        for i in 0..moves.len() {
            let mv = moves[i];
            let info = self.evaluate_child(pos, mv, max_depth, is_or_node);
            children.push(info);
        }

        let mut pn;
        let mut dn;
        if is_or_node {
            pn = INF;
            dn = 0;
            for c in &children {
                pn = std::cmp::min(pn, c.pn);
                dn = std::cmp::min(INF, dn.saturating_add(c.dn));
            }
        } else {
            pn = 0;
            dn = INF;
            for c in &children {
                pn = std::cmp::min(INF, pn.saturating_add(c.pn));
                dn = std::cmp::min(dn, c.dn);
            }
        }

        let solved = Self::is_solved_by_children(&children, is_or_node);

        // Choose the child to expand from the unsolved children only.
        let (best_idx, second_idx) = Self::best_and_second_unsolved(&children, is_or_node);
        let best = best_idx.map(|i| &children[i]);
        let second = second_idx.map(|i| &children[i]);

        let best_child = best.map(|b| (b.mv, b.pn, b.dn, b.vpn, b.vdn)).unwrap_or((
            Move::NONE,
            INF,
            INF,
            INF,
            INF,
        ));
        let second_child = second.map(|s| (s.pn, s.dn)).unwrap_or((INF, INF));

        let best_move = if let Some((_, _, mv, _, _)) = solved {
            mv
        } else {
            best_idx.map(|i| children[i].mv).unwrap_or(Move::NONE)
        };

        let depth = solved.map(|(_, d, _, _, _)| d).unwrap_or(0);
        let all_solved = solved.map(|(_, _, _, all, _)| all).unwrap_or(false);

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
            pn,
            dn,
            depth,
            best_move,
            solved_outcome: solved.map(|(o, _, _, _, _)| o),
            all_solved,
            repetition_seen,
        }
    }

    fn evaluate_child(
        &mut self,
        pos: &mut Position,
        mv: Move,
        max_depth: u32,
        is_or_node: bool,
    ) -> ChildInfo {
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
                vpn: pn,
                vdn: dn,
                outcome: Some(outcome),
                depth: 0,
                repetition_seen: false,
            }
        } else if self.path.contains(&child_rep_key) {
            let (pn, dn) = Outcome::Draw.pn_dn_for(child_is_or);
            ChildInfo {
                mv,
                pn,
                dn,
                vpn: pn,
                vdn: dn,
                outcome: Some(Outcome::Draw),
                depth: 0,
                repetition_seen: true,
            }
        } else if let Some(entry) = self.tt.probe(child_key).copied() {
            let child_max_depth = max_depth.saturating_sub(1);
            let child_path_length = self.path_stack.len() as u32;
            if let Some(resolved) = self.try_use_tt(
                pos,
                &entry,
                child_max_depth,
                child_path_code,
                child_path_length,
            ) {
                let (pn, dn) = resolved.outcome.pn_dn_for(child_is_or);
                ChildInfo {
                    mv,
                    pn,
                    dn,
                    vpn: pn,
                    vdn: dn,
                    outcome: Some(resolved.outcome),
                    depth: resolved.depth,
                    repetition_seen: resolved.repetition_seen,
                }
            } else {
                let use_as_unsolved = entry.outcome.is_none() && entry.depth <= child_max_depth;
                let (pn, dn) = if use_as_unsolved {
                    (entry.pn, entry.dn)
                } else {
                    (1, 1)
                };
                ChildInfo {
                    mv,
                    pn,
                    dn,
                    vpn: pn,
                    vdn: dn,
                    outcome: None,
                    depth: 0,
                    repetition_seen: entry.repetition_seen,
                }
            }
        } else {
            ChildInfo {
                mv,
                pn: 1,
                dn: 1,
                vpn: 1,
                vdn: 1,
                outcome: None,
                depth: 0,
                repetition_seen: false,
            }
        };

        pos.undo_move(mv);
        info
    }
}
