//! Kawano-style simulation for verifying cross-path twin entries.

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::search::tt::MAX_TWINS;
use crate::zobrist;

use super::Search;
use super::core::Resolved;

pub const SIM_MAX_DEPTH: usize = 1000;
const SIM_MAX_NODES: u64 = 1000;

impl Search {
    pub(super) fn try_use_tt(
        &mut self,
        pos: &Position,
        key: u64,
        max_depth: u32,
        path_code: u64,
        path_length: u32,
    ) -> Option<Resolved> {
        // 1. Path-independent base result.
        let entry = self.tt.probe(key)?;
        if let Some(outcome) = entry.outcome
            && !entry.repetition_seen
            && entry.remaining_depth >= max_depth
        {
            if outcome == Outcome::Draw {
                return Some(Resolved {
                    outcome,
                    depth: 0,
                    repetition_seen: false,
                });
            }
            if entry.depth <= max_depth {
                return Some(Resolved {
                    outcome,
                    depth: entry.depth,
                    repetition_seen: false,
                });
            }
        }

        // 2. Try existing twins for the current path.
        for twin in entry.twins.iter() {
            if let Some(outcome) = twin.outcome
                && twin.path_code == path_code
                && twin.remaining_depth >= max_depth
            {
                if outcome == Outcome::Draw {
                    return Some(Resolved {
                        outcome,
                        depth: 0,
                        repetition_seen: true,
                    });
                }
                if twin.depth <= max_depth {
                    return Some(Resolved {
                        outcome,
                        depth: twin.depth,
                        repetition_seen: true,
                    });
                }
            }
        }

        // 3. Kawano simulation: verify a twin from another path for the current path.
        // Probe the entry once per twin so that the mutable `store_twin` call is not
        // blocked by an outstanding immutable borrow.
        for i in 0..MAX_TWINS {
            let (outcome, twin) = {
                let entry = self.tt.probe(key)?;
                let twin = entry.twins[i];
                let outcome = match twin.outcome {
                    Some(o) => o,
                    None => continue,
                };
                if twin.remaining_depth < max_depth || twin.depth > max_depth {
                    continue;
                }
                (outcome, twin)
            };

            let mut sim_pos = pos.clone();
            let mut sim_stack = self.path_stack.clone();
            let mut sim_nodes = 0u64;
            if self.simulate(
                &mut sim_pos,
                twin.path_code,
                twin.path_length,
                outcome,
                twin.best_move,
                &mut sim_stack,
                &mut sim_nodes,
                self.max_ply.max(SIM_MAX_DEPTH),
            ) {
                self.tt.store_twin(
                    key,
                    path_code,
                    path_length,
                    outcome,
                    twin.best_move,
                    twin.depth,
                    twin.remaining_depth,
                );
                return Some(Resolved {
                    outcome,
                    depth: twin.depth,
                    repetition_seen: true,
                });
            }
        }

        None
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn simulate(
        &self,
        pos: &mut Position,
        path_code: u64,
        path_length: u32,
        expected: Outcome,
        best_move: Move,
        sim_stack: &mut Vec<u64>,
        sim_nodes: &mut u64,
        remaining_depth: usize,
    ) -> bool {
        if *sim_nodes >= SIM_MAX_NODES {
            return false;
        }
        *sim_nodes += 1;

        if remaining_depth == 0 {
            return false;
        }

        let mut moves = MoveList::new();
        let mut state = StateInfo::new();
        pos.legal_moves_with_state(&mut moves, &mut state);

        if let Some(outcome) = pos.outcome_from_state(&state, &moves) {
            return outcome == expected;
        }

        let rep_key = pos.repetition_key();
        if sim_stack.contains(&rep_key) {
            return expected == Outcome::Draw;
        }
        sim_stack.push(rep_key);

        let child_depth = (path_length as usize).saturating_add(1);

        let ok = match expected {
            Outcome::Win | Outcome::Draw => {
                if best_move == Move::NONE {
                    false
                } else {
                    pos.do_move(best_move);
                    let child_tt_key = pos.hash();
                    let child_path_code = path_code ^ zobrist::path_random(best_move, child_depth);
                    let child_path_length = path_length.saturating_add(1);
                    let child_expected = if expected == Outcome::Draw {
                        Outcome::Draw
                    } else {
                        Outcome::Loss
                    };
                    let ok = if let Some(outcome) = pos.outcome() {
                        outcome == child_expected
                    } else {
                        let child_best = self
                            .tt
                            .probe(child_tt_key)
                            .and_then(|e| e.find_result_for_path(child_path_code, child_expected));
                        child_best.is_some_and(|b| {
                            self.simulate(
                                pos,
                                child_path_code,
                                child_path_length,
                                child_expected,
                                b.best_move,
                                sim_stack,
                                sim_nodes,
                                remaining_depth - 1,
                            )
                        })
                    };
                    pos.undo_move(best_move);
                    ok
                }
            }
            Outcome::Loss => {
                let mut ok = true;
                for i in 0..moves.len() {
                    let mv = moves[i];
                    pos.do_move(mv);
                    let child_tt_key = pos.hash();
                    let child_path_code = path_code ^ zobrist::path_random(mv, child_depth);
                    let child_path_length = path_length.saturating_add(1);
                    let child_ok = if let Some(outcome) = pos.outcome() {
                        outcome == Outcome::Win
                    } else {
                        let child_best = self
                            .tt
                            .probe(child_tt_key)
                            .and_then(|e| e.find_result_for_path(child_path_code, Outcome::Win));
                        child_best.is_some_and(|b| {
                            self.simulate(
                                pos,
                                child_path_code,
                                child_path_length,
                                Outcome::Win,
                                b.best_move,
                                sim_stack,
                                sim_nodes,
                                remaining_depth - 1,
                            )
                        })
                    };
                    if !child_ok {
                        ok = false;
                    }
                    pos.undo_move(mv);
                    if !ok {
                        break;
                    }
                }
                ok
            }
        };

        sim_stack.pop();
        ok
    }
}
