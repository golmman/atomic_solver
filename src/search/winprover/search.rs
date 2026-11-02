//! Win-only DF-PN+ loop.
//!
//! This is a deliberately small sibling of `search::dfpn::core`: the same
//! AND/OR proof-number recurrence and `1+ε` thresholds, without the general
//! solver's refinement rounds, pre-phase, cheap probe, repetition cache,
//! history/killer learning, or opt-in heuristics. It returns the side-to-move
//! outcome of one frame; [`super::WinProver::solve`] only turns a root `Win`
//! into a claim, and anything else falls back to the general search.
//!
//! This file is slightly larger than the 10 KiB guideline because the frame
//! loop, the AND/OR selection, and the threshold arithmetic share the
//! `ChildInfo` table; splitting them would separate the recurrence from the
//! solved detection that feeds it.
//!
//! The minimax rule is the general one (the side to move always chooses):
//! `Win` if any child is `Loss`, `Loss` if all children are `Win`, `Draw`
//! otherwise. The plan's AND-node bullet ("`Win` if all children are `Win`")
//! is the same statement read from the root attacker's perspective; using it
//! literally as a side-to-move rule would be unsound, so the shipped rule is
//! the standard one shared with `dfpn`.

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::zobrist::INF;

use super::WinProver;
use super::children::ChildInfo;

/// The parent's selected child state for one loop iteration.
struct Selection {
    /// Set when the children already determine the parent's outcome.
    solved: Option<(Outcome, u32, Move)>,
    best_idx: Option<usize>,
    pn: u64,
    dn: u64,
    depth: u32,
    best_move: Move,
    second_pn: u64,
    second_dn: u64,
}

impl WinProver {
    /// One DF-PN frame. `is_or_node` is true when the side to move is the
    /// root attacker; `th_pn`/`th_dn` are the inherited threshold bounds
    /// (`INF` at the root).
    pub(super) fn dfpn(
        &mut self,
        pos: &mut Position,
        th_pn: u64,
        th_dn: u64,
        is_or_node: bool,
    ) -> Outcome {
        if self.stop() {
            return Outcome::Draw;
        }
        self.nodes += 1;

        let tt_key = pos.hash();
        let rep_key = pos.repetition_key();

        let mut moves = MoveList::new();
        let mut state = StateInfo::new();
        pos.legal_moves_with_state(&mut moves, &mut state);

        if let Some(outcome) = pos.outcome_from_state(&state, &moves) {
            if outcome != Outcome::Draw {
                let (pn, dn) = outcome.pn_dn_for(is_or_node);
                self.tt.store(
                    tt_key,
                    Move::NONE,
                    u8::MAX,
                    0,
                    Some(outcome),
                    pn,
                    dn,
                    0,
                    u32::MAX,
                );
            }
            self.emit_proof_node(pos, outcome, 0);
            return outcome;
        }

        // Repetition: the board is already on the current path, so this is a
        // Draw under the solver's semantics — never a win, never TT-stored.
        if self.path_contains(rep_key) {
            return Outcome::Draw;
        }

        let entry = self.tt.probe(tt_key).copied();
        if let Some(entry) = entry.as_ref()
            && let Some(outcome) = entry.outcome
            && outcome != Outcome::Draw
        {
            self.emit_proof_node(pos, outcome, entry.depth);
            return outcome;
        }

        let best_from_tt = entry.as_ref().map_or(Move::NONE, |e| e.best_move);
        self.sort_moves(pos, &state, &mut moves, best_from_tt, is_or_node);

        self.path_push(rep_key);
        let evals_start = self.child_evals;

        let mut children: Vec<ChildInfo> = Vec::new();
        self.evaluate_all_children(pos, &moves, is_or_node, &mut children);

        let mut best_move = Move::NONE;
        let mut pn = INF;
        let mut dn = INF;
        let mut depth = 0;
        let mut solved: Option<(Outcome, u32, Move)> = None;

        loop {
            if self.stop() {
                break;
            }
            let selection = select(&children, is_or_node);
            pn = selection.pn;
            dn = selection.dn;
            depth = selection.depth;
            best_move = selection.best_move;

            if let Some(found) = selection.solved {
                solved = Some(found);
                break;
            }
            if (th_pn != INF && pn >= th_pn) || (th_dn != INF && dn >= th_dn) {
                break;
            }
            let Some(idx) = selection.best_idx else {
                break;
            };
            let mv = children[idx].mv;
            let child_pn = children[idx].pn;
            let child_dn = children[idx].dn;

            let (next_pn, next_dn) = if is_or_node {
                let next_pn = th_pn.min(self.epsilon_ceil(selection.second_pn));
                let next_dn = if th_dn == INF {
                    INF
                } else {
                    th_dn.saturating_sub(dn).saturating_add(child_dn)
                };
                (next_pn, next_dn)
            } else {
                let next_dn = th_dn.min(self.epsilon_ceil(selection.second_dn));
                let next_pn = if th_pn == INF {
                    INF
                } else {
                    th_pn.saturating_sub(pn).saturating_add(child_pn)
                };
                (next_pn, next_dn)
            };

            pos.do_move(mv);
            self.move_stack.push(mv);
            let _ = self.dfpn(pos, next_pn, next_dn, !is_or_node);
            self.move_stack.pop();
            pos.undo_move(mv);

            children[idx] = self.evaluate_child(pos, mv, is_or_node);
        }

        let work = self.child_evals - evals_start;
        let store_outcome = solved.and_then(|(outcome, _, _)| {
            if outcome == Outcome::Draw {
                None
            } else {
                Some(outcome)
            }
        });
        let (store_pn, store_dn) = if let Some(outcome) = store_outcome {
            let (pn, dn) = outcome.pn_dn_for(is_or_node);
            (pn, dn)
        } else if solved.is_some() {
            // A proven Draw is path-dependent only for repetition draws and is
            // never stored; keep the entry reusable-unsolved instead.
            (1, 1)
        } else {
            (pn.clamp(1, INF), dn.clamp(1, INF))
        };
        let store_move = solved.map_or(best_move, |(_, _, mv)| mv);
        let store_depth = solved.map_or(depth, |(_, depth, _)| depth);
        self.tt.store(
            tt_key,
            store_move,
            u8::MAX,
            work,
            store_outcome,
            store_pn,
            store_dn,
            store_depth,
            u32::MAX,
        );

        self.path_pop();

        if let Some((outcome, depth, _)) = solved {
            self.emit_proof_node(pos, outcome, depth);
            outcome
        } else {
            Outcome::Draw
        }
    }

