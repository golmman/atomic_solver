//! Core DF-PN recursive search routine.

#![allow(clippy::similar_names)]

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};

use super::children::{ChildInfo, ChildSelection};
use super::selection::select_from_children;
use super::{INF, Search};

pub(super) struct Resolved {
    pub outcome: Outcome,
    pub depth: u32,
}

impl Search {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn dfpn(
        &mut self,
        pos: &mut Position,
        th_pn: u64,
        th_dn: u64,
        max_depth: u32,
        max_work: u64,
        is_or_node: bool,
    ) -> Outcome {
        let emit = self.proof_tree_sender.is_some();

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
            );
            self.emit_proof_node(outcome, 0);
            return outcome;
        }

        if max_depth == 0 {
            // A non-terminal leaf is an unsolved frontier, not a proven draw.
            // Store cheap (1, 1) bounds so the next deeper probe can grow past
            // the horizon without re-expanding the entire subtree.
            self.tt
                .store(tt_key, Move::NONE, u8::MAX, 0, None, 1, 1, 0, 0);
            return Outcome::Draw;
        }

        // Local repetition: this board is already on the current search stack.
        if self.path_contains(rep_key) {
            return Outcome::Draw;
        }

        if let Some(resolved) = self.try_use_tt(pos, tt_key, max_depth) {
            self.emit_proof_node(resolved.outcome, resolved.depth);
            return resolved.outcome;
        }

        let previous_summary = self.tt.probe_summary(tt_key);

        let best_from_tt = self.tt.probe_best_move(tt_key).unwrap_or(Move::NONE);
        self.sort_moves(pos, &mut moves, best_from_tt);

        self.path_push(rep_key);

        let mut outcome_to_store: Option<Outcome> = None;
        let mut outcome_to_store_best_move = Move::NONE;
        let mut outcome_to_store_pn = INF;
        let mut outcome_to_store_dn = INF;
        let mut outcome_to_store_depth = 0;
        let mut outcome_to_store_repetition_seen = false;
        let mut best_move = Move::NONE;
        let mut pn = INF;
        let mut dn = INF;
        let mut depth = 0;

        let mut children: Vec<ChildInfo> = Vec::new();
        let mut selection: Option<ChildSelection> = None;

        loop {
            if self.time_exceeded() {
                break;
            }

            // Work-bounded search: stop this chunk once it has consumed its node
            // budget.  The result is stored as an unsolved entry so the next chunk
            // can resume from the same bounds.
            if max_work != u64::MAX && self.child_evals - child_evals_start >= max_work {
                break;
            }

            if children.is_empty() {
                children = self.evaluate_all_children(pos, &moves, max_depth, is_or_node);
            } else if let Some(prev) = selection
                && let Some(idx) = prev.best_child_index
            {
                let mv = children[idx].mv;
                let old_pn = children[idx].pn;
                let old_dn = children[idx].dn;
                children[idx] = self.evaluate_child(pos, mv, max_depth, is_or_node);
                // In a work-bounded call, if the child came back with exactly the
                // same (pn, dn) bounds re-expanding it cannot make progress. Mark
                // it explored so the search moves on to other children.
                if max_work != u64::MAX && children[idx].pn == old_pn && children[idx].dn == old_dn
                {
                    children[idx].explored = true;
                }
            }

            let previous_best_move = previous_summary
                .as_ref()
                .filter(|s| s.best_move != Move::NONE)
                .map(|s| s.best_move);
            let previous_best_child = previous_summary
                .as_ref()
                .filter(|s| s.best_child != u8::MAX)
                .map(|s| s.best_child);
            selection = Some(select_from_children(
                &children,
                is_or_node,
                previous_best_move,
                previous_best_child,
            ));
            let selection = selection.as_ref().unwrap();
            best_move = selection.best_move;
            pn = selection.pn;
            dn = selection.dn;
            depth = selection.depth;

            if let Some(solved) = selection.solved_outcome {
                // Win: one winning child is enough. Loss and Draw require all
                // children to be solved.
                outcome_to_store = Some(solved);
                outcome_to_store_best_move = selection.best_move;
                outcome_to_store_pn = selection.pn;
                outcome_to_store_dn = selection.dn;
                outcome_to_store_depth = selection.depth;
                outcome_to_store_repetition_seen = selection.repetition_seen;
                break;
            }

            if (th_pn != INF && pn >= th_pn) || (th_dn != INF && dn >= th_dn) {
                break;
            }

            let (mv, child_pn, child_dn) = selection.best_child;
            if mv == Move::NONE {
                break;
            }
            let (second_pn, second_dn) = selection.second_child;

            let work_spent = self.child_evals - child_evals_start;
            if max_work != u64::MAX && work_spent >= max_work {
                break;
            }
            let child_max_work = max_work.saturating_sub(work_spent);

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
            if emit {
                let uci = crate::notation::move_to_uci(mv);
                let proof_len = self.proof_path.len();
                self.proof_path.push('.');
                self.proof_path.push_str(&uci);
                self.move_stack.push(mv);
                let _ = self.dfpn(
                    pos,
                    np,
                    nd,
                    max_depth.saturating_sub(1),
                    child_max_work,
                    !is_or_node,
                );
                self.move_stack.pop();
                self.proof_path.truncate(proof_len);
            } else {
                let _ = self.dfpn(
                    pos,
                    np,
                    nd,
                    max_depth.saturating_sub(1),
                    child_max_work,
                    !is_or_node,
                );
            }
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

        // First-player-loss shortcut: do not cache a Draw that only holds
        // because of a repetition in the current path. Store it as an unsolved
        // (1, 1) entry so the next search re-expands it and sees the local Draw.
        let suppress_draw =
            outcome_to_store == Some(Outcome::Draw) && outcome_to_store_repetition_seen;
        let store_outcome = if suppress_draw {
            None
        } else {
            outcome_to_store
        };
        let (store_pn, store_dn) = if suppress_draw {
            (1, 1)
        } else if outcome_to_store.is_some() {
            (outcome_to_store_pn, outcome_to_store_dn)
        } else {
            (pn.max(1), dn.max(1))
        };
        let store_depth = if outcome_to_store.is_some() {
            outcome_to_store_depth
        } else {
            depth
        };
        let store_remaining_depth = if outcome_to_store.is_some() {
            u32::MAX
        } else {
            max_depth
        };

        self.tt.store(
            tt_key,
            store_best_move,
            store_best_child,
            work,
            store_outcome,
            store_pn,
            store_dn,
            store_depth,
            store_remaining_depth,
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

        if let Some(outcome) = outcome_to_store {
            self.emit_proof_node(outcome, outcome_to_store_depth);
            outcome
        } else {
            Outcome::Draw
        }
    }

    /// Try to reuse a solved, path-independent result from the transposition table.
    ///
    /// A one-ply guard rejects the result if the stored best move would
    /// immediately repeat a position on the current search stack. This catches
    /// the most obvious cross-path GHI case without keeping the full simulation
    /// machinery.
    pub(super) fn try_use_tt(&self, pos: &Position, key: u64, max_depth: u32) -> Option<Resolved> {
        let entry = self.tt.probe(key)?;
        let outcome = entry.outcome?;
        if entry.remaining_depth < max_depth || entry.depth > max_depth {
            return None;
        }

        if entry.best_move != Move::NONE {
            let mut child = pos.clone();
            child.do_move(entry.best_move);
            if self.path_stack.contains(&child.repetition_key()) {
                return None;
            }
        }

        Some(Resolved {
            outcome,
            depth: entry.depth,
        })
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
    use crate::position::Outcome;
    use crate::search::dfpn::{INF, Search, outcome_from_pn_dn};

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

    #[test]
    fn outcome_from_pn_dn_only_recognizes_win() {
        assert_eq!(outcome_from_pn_dn(0, INF), Some(Outcome::Win));
        assert_eq!(outcome_from_pn_dn(INF, 0), None);
        assert_eq!(outcome_from_pn_dn(1, 1), None);
    }
}
