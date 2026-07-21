//! History and killer move ordering helpers.

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Color, Move, MoveList};

use crate::position::Position;

use super::Search;
use crate::search::ordering::MoveScorer;

pub(crate) const HISTORY_MAX: i32 = 10_000;
pub(crate) const HISTORY_BONUS: i32 = 100;
pub(crate) const HISTORY_AGE_INTERVAL: u64 = 10_000;
pub(crate) const SCORE_KILLER: i32 = 50_000;
pub(crate) const KILLER_SLOTS: usize = 2;
pub(crate) const MAX_KILLER_DEPTH: usize = 256;

impl Search {
    pub(super) fn sort_moves(&self, pos: &Position, moves: &mut MoveList, best_from_tt: Move) {
        let mut state = StateInfo::new();
        pos.board.populate_state(&mut state);

        let us = pos.side_to_move() as usize;
        let depth = self.path_stack.len();

        let slice = moves.as_mut_slice();
        let mut scored: Vec<(Move, i32)> = slice
            .iter()
            .copied()
            .map(|m| {
                let score = self.scorer.score(&pos.board, m, &state)
                    + self.history[us][m.from_sq() as usize][m.to_sq() as usize]
                    + self.killer_bonus(m, depth);
                (m, score)
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

        for (i, (m, _)) in scored.into_iter().enumerate() {
            slice[i] = m;
        }
    }

    pub(super) fn update_history(&mut self, m: Move, side: Color) {
        let from = m.from_sq() as usize;
        let to = m.to_sq() as usize;
        let entry = &mut self.history[side as usize][from][to];
        *entry = (*entry + HISTORY_BONUS).min(HISTORY_MAX);
    }

    pub(super) fn update_killers(&mut self, best_move: Move) {
        if best_move == Move::NONE {
            return;
        }
        let depth = self.path_stack.len();
        if depth >= MAX_KILLER_DEPTH {
            return;
        }
        let slot = &mut self.killers[depth];
        if best_move != slot[0] {
            slot[1] = slot[0];
            slot[0] = best_move;
        }
    }

    pub(super) fn maybe_age_history(&mut self) {
        self.history_age_counter += 1;
        if self.history_age_counter < HISTORY_AGE_INTERVAL {
            return;
        }
        self.history_age_counter = 0;
        for side in &mut self.history {
            for from in side {
                for entry in from {
                    *entry = (*entry / 2).min(HISTORY_MAX);
                }
            }
        }
    }

    fn killer_bonus(&self, m: Move, depth: usize) -> i32 {
        if depth >= MAX_KILLER_DEPTH {
            return 0;
        }
        if self.killers[depth].contains(&m) {
            SCORE_KILLER
        } else {
            0
        }
    }
}
