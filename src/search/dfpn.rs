//! Sequential DF-PN+ solver for atomic chess.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::zobrist;

use super::ordering::{MoveScorer, StaticAtomicScorer};
use super::tt::TranspositionTable;

pub const INF: u64 = zobrist::INF;

const EPSILON: f64 = 0.25;
const TIMEOUT_SECS: u64 = 5;

pub struct Search {
    tt: TranspositionTable,
    path: HashSet<u64>,
    path_stack: Vec<u64>,
    path_code: u64,
    nodes: u64,
    start: Instant,
    deadline: Instant,
    epsilon: f64,
    scorer: Box<dyn MoveScorer>,
}

impl Search {
    pub fn new(tt_mb: usize) -> Self {
        Self {
            tt: TranspositionTable::with_mb(tt_mb),
            path: HashSet::new(),
            path_stack: Vec::new(),
            path_code: 0,
            nodes: 0,
            start: Instant::now(),
            deadline: Instant::now(),
            epsilon: EPSILON,
            scorer: Box::new(StaticAtomicScorer),
        }
    }

    pub fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        self.nodes = 0;
        self.start = Instant::now();
        self.deadline = self.start + Duration::from_secs(TIMEOUT_SECS);
        self.path.clear();
        self.path_stack.clear();
        self.path_code = 0;

