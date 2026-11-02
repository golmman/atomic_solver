//! Sibling proof replay (Kawano's simulation), first form.
//!
//! At an AND node a refuted defence has a proof: the attacker's winning line
//! from the position after that defence. The same line often answers the other
//! defences. This module replays it on each still-open sibling, in lockstep
//! with the guide position whose proof it is. A replay is exact: a sibling is
//! accepted only when every one of its defences is answered by a terminal
//! loss, a one-ply win, a path-independent TT proof, or the same defence in
//! the guide. A position repeated on the search path or on the replay line is
//! a Draw and is never accepted, and rule50 stays a terminal Draw through
//! [`Position::outcome`].
//!
//! The first form deliberately has **no** "parried threat" proof-number
//! halving and no null-move / static-tactics probe (that is the second form).
//! It only stores the newly proven sibling root in the scratch TT; the
//! intermediate replay nodes are emitted as proof events but are not cached.
//!
//! Budget: every simulated position and every legal move tried by the
//! one-ply immediate-win helper charges one child evaluation to the front-end
//! cap, and each replay has its own [`SIMULATION_BUDGET`]-position budget
//! (16, matching provari's `SIMULATION`). The deterministic cap is checked at
//! the DF-PN loop heads, so one replay can cross it by at most its own budget.

use atomic_movegen::types::{Move, MoveList};

use super::WinProver;
use super::children::ChildInfo;
use crate::position::{Outcome, Position};

/// Positions a single replay may visit, matching provari's `SIMULATION = 16`.
const SIMULATION_BUDGET: u32 = 16;

/// A replay result: the proof depth in plies and the attacker's first move.
pub(super) struct SimulatedProof {
    depth: u32,
    best_move: Move,
}

impl WinProver {
    /// Replay the freshly refuted defence `guide_mv`'s proof on every other
    /// open child of an AND node. Successfully replayed siblings are marked
    /// solved and stored in the scratch TT; failed ones stay open but are
    /// flagged `simulated` so they are not retried.
    pub(super) fn run_simulation(
        &mut self,
        pos: &Position,
        children: &mut [ChildInfo],
        guide_mv: Move,
        is_or_node: bool,
    ) {
        if is_or_node || !self.simulation_enabled {
            return;
        }
        let mut guide = pos.clone();
        guide.do_move(guide_mv);

        for child in children.iter_mut() {
            if child.mv == guide_mv || child.simulated || child.outcome.is_some() {
                continue;
            }
            child.simulated = true;
            self.simulation_attempts += 1;

            let mut sibling = pos.clone();
            sibling.do_move(child.mv);
            let mut budget = SIMULATION_BUDGET;
            let mut line = Vec::new();
            let mut path = self.move_stack.clone();
            path.push(child.mv);

            let Some(proof) =
                self.simulate_attacker(&sibling, &guide, &mut budget, &mut line, &mut path)
            else {
                continue;
            };
            let (pn, dn) = Outcome::Win.pn_dn_for(true);
            self.tt.store(
                sibling.hash(),
                proof.best_move,
                u8::MAX,
                0,
                Some(Outcome::Win),
                pn,
                dn,
                proof.depth,
                u32::MAX,
            );
            child.outcome = Some(Outcome::Win);
            child.pn = pn;
            child.dn = dn;
            child.depth = proof.depth;
            self.simulation_successes += 1;
        }
    }

    /// Attacker to move: find a proof for `pos` by following `guide`'s.
    pub(super) fn simulate_attacker(
        &mut self,
        pos: &Position,
        guide: &Position,
        budget: &mut u32,
        line: &mut Vec<u64>,
        path: &mut Vec<Move>,
    ) -> Option<SimulatedProof> {
        match pos.outcome() {
            Some(Outcome::Win) => {
                self.emit_proof_node_with_path(path, pos, Outcome::Win, 0);
                return Some(SimulatedProof {
                    depth: 0,
                    best_move: Move::NONE,
                });
            }
            Some(_) => return None,
            None => {}
        }
        let rep = pos.repetition_key();
        if self.path_contains(rep) || line.contains(&rep) {
            return None;
        }
        if let Some(entry) = self.tt.probe(pos.hash()).copied()
            && let Some(outcome) = entry.outcome
        {
            if outcome == Outcome::Win {
                // Re-emit at the replay path: the existing event (if any) is
                // at a different path, and the worker needs this node's
                // hash/outcome/depth to canonicalise the transposition.
                self.emit_proof_node_with_path(path, pos, outcome, entry.depth);
                return Some(SimulatedProof {
                    depth: entry.depth,
                    best_move: entry.best_move,
                });
            }
            return None;
        }
        if *budget == 0 || self.stop() {
            return None;
        }
        *budget -= 1;
        self.charge_simulation_eval();

        let guide_move = self
            .tt
            .probe(guide.hash())
            .and_then(|entry| (entry.outcome == Some(Outcome::Win)).then_some(entry.best_move));
        let guide_terminal = guide.outcome() == Some(Outcome::Win);
        let mv = match guide_move {
            Some(mv) if mv != Move::NONE => mv,
            _ if guide_terminal => self.immediate_win_move(pos, budget)?,
            _ => return None,
        };

        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);
        if !moves.as_slice().contains(&mv) {
            return None;
        }
        let mut after = pos.clone();
        after.do_move(mv);
        let mut guide_after = guide.clone();
        guide_after.do_move(mv);

