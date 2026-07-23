//! Core DF-PN recursive search routine.

#![allow(clippy::similar_names)]

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::zobrist;

use super::children::{ChildInfo, ChildSelection};
use super::{INF, Search};

pub(super) struct Resolved {
    pub outcome: Outcome,
    pub depth: u32,
    pub repetition_seen: bool,
}

impl Search {
    pub(super) fn dfpn(
        &mut self,
        pos: &mut Position,
        th_pn: u64,
        th_dn: u64,
        max_depth: u32,
        is_or_node: bool,
    ) -> Outcome {
        if self.time_exceeded() {
            return Outcome::Draw;
        }

        self.nodes += 1;

        let tt_key = pos.hash();
        let rep_key = pos.repetition_key();

        // Track the effort spent in this subtree so the transposition table can
        // prefer keeping hard-won entries.
        let child_evals_start = self.child_evals;

        let mut moves = MoveList::new();
        let mut state = StateInfo::new();
        pos.legal_moves_with_state(&mut moves, &mut state);

        let path_length = self.path_stack.len() as u32;

        if let Some(outcome) = pos.outcome_from_state(&state, &moves) {
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
                self.path_code,
                path_length,
                false,
            );
            return outcome;
        }

        if max_depth == 0 {
            let (pn, dn) = Outcome::Draw.pn_dn_for(is_or_node);
            self.tt.store(
                tt_key,
                Move::NONE,
                u8::MAX,
                0,
                Some(Outcome::Draw),
                pn,
                dn,
                0,
                0,
                self.path_code,
                path_length,
                false,
            );
            return Outcome::Draw;
        }

        if let Some(resolved) = self.try_use_tt(pos, tt_key, max_depth, self.path_code, path_length)
        {
            return resolved.outcome;
        }

        if self.path_contains(rep_key) {
            return Outcome::Draw;
        }

        let previous_summary = self.tt.probe_summary(tt_key);

        let best_from_tt = self
            .tt
            .probe_best_move(tt_key, self.path_code)
            .unwrap_or(Move::NONE);
        self.sort_moves(pos, &mut moves, best_from_tt);

        self.path_push(rep_key);
        let old_path_code = self.path_code;

        let mut outcome_to_store: Option<Outcome> = None;
        let mut outcome_to_store_best_move = Move::NONE;
        let mut outcome_to_store_pn = INF;
        let mut outcome_to_store_dn = INF;
        let mut outcome_to_store_depth = 0;
        let mut outcome_to_store_repetition_seen = false;
        let mut best_win_depth = u32::MAX;
        let mut best_loss_depth = 0u32;
        let mut best_move = Move::NONE;
        let mut pn = INF;
        let mut dn = INF;
        let mut depth = 0;
        let mut repetition_seen = false;

        let mut children: Vec<ChildInfo> = Vec::new();
        let mut selection: Option<ChildSelection> = None;

        loop {
            if self.time_exceeded() {
                break;
            }

            if children.is_empty() {
                children = self.evaluate_all_children(
                    pos,
                    &moves,
                    max_depth,
                    is_or_node,
                    self.refine_shortest,
                );
            } else if let Some(prev) = selection
                && let Some(idx) = prev.best_child_index
            {
                let mv = children[idx].mv;
                children[idx] = self.evaluate_child(pos, mv, max_depth, is_or_node);
            }

            let previous_best_move = previous_summary
                .as_ref()
                .filter(|s| s.best_move != Move::NONE)
                .map(|s| s.best_move);
            let previous_best_child = previous_summary
                .as_ref()
                .filter(|s| s.best_child != u8::MAX)
                .map(|s| s.best_child);
            selection = Some(Search::select_from_children(
                &children,
                is_or_node,
                self.refine_shortest,
                previous_best_move,
                previous_best_child,
            ));
            let selection = selection.as_ref().unwrap();
            best_move = selection.best_move;
            pn = selection.pn;
            dn = selection.dn;
            depth = selection.depth;
            repetition_seen = selection.repetition_seen;

            if let Some(solved) = selection.solved_outcome {
                if solved == Outcome::Win {
                    // Keep the shortest known winning child.
                    if selection.depth < best_win_depth {
                        best_win_depth = selection.depth;
                        outcome_to_store = Some(solved);
                        outcome_to_store_best_move = selection.best_move;
                        outcome_to_store_pn = selection.pn;
                        outcome_to_store_dn = selection.dn;
                        outcome_to_store_depth = best_win_depth;
                        outcome_to_store_repetition_seen = selection.repetition_seen;
                    }
                    if selection.all_solved {
                        break;
                    }
                    if !self.refine_shortest {
                        break;
                    }
                } else if solved == Outcome::Loss {
                    // Keep the longest known losing child (most resistant defense).
                    if selection.depth > best_loss_depth {
                        best_loss_depth = selection.depth;
                        outcome_to_store = Some(solved);
                        outcome_to_store_best_move = selection.best_move;
                        outcome_to_store_pn = selection.pn;
                        outcome_to_store_dn = selection.dn;
                        outcome_to_store_depth = best_loss_depth;
                        outcome_to_store_repetition_seen = selection.repetition_seen;
                    }
                    if selection.all_solved {
                        break;
                    }
                } else {
                    outcome_to_store = Some(solved);
                    outcome_to_store_best_move = selection.best_move;
                    outcome_to_store_pn = selection.pn;
                    outcome_to_store_dn = selection.dn;
                    outcome_to_store_depth = selection.depth;
                    outcome_to_store_repetition_seen = selection.repetition_seen;
                    if selection.all_solved {
                        break;
                    }
                }
            }

            if (th_pn != INF && pn >= th_pn) || (th_dn != INF && dn >= th_dn) {
                break;
            }

            let (mv, child_pn, child_dn) = selection.best_child;
            if mv == Move::NONE {
                break;
            }
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
            let _ = self.dfpn(pos, np, nd, max_depth.saturating_sub(1), !is_or_node);
            self.path_code ^= zobrist::path_random(mv, self.path_stack.len());
            pos.undo_move(mv);
        }