        let outcome = self.dfpn(pos, INF, INF, true);
        let pv = self.extract_pv(pos);
        (outcome, pv, self.nodes)
    }

    fn dfpn(&mut self, pos: &mut Position, th_pn: u64, th_dn: u64, is_or_node: bool) -> Outcome {
        if Instant::now() >= self.deadline {
            return Outcome::Draw;
        }

        self.nodes += 1;

        let key = pos.hash();

        if let Some(outcome) = pos.outcome() {
            let (pn, dn) = outcome.pn_dn_for(is_or_node);
            self.tt.store(
                key,
                Move::NONE,
                Some(outcome),
                pn,
                dn,
                self.path_code,
                false,
            );
            return outcome;
        }

        if let Some(entry) = self.tt.probe(key)
            && let Some(outcome) = self.try_use_tt(entry, self.path_code)
        {
            return outcome;
        }

        if !self.path.insert(key) {
            return Outcome::Draw;
        }

        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);

        if moves.is_empty() {
            self.path.remove(&key);
            let (pn, dn) = Outcome::Draw.pn_dn_for(is_or_node);
            self.tt.store(
                key,
                Move::NONE,
                Some(Outcome::Draw),
                pn,
                dn,
                self.path_code,
                false,
            );
            return Outcome::Draw;
        }

        let best_from_tt = self
            .tt
            .probe(key)
            .map(|e| e.best_move)
            .unwrap_or(Move::NONE);
        self.sort_moves(pos, &mut moves, best_from_tt);

        self.path_stack.push(key);
        let old_path_code = self.path_code;

        let mut outcome_to_store: Option<Outcome> = None;
        let mut best_move = Move::NONE;
        let mut pn = INF;
        let mut dn = INF;
        let mut repetition_seen = false;

        loop {
            if Instant::now() >= self.deadline {
                break;
            }

            let selection = self.select_children(pos, &moves, is_or_node);
            best_move = selection.best_move;
            pn = selection.pn;
            dn = selection.dn;
            repetition_seen = selection.repetition_seen;

            if let Some(solved) = selection.solved_outcome {
                outcome_to_store = Some(solved);
                break;
            }

            if (th_pn != INF && pn >= th_pn) || (th_dn != INF && dn >= th_dn) {
                break;
            }

            let (mv, child_pn, child_dn, _vpn, _vdn) = selection.best_child;
            let (second_pn, second_dn) = selection.second_child;

            let (np, nd) = if is_or_node {
                let new_th_pn = std::cmp::min(th_pn, self.epsilon_ceil(second_pn));
                let new_th_dn = if th_dn == INF {
                    INF
                } else {
                    th_dn.saturating_sub(dn).saturating_add(child_dn)
                };
                (new_th_pn, new_th_dn)
            } else {
                let new_th_dn = std::cmp::min(th_dn, self.epsilon_ceil(second_dn));
                let new_th_pn = if th_pn == INF {
                    INF
                } else {
                    th_pn.saturating_sub(pn).saturating_add(child_pn)
                };
                (new_th_pn, new_th_dn)
            };

            pos.do_move(mv);
            self.path_code ^= zobrist::path_random(mv, self.path_stack.len());
            let _ = self.dfpn(pos, np, nd, !is_or_node);
            self.path_code ^= zobrist::path_random(mv, self.path_stack.len());
            pos.undo_move(mv);
        }

        self.path_stack.pop();
        self.path.remove(&key);
        self.path_code = old_path_code;

        self.tt.store(
            key,
            best_move,
            outcome_to_store,
            pn,
            dn,
            old_path_code,
            repetition_seen,
        );
        outcome_to_store.unwrap_or(Outcome::Draw)
    }

    fn try_use_tt(&self, entry: &super::tt::TtEntry, path_code: u64) -> Option<Outcome> {
        entry.outcome?;
        if !entry.repetition_seen || entry.path_code == path_code {
            entry.outcome
        } else {
            None
        }
    }

    fn epsilon_ceil(&self, x: u64) -> u64 {
        if x >= INF {
            return INF;
        }
        let scaled = (x as f64 * (1.0 + self.epsilon)).ceil() as u64;
        scaled.min(INF)
    }

    fn select_children(
        &mut self,
        pos: &mut Position,
        moves: &MoveList,
        is_or_node: bool,
    ) -> ChildSelection {
        let mut children = Vec::with_capacity(moves.len());
        for i in 0..moves.len() {
            let mv = moves[i];
            let info = self.evaluate_child(pos, mv, is_or_node);
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

        let solved_outcome = Self::is_solved_by_children(&children, is_or_node);

        let (best_idx, second_idx) = Self::best_and_second(&children, is_or_node);
        let best = &children[best_idx];
        let second = second_idx.map(|i| &children[i]);

        let best_child = (best.mv, best.pn, best.dn, best.vpn, best.vdn);
        let second_child = second.map(|s| (s.pn, s.dn)).unwrap_or((INF, INF));

        let best_move = best.mv;

        let repetition_seen = children.iter().any(|c| c.repetition_seen);

        ChildSelection {
            best_child,
            second_child,
            pn,
            dn,
            best_move,
            solved_outcome,
            repetition_seen,
        }
    }

    fn evaluate_child(&mut self, pos: &mut Position, mv: Move, is_or_node: bool) -> ChildInfo {
        pos.do_move(mv);
        let child_key = pos.hash();
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
                repetition_seen: false,
            }
        } else if self.path.contains(&child_key) {
            let (pn, dn) = Outcome::Draw.pn_dn_for(child_is_or);
            ChildInfo {
                mv,
                pn,
                dn,
                vpn: pn,
                vdn: dn,
                outcome: Some(Outcome::Draw),
                repetition_seen: true,
            }
        } else if let Some(entry) = self.tt.probe(child_key) {
            if let Some(outcome) = self.try_use_tt(entry, child_path_code) {
                let (pn, dn) = outcome.pn_dn_for(child_is_or);
                ChildInfo {
                    mv,
                    pn,
                    dn,
                    vpn: pn,
                    vdn: dn,
                    outcome: Some(outcome),
                    repetition_seen: entry.repetition_seen,
                }
            } else {
                let (pn, dn) = if entry.outcome.is_some() {
                    (1, 1)
                } else {
                    (entry.pn, entry.dn)
                };
                ChildInfo {
                    mv,
                    pn,
                    dn,
                    vpn: pn,
                    vdn: dn,
                    outcome: None,
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
                repetition_seen: false,
            }
        };

        pos.undo_move(mv);
        info
    }

    fn is_solved_by_children(children: &[ChildInfo], _is_or_node: bool) -> Option<Outcome> {
        if children.iter().any(|c| c.outcome == Some(Outcome::Loss)) {
            return Some(Outcome::Win);
        }
        if children.iter().all(|c| c.outcome.is_some()) {
            if children.iter().all(|c| c.outcome == Some(Outcome::Draw)) {
                return Some(Outcome::Draw);
            }
            return Some(Outcome::Loss);
        }
        None
    }

    fn best_and_second(children: &[ChildInfo], is_or_node: bool) -> (usize, Option<usize>) {
        let mut best = 0;
        let mut second: Option<usize> = None;
        for i in 1..children.len() {
            let c = &children[i];
            let best_c = &children[best];
            let cmp_c = if is_or_node { c.vpn } else { c.vdn };
            let cmp_best = if is_or_node { best_c.vpn } else { best_c.vdn };
            if cmp_c < cmp_best {
                second = Some(best);
                best = i;
            } else if second.is_none() {
                second = Some(i);
            } else {
                let second_c = &children[second.unwrap()];
                let cmp_second = if is_or_node {
                    second_c.vpn
                } else {
                    second_c.vdn
                };
                if cmp_c < cmp_second {
                    second = Some(i);
                }
            }
        }
        (best, second)
    }

    fn sort_moves(&self, pos: &Position, moves: &mut MoveList, best_from_tt: Move) {
        let mut state = atomic_movegen::board::StateInfo::new();
        pos.board.populate_state(&mut state);

        let slice = moves.as_mut_slice();
        if best_from_tt != Move::NONE
            && let Some(idx) = slice.iter().position(|&m| m == best_from_tt)
        {
            slice.swap(0, idx);
        }

        slice.sort_by(|&a, &b| {
            let sa = self.scorer.score(&pos.board, a, &state);
            let sb = self.scorer.score(&pos.board, b, &state);
            sb.cmp(&sa)
        });
    }

    fn extract_pv(&self, pos: &Position) -> Vec<Move> {
        let mut pv = Vec::new();
        let mut seen = HashSet::new();
        let mut current = pos.clone();
        for _ in 0..1000 {
            let key = current.hash();
            if seen.contains(&key) {
                break;
            }
            if current.outcome().is_some() {
                break;
            }
            if let Some(entry) = self.tt.probe(key) {
                if entry.best_move == Move::NONE {
                    break;
                }
                seen.insert(key);
                pv.push(entry.best_move);
                current.do_move(entry.best_move);
            } else {
                break;
            }
        }
        pv
    }
}

struct ChildInfo {
    mv: Move,
    pn: u64,
    dn: u64,
    vpn: u64,
    vdn: u64,
    outcome: Option<Outcome>,
    repetition_seen: bool,
}

struct ChildSelection {
    best_child: (Move, u64, u64, u64, u64),
    second_child: (u64, u64),
    pn: u64,
    dn: u64,
    best_move: Move,
    solved_outcome: Option<Outcome>,
    repetition_seen: bool,
}

pub fn outcome_from_pn_dn(pn: u64, dn: u64) -> Option<Outcome> {
    if pn == 0 && dn == INF {
        Some(Outcome::Win)
    } else if pn == INF && dn == 0 {
        Some(Outcome::Loss)
    } else {
        None
    }
}
