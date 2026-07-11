//! Exact win/loss/draw solver using retrograde-style minimax with transpositions.

use std::collections::HashSet;
use std::time::Instant;

use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};

use super::ordering::{MoveScorer, StaticAtomicScorer};
use super::tt::TranspositionTable;

pub const INF: u64 = 1 << 60;

pub struct Search {
    tt: TranspositionTable,
    path: HashSet<u64>,
    nodes: u64,
    start: Instant,
    scorer: Box<dyn MoveScorer>,
}

impl Search {
    pub fn new(tt_mb: usize) -> Self {
        Self {
            tt: TranspositionTable::with_mb(tt_mb),
            path: HashSet::new(),
            nodes: 0,
            start: Instant::now(),
            scorer: Box::new(StaticAtomicScorer),
        }
    }

    pub fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        self.nodes = 0;
        self.start = Instant::now();
        self.path.clear();
        let outcome = self.solve_position(pos);
        let pv = self.extract_pv(pos);
        (outcome, pv, self.nodes)
    }

    fn solve_position(&mut self, pos: &mut Position) -> Outcome {
        self.nodes += 1;
        let key = pos.hash();

        if let Some(entry) = self.tt.probe(key)
            && let Some(outcome) = entry.outcome {
                return outcome;
            }

        if let Some(outcome) = pos.outcome() {
            self.tt.store(key, Move::NONE, Some(outcome), 0, 0);
            return outcome;
        }

        if !self.path.insert(key) {
            return Outcome::Draw;
        }

        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);

        if moves.is_empty() {
            self.path.remove(&key);
            self.tt.store(key, Move::NONE, Some(Outcome::Draw), 0, 0);
            return Outcome::Draw;
        }

        let best_from_tt = self.tt.probe(key).map(|e| e.best_move).unwrap_or(Move::NONE);
        self.sort_moves(pos, &mut moves, best_from_tt);

        let mut best = Outcome::Loss;
        let mut best_move = Move::NONE;

        for i in 0..moves.len() {
            let m = moves[i];
            pos.do_move(m);
            let child = self.solve_position(pos);
            pos.undo_move(m);

            let current = child.flip();
            if current > best {
                best = current;
                best_move = m;
                if best == Outcome::Win {
                    break;
                }
            }
        }

        self.path.remove(&key);
        self.tt.store(key, best_move, Some(best), 0, 0);
        best
    }

    fn sort_moves(&self, pos: &Position, moves: &mut MoveList, best_from_tt: Move) {
        let mut state = atomic_movegen::board::StateInfo::new();
        pos.board.populate_state(&mut state);

        let slice = moves.as_mut_slice();
        if best_from_tt != Move::NONE
            && let Some(idx) = slice.iter().position(|&m| m == best_from_tt) {
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

pub fn outcome_from_pn_dn(pn: u64, dn: u64) -> Option<Outcome> {
    if pn == 0 && dn == INF {
        Some(Outcome::Win)
    } else if dn == 0 && pn == INF {
        Some(Outcome::Loss)
    } else if pn == INF && dn == INF {
        Some(Outcome::Draw)
    } else {
        None
    }
}