    /// `⌈x · (1+ε)⌉`, clamped to one above `x` and at [`INF`]. Identical to
    /// the general solver's threshold helper.
    pub(super) fn epsilon_ceil(&self, x: u64) -> u64 {
        if x >= INF {
            return INF;
        }
        let scaled = (u128::from(x) * u128::from(self.epsilon_num))
            .div_ceil(u128::from(self.epsilon_den)) as u64;
        scaled.max(x.saturating_add(1)).min(INF)
    }
}

/// Select the solved outcome (if any), the proof/disproof numbers, and the
/// best/second unsolved children of a frame.
fn select(children: &[ChildInfo], is_or_node: bool) -> Selection {
    let mut all_solved = !children.is_empty();
    let mut win_idx: Option<usize> = None;
    let mut win_depth = u32::MAX;
    let mut loss_idx: Option<usize> = None;
    let mut loss_depth = 0u32;
    let mut draw_idx: Option<usize> = None;
    let mut draw_depth = 0u32;
    let mut found_draw = false;

    for (i, child) in children.iter().enumerate() {
        let depth = child.depth.saturating_add(1);
        match child.outcome {
            None => all_solved = false,
            Some(Outcome::Loss) => {
                if depth < win_depth {
                    win_depth = depth;
                    win_idx = Some(i);
                }
            }
            Some(Outcome::Draw) => {
                found_draw = true;
                if depth > draw_depth {
                    draw_depth = depth;
                    draw_idx = Some(i);
                }
            }
            Some(Outcome::Win) => {
                if depth > loss_depth {
                    loss_depth = depth;
                    loss_idx = Some(i);
                }
            }
        }
    }

    let solved = if let Some(idx) = win_idx {
        Some((Outcome::Win, win_depth, children[idx].mv))
    } else if all_solved {
        if found_draw {
            let idx = draw_idx.unwrap_or(0);
            Some((Outcome::Draw, draw_depth, children[idx].mv))
        } else {
            let idx = loss_idx.unwrap_or(0);
            Some((Outcome::Loss, loss_depth, children[idx].mv))
        }
    } else {
        None
    };

    let (mut pn, mut dn) = if is_or_node { (INF, 0) } else { (0, INF) };
    for child in children {
        if is_or_node {
            pn = pn.min(child.pn);
            dn = dn.saturating_add(child.dn).min(INF);
        } else {
            pn = pn.saturating_add(child.pn).min(INF);
            dn = dn.min(child.dn);
        }
    }

    let mut best_idx: Option<usize> = None;
    let mut second_idx: Option<usize> = None;
    for i in 0..children.len() {
        if children[i].outcome.is_some() {
            continue;
        }
        let key = if is_or_node {
            children[i].pn
        } else {
            children[i].dn
        };
        match best_idx {
            None => best_idx = Some(i),
            Some(best) => {
                let best_key = if is_or_node {
                    children[best].pn
                } else {
                    children[best].dn
                };
                if key < best_key {
                    second_idx = best_idx;
                    best_idx = Some(i);
                } else {
                    match second_idx {
                        None => second_idx = Some(i),
                        Some(second) => {
                            let second_key = if is_or_node {
                                children[second].pn
                            } else {
                                children[second].dn
                            };
                            if key < second_key {
                                second_idx = Some(i);
                            }
                        }
                    }
                }
            }
        }
    }

    let best_move = solved.map_or_else(
        || best_idx.map_or(Move::NONE, |i| children[i].mv),
        |(_, _, mv)| mv,
    );
    let depth = solved.map_or(0, |(_, depth, _)| depth);
    let (second_pn, second_dn) =
        second_idx.map_or((INF, INF), |i| (children[i].pn, children[i].dn));

    Selection {
        solved,
        best_idx,
        pn,
        dn,
        depth,
        best_move,
        second_pn,
        second_dn,
    }
}