        line.push(rep);
        path.push(mv);
        let reached = self.simulate_defender(&after, &guide_after, budget, line, path);
        path.pop();
        line.pop();

        let depth = reached?.depth.saturating_add(1);
        self.emit_proof_node_with_path(path, pos, Outcome::Win, depth);
        Some(SimulatedProof {
            depth,
            best_move: mv,
        })
    }

    /// Defender to move: every legal defence must be answered by the guide's
    /// replay, a known proof, or an immediate explosion. Returns the attacker
    /// proof depth below `pos`.
    fn simulate_defender(
        &mut self,
        pos: &Position,
        guide: &Position,
        budget: &mut u32,
        line: &mut Vec<u64>,
        path: &mut Vec<Move>,
    ) -> Option<SimulatedProof> {
        match pos.outcome() {
            Some(Outcome::Loss) => {
                self.emit_proof_node_with_path(path, pos, Outcome::Loss, 0);
                return Some(SimulatedProof {
                    depth: 0,
                    best_move: Move::NONE,
                });
            }
            Some(_) => return None,
            None => {}
        }
        let rep = pos.repetition_key();
        if self.path_contains(rep) || line.contains(&rep) {
            return None;
        }
        if let Some(entry) = self.tt.probe(pos.hash()).copied()
            && let Some(outcome) = entry.outcome
        {
            if outcome == Outcome::Loss {
                self.emit_proof_node_with_path(path, pos, outcome, entry.depth);
                return Some(SimulatedProof {
                    depth: entry.depth,
                    best_move: entry.best_move,
                });
            }
            return None;
        }
        if *budget == 0 || self.stop() {
            return None;
        }
        *budget -= 1;
        self.charge_simulation_eval();

        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);
        if moves.as_slice().is_empty() {
            return None;
        }
        let mut guide_moves = MoveList::new();
        guide.legal_moves(&mut guide_moves);

        let mut max_depth = 0u32;
        line.push(rep);
        for &reply in moves.as_slice() {
            path.push(reply);
            let mut child = pos.clone();
            child.do_move(reply);
            let child_rep = child.repetition_key();
            // A child that repeats the search path or this replay line is a
            // Draw, even if the TT has a proof for it from another context.
            // Terminal answers cannot be such repetitions (a terminal position
            // is never on a non-terminal path), but the TT answer can.
            if self.path_contains(child_rep) || line.contains(&child_rep) {
                path.pop();
                line.pop();
                return None;
            }
            let child_outcome = child.outcome();
            let answer = match child_outcome {
                Some(Outcome::Win) => {
                    self.emit_proof_node_with_path(path, &child, Outcome::Win, 0);
                    Some(0)
                }
                Some(_) => None,
                None => {
                    if let Some(win) = self.immediate_win_move(&child, budget) {
                        self.emit_immediate_win(&child, win, path);
                        Some(1)
                    } else if let Some(entry) = self.tt.probe(child.hash()).copied()
                        && entry.outcome == Some(Outcome::Win)
                    {
                        self.emit_proof_node_with_path(path, &child, Outcome::Win, entry.depth);
                        Some(entry.depth)
                    } else if guide_moves.as_slice().contains(&reply) {
                        let mut guide_child = guide.clone();
                        guide_child.do_move(reply);
                        self.simulate_attacker(&child, &guide_child, budget, line, path)
                            .map(|proof| proof.depth)
                    } else {
                        None
                    }
                }
            };
            path.pop();
            match answer {
                Some(depth) => max_depth = max_depth.max(depth),
                None => {
                    line.pop();
                    return None;
                }
            }
        }
        line.pop();

        let depth = max_depth.saturating_add(1);
        self.emit_proof_node_with_path(path, pos, Outcome::Loss, depth);
        Some(SimulatedProof {
            depth,
            best_move: Move::NONE,
        })
    }

    /// The first legal move of `pos` after which the opponent is lost (its
    /// commoner is exploded or it is mated). Simulation-local: the search's
    /// child evaluation never calls it.
    fn immediate_win_move(&mut self, pos: &Position, budget: &mut u32) -> Option<Move> {
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);
        for &mv in moves.as_slice() {
            self.charge_simulation_eval();
            let mut child = pos.clone();
            child.do_move(mv);
            if child.outcome() == Some(Outcome::Loss) {
                return Some(mv);
            }
        }
        None
    }

    /// Emit the two nodes an immediate one-ply win proves: the attacker's
    /// child (`Win`, depth 1) and its terminal defender leaf (`Loss`, depth 0).
    fn emit_immediate_win(&self, child: &Position, mv: Move, path: &[Move]) {
        let mut leaf = child.clone();
        leaf.do_move(mv);
        let mut leaf_path = path.to_vec();
        leaf_path.push(mv);
        self.emit_proof_node_with_path(&leaf_path, &leaf, Outcome::Loss, 0);
        self.emit_proof_node_with_path(path, child, Outcome::Win, 1);
    }

    fn charge_simulation_eval(&mut self) {
        self.child_evals += 1;
        self.simulation_evaluations += 1;
    }
}