        let store_best_move = if outcome_to_store.is_some() {
            outcome_to_store_best_move
        } else {
            best_move
        };
        let store_best_child = children
            .iter()
            .position(|c| c.mv == store_best_move)
            .map_or(u8::MAX, |i| i as u8);
        let work = self.child_evals - child_evals_start;
        let store_remaining_depth = match outcome_to_store {
            Some(Outcome::Win | Outcome::Loss) => u32::MAX,
            _ => max_depth,
        };
        self.tt.store(
            tt_key,
            store_best_move,
            store_best_child,
            work,
            outcome_to_store,
            if outcome_to_store.is_some() {
                outcome_to_store_pn
            } else {
                pn
            },
            if outcome_to_store.is_some() {
                outcome_to_store_dn
            } else {
                dn
            },
            if outcome_to_store.is_some() {
                outcome_to_store_depth
            } else {
                depth
            },
            store_remaining_depth,
            old_path_code,
            (self.path_stack.len() - 1) as u32,
            if outcome_to_store.is_some() {
                outcome_to_store_repetition_seen
            } else {
                repetition_seen
            },
        );

        if let Some(outcome) = outcome_to_store
            && outcome != Outcome::Draw
            && store_best_move != Move::NONE
        {
            let us = pos.side_to_move();
            self.update_history(store_best_move, us);
            self.update_killers(store_best_move);
        }

        self.maybe_age_history();

        self.path_pop();
        self.path_code = old_path_code;

        outcome_to_store.unwrap_or(Outcome::Draw)
    }

    pub(super) fn epsilon_ceil(&self, x: u64) -> u64 {
        if x >= INF {
            return INF;
        }
        let scaled =
            (x as u128 * self.epsilon_num as u128).div_ceil(self.epsilon_den as u128) as u64;
        scaled.max(x.saturating_add(1)).min(INF)
    }
}

/// Convert solved `pn`/`dn` bounds back to an [`Outcome`].
///
/// This can only be done unambiguously for a Win (`pn == 0`, `dn == INF`).
/// `Loss` and `Draw` both encode as `(INF, 0)`, so `(INF, 0)` returns `None`.
/// The `outcome` field stored in the transposition table must be used as the
/// source of truth when a distinction between `Loss` and `Draw` is required.
pub fn outcome_from_pn_dn(pn: u64, dn: u64) -> Option<Outcome> {
    if pn == 0 && dn == INF {
        Some(Outcome::Win)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::search::dfpn::{INF, Search};

    #[test]
    fn epsilon_ceil_scales_threshold() {
        let mut search = Search::new(64);

        search.set_epsilon(0.0);
        assert_eq!(search.epsilon_ceil(0), 1);
        assert_eq!(search.epsilon_ceil(1), 2);
        assert_eq!(search.epsilon_ceil(5), 6);
        assert_eq!(search.epsilon_ceil(100), 101);
        assert_eq!(search.epsilon_ceil(1_000_000), 1_000_001);
        assert_eq!(search.epsilon_ceil(INF - 1), INF);
        assert_eq!(search.epsilon_ceil(INF), INF);

        search.set_epsilon(0.25);
        assert_eq!(search.epsilon_ceil(0), 1);
        assert_eq!(search.epsilon_ceil(1), 2);
        assert_eq!(search.epsilon_ceil(10), 13);
        assert_eq!(search.epsilon_ceil(100), 125);
        assert_eq!(search.epsilon_ceil(1_000_000), 1_250_000);
        assert_eq!(search.epsilon_ceil(INF - 1), INF);
        assert_eq!(search.epsilon_ceil(INF), INF);

        search.set_epsilon(0.5);
        assert_eq!(search.epsilon_ceil(10), 15);
        assert_eq!(search.epsilon_ceil(1_000_000), 1_500_000);
        assert_eq!(search.epsilon_ceil(INF - 1), INF);

        search.set_epsilon(1.0);
        assert_eq!(search.epsilon_ceil(0), 1);
        assert_eq!(search.epsilon_ceil(1), 2);
        assert_eq!(search.epsilon_ceil(10), 20);
        assert_eq!(search.epsilon_ceil(1_000_000), 2_000_000);
        assert_eq!(search.epsilon_ceil(INF - 1), INF);
    }

    #[test]
    #[should_panic(expected = "epsilon must be in [0.0, 1.0]")]
    fn set_epsilon_rejects_negative() {
        let mut search = Search::new(64);
        search.set_epsilon(-0.1);
    }

    #[test]
    #[should_panic(expected = "epsilon must be in [0.0, 1.0]")]
    fn set_epsilon_rejects_greater_than_one() {
        let mut search = Search::new(64);
        search.set_epsilon(1.1);
    }
}
