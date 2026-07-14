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
                0,
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
                0,
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
        let mut depth = 0;
        let mut repetition_seen = false;

        loop {
            if Instant::now() >= self.deadline {
                break;
            }

            let selection = self.select_children(pos, &moves, is_or_node);
            best_move = selection.best_move;
            pn = selection.pn;
            dn = selection.dn;
            depth = selection.depth;
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
            depth,
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

        let solved = Self::is_solved_by_children(&children, is_or_node);

        let (best_idx, second_idx) = Self::best_and_second(&children, is_or_node);
        let best = &children[best_idx];
        let second = second_idx.map(|i| &children[i]);

        let best_child = (best.mv, best.pn, best.dn, best.vpn, best.vdn);
        let second_child = second.map(|s| (s.pn, s.dn)).unwrap_or((INF, INF));

        let best_move = if let Some((_, _, mv)) = solved {
            mv
        } else {
            best.mv
        };

        let depth = solved.map(|(_, d, _)| d).unwrap_or(0);

        let repetition_seen = children.iter().any(|c| c.repetition_seen);

        ChildSelection {
            best_child,
            second_child,
            pn,
            dn,
            depth,
            best_move,
            solved_outcome: solved.map(|(o, _, _)| o),
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
                depth: 0,
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
                depth: 0,
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
                    depth: entry.depth,
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

    fn is_solved_by_children(
        children: &[ChildInfo],
        _is_or_node: bool,
    ) -> Option<(Outcome, u32, Move)> {
        let mut all_solved = true;
        let mut win_depth = u32::MAX;
        let mut win_mv = Move::NONE;
        let mut draw_depth = 0;
        let mut draw_mv = Move::NONE;
        let mut found_draw = false;
        let mut loss_depth = 0;
        let mut loss_mv = Move::NONE;

        for c in children {
            let d = c.depth.saturating_add(1);
            match c.outcome {
                None => {
                    all_solved = false;
                }
                Some(Outcome::Loss) => {
                    if d < win_depth {
                        win_depth = d;
                        win_mv = c.mv;
                    }
                }
                Some(Outcome::Draw) => {
                    if !found_draw || d > draw_depth {
                        draw_depth = d;
                        draw_mv = c.mv;
                    }
                    found_draw = true;
                }
                Some(Outcome::Win) => {
                    if d > loss_depth {
                        loss_depth = d;
                        loss_mv = c.mv;
                    }
                }
            }
        }

        // A win can be declared as soon as a losing child is found.
        if win_depth != u32::MAX {
            return Some((Outcome::Win, win_depth, win_mv));
        }

        if all_solved {
            if found_draw {
                return Some((Outcome::Draw, draw_depth, draw_mv));
            }
            return Some((Outcome::Loss, loss_depth, loss_mv));
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
    depth: u32,
    repetition_seen: bool,
}

struct ChildSelection {
    best_child: (Move, u64, u64, u64, u64),
    second_child: (u64, u64),
    pn: u64,
    dn: u64,
    depth: u32,
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

#[cfg(test)]
mod tests {
    use super::*;
    use atomic_movegen::types::Square;

    fn child(outcome: Option<Outcome>, depth: u32, from: Square, to: Square) -> ChildInfo {
        ChildInfo {
            mv: Move::make_move(from, to),
            pn: 1,
            dn: 1,
            vpn: 1,
            vdn: 1,
            outcome,
            depth,
            repetition_seen: false,
        }
    }

    #[test]
    fn win_picks_shortest_loss_child() {
        let children = vec![
            child(Some(Outcome::Loss), 5, Square::A1, Square::A2),
            child(Some(Outcome::Loss), 2, Square::B1, Square::B2),
            child(Some(Outcome::Win), 0, Square::C1, Square::C2),
        ];
        let (outcome, depth, mv) = Search::is_solved_by_children(&children, true).unwrap();
        assert_eq!(outcome, Outcome::Win);
        assert_eq!(depth, 3);
        assert_eq!(mv, Move::make_move(Square::B1, Square::B2));
    }

    #[test]
    fn draw_picks_longest_draw_child() {
        let children = vec![
            child(Some(Outcome::Win), 4, Square::A1, Square::A2),
            child(Some(Outcome::Draw), 1, Square::B1, Square::B2),
            child(Some(Outcome::Draw), 7, Square::C1, Square::C2),
        ];
        let (outcome, depth, mv) = Search::is_solved_by_children(&children, false).unwrap();
        assert_eq!(outcome, Outcome::Draw);
        assert_eq!(depth, 8);
        assert_eq!(mv, Move::make_move(Square::C1, Square::C2));
    }

    #[test]
    fn loss_picks_longest_win_child() {
        let children = vec![
            child(Some(Outcome::Win), 2, Square::A1, Square::A2),
            child(Some(Outcome::Win), 5, Square::B1, Square::B2),
        ];
        let (outcome, depth, mv) = Search::is_solved_by_children(&children, true).unwrap();
        assert_eq!(outcome, Outcome::Loss);
        assert_eq!(depth, 6);
        assert_eq!(mv, Move::make_move(Square::B1, Square::B2));
    }

    #[test]
    fn unsolved_returns_none() {
        let children = vec![
            child(Some(Outcome::Win), 0, Square::A1, Square::A2),
            child(None, 0, Square::B1, Square::B2),
        ];
        assert!(Search::is_solved_by_children(&children, true).is_none());
    }
}
