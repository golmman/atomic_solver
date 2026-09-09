//! DF-PN child evaluation.
//!
//! This file is larger than 20 KB because `ChildPrecompute` (the pooled
//! frame movegen slots consumed by each `dfpn` entry), `ChildInfo`
//! construction, terminal detection with its cost-ordered fast paths, TT
//! reuse, proof-tree event emission, and the slot-allocator invariants all
//! share the same `Position` move/undo sequence; splitting them would
//! scatter the staleness invariants that make the slot reuse safe.

#![allow(clippy::similar_names)]

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::proof_event::{NodeProven, ProofEvent};

use super::{INF, Search};

/// Movegen slot for one active `dfpn` frame: the legal-move list and node
/// state the frame generates at its own entry and uses for the terminal
/// check, move ordering, and child enumeration.
///
/// Slots live in `Search::precompute_pool`, one per active `dfpn` frame
/// (indexed by the frame's `path_stack.len()` after `path_push`). The entry
/// takes its depth's slot with `mem::take`, clears and regenerates into it
/// (correctness never depends on pooled contents), and returns it at frame
/// exit, so storage is reused warm across frames at the same depth: no
/// allocation, no copy. A frame that exits early (terminal at entry,
/// depth-0 leaf, path repetition, TT-resolved) drops its slot contents; the
/// pool entry keeps a fresh default which the next frame at that depth
/// regenerates.
///
/// # Staleness invariant
///
/// Each active depth owns exactly one slot and DFS assigns depths uniquely
/// among active frames, so no two frames can share a slot. Any future change
/// that lets a frame outlive its slot or access a different depth's slot
/// must revisit this.
pub(super) struct ChildPrecompute {
    pub moves: MoveList,
    pub state: StateInfo,
}

impl ChildPrecompute {
    pub(super) fn new() -> Self {
        Self {
            moves: MoveList::new(),
            state: StateInfo::new(),
        }
    }
}

impl Default for ChildPrecompute {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ChildInfo {
    pub mv: Move,
    pub pn: u64,
    pub dn: u64,
    pub outcome: Option<Outcome>,
    pub depth: u32,
    pub repetition_seen: bool,
    pub explored: bool,
}

#[derive(Clone, Copy)]
pub struct ChildSelection {
    pub best_child: (Move, u64, u64),
    pub second_child: (u64, u64),
    pub best_child_index: Option<usize>,
    pub pn: u64,
    pub dn: u64,
    pub depth: u32,
    pub best_move: Move,
    pub solved_outcome: Option<Outcome>,
    pub repetition_seen: bool,
}

impl Search {
    /// Evaluate every legal move and fill `children` with a fresh `ChildInfo`
    /// table (the caller owns the pooled vector).
    ///
    /// A single child `Loss` is enough to prove the parent is a win for the
    /// side to move, so we can stop evaluating the remaining children once a
    /// winning child is found; the remaining entries are filled with dummy
    /// unexplored children. A Loss parent requires all children to be solved.
    ///
    /// The terminal check per child is an upstream early-exit existence
    /// query (`Position::has_legal_move`); no move list is generated here —
    /// each searched child regenerates its own moves at its `dfpn` entry.
    pub(super) fn evaluate_all_children(
        &mut self,
        pos: &mut Position,
        moves: &MoveList,
        max_depth: u32,
        is_or_node: bool,
        children: &mut Vec<ChildInfo>,
    ) {
        children.clear();
        for i in 0..moves.len() {
            let mv = moves[i];
            let info = self.evaluate_child(pos, mv, max_depth, is_or_node);
            let decisive = info.outcome == Some(Outcome::Loss);
            children.push(info);
            if decisive {
                for j in (i + 1)..moves.len() {
                    children.push(ChildInfo {
                        mv: moves[j],
                        pn: INF,
                        dn: 0,
                        outcome: None,
                        depth: 0,
                        repetition_seen: false,
                        explored: false,
                    });
                }
                break;
            }
        }
    }

    pub(super) fn evaluate_child(
        &mut self,
        pos: &mut Position,
        mv: Move,
        max_depth: u32,
        is_or_node: bool,
    ) -> ChildInfo {
        self.child_evals += 1;
        pos.do_move(mv);
        let child_key = pos.hash();
        let child_rep_key = pos.repetition_key();
        let child_is_or = !is_or_node;

        // Terminal-check fast path, reordered for cost (result-equivalent):
        // the commoner-extinction branches strictly precede the moves-empty
        // branch in `outcome_from_state`, so they can be evaluated without
        // movegen. The repetition check keeps its place before any TT use
        // (first-player-loss GHI shortcut) but only fires when the halfmove
        // clock has *not* expired: the repetition key ignores rule50, while
        // the old code's `outcome()` ran before the repetition check, so a
        // rule50-expired child was always reported as a rule50 terminal
        // (checkmate still outranks rule50 and needs movegen) with
        // `repetition_seen = false` and no TT probe.
        let rule50_expired = pos.board().rule50() >= 100;
        let info = if pos.commoners(pos.side_to_move()).is_empty() {
            ChildInfo {
                mv,
                pn: Outcome::Loss.pn_dn_for(child_is_or).0,
                dn: Outcome::Loss.pn_dn_for(child_is_or).1,
                outcome: Some(Outcome::Loss),
                depth: 0,
                repetition_seen: false,
                explored: false,
            }
        } else if pos.commoners(pos.side_to_move().flip()).is_empty() {
            ChildInfo {
                mv,
                pn: Outcome::Win.pn_dn_for(child_is_or).0,
                dn: Outcome::Win.pn_dn_for(child_is_or).1,
                outcome: Some(Outcome::Win),
                depth: 0,
                repetition_seen: false,
                explored: false,
            }
        } else if !rule50_expired && self.path_contains(child_rep_key) {
            ChildInfo {
                mv,
                pn: Outcome::Draw.pn_dn_for(child_is_or).0,
                dn: Outcome::Draw.pn_dn_for(child_is_or).1,
                outcome: Some(Outcome::Draw),
                depth: 0,
                repetition_seen: true,
                explored: false,
            }
        } else if rule50_expired {
            // rule50 has expired: the child is terminal unless it is also
            // checkmate/stalemate (the moves-empty branch outranks rule50, so
            // the existence query decides). The early-exit existence query
            // replaces the full generation: any legal move ⇒ the rule50 draw
            // (which outranks the occupied == 2 branch of
            // `outcome_from_state`); no legal move ⇒ checkmate (Loss) vs
            // stalemate (Draw) via the checkers bit. Always `Some` here,
            // matching the old code where `outcome()` concluded the child
            // before repetition and TT were consulted.
            let mut state = StateInfo::new();
            pos.populate_state(&mut state);
            let outcome = if pos.has_legal_move(&state) || state.checkers.is_empty() {
                Outcome::Draw
            } else {
                Outcome::Loss
            };
            let (pn, dn) = outcome.pn_dn_for(child_is_or);
            ChildInfo {
                mv,
                pn,
                dn,
                outcome: Some(outcome),
                depth: 0,
                repetition_seen: false,
                explored: false,
            }
        } else {
            let child_max_depth = max_depth.saturating_sub(1);
            // Single TT probe for this child: the copied entry (TtEntry is
            // Copy) feeds both the solved-result reuse (depth checks + one-ply
            // guard) and the unsolved-bounds reuse below. Solved results are
            // position-static (terminality is deterministic per hash), so a
            // TT-resolved hit is identical to the terminal path that used to
            // run before the probe.
            let entry = self.tt.probe(child_key).copied();
            let mut resolved = entry
                .as_ref()
                .and_then(|e| Self::resolved_from_entry(e, child_max_depth));
            if let Some(e) = entry.as_ref()
                && resolved.is_some()
                && e.best_move != Move::NONE
                && self.best_move_repeats_path(pos, e.best_move)
            {
                resolved = None;
            }

            if let Some(resolved) = resolved {
                let (pn, dn) = resolved.outcome.pn_dn_for(child_is_or);
                ChildInfo {
                    mv,
                    pn,
                    dn,
                    outcome: Some(resolved.outcome),
                    depth: resolved.depth,
                    repetition_seen: false,
                    explored: false,
                }
            } else {
                // Still undecided: the terminal check is now an early-exit
                // existence query (upstream `has_legal_move_with_state`) fed
                // by a caller-populated `StateInfo` — no move list is
                // generated here. Classification mirrors `outcome_from_state`
                // precedence: moves-empty (checkmate/stalemate via the
                // checkers bit) first, then the board-static occupied == 2
                // draw; the extinction and rule50 branches already returned.
                let mut state = StateInfo::new();
                pos.populate_state(&mut state);
                let terminal_outcome = if !pos.has_legal_move(&state) {
                    // No legal move: checkmate (Loss) vs stalemate (Draw).
                    Some(if state.checkers.is_empty() {
                        Outcome::Draw
                    } else {
                        Outcome::Loss
                    })
                } else if pos.board().occupied().count() == 2 {
                    // `occupied == 2` follows the moves-empty branch in
                    // `outcome_from_state`, so a K-vs-K child with legal
                    // moves is a draw without any move list.
                    Some(Outcome::Draw)
                } else {
                    None
                };
                if let Some(outcome) = terminal_outcome {
                    let (pn, dn) = outcome.pn_dn_for(child_is_or);
                    ChildInfo {
                        mv,
                        pn,
                        dn,
                        outcome: Some(outcome),
                        depth: 0,
                        repetition_seen: false,
                        explored: false,
                    }
                } else {
                    // Unsolved bounds are only consulted after the movegen-based
                    // terminal check says non-terminal, like the old code where
                    // the outcome check preceded the TT probe. Only reuse bounds
                    // when they are non-degenerate: a previous work-bounded
                    // search may have stored a candidate terminal-like bound
                    // (pn == 0 or dn == 0) without an outcome, and propagating
                    // such values can trick the parent search into treating an
                    // unproven node as solved. Fall back to neutral (1, 1).
                    let (pn, dn) = match entry.as_ref() {
                        Some(e)
                            if e.outcome.is_none()
                                && e.pn > 0
                                && e.dn > 0
                                && e.remaining_depth != u32::MAX
                                && e.remaining_depth <= child_max_depth =>
                        {
                            (e.pn, e.dn)
                        }
                        _ => (1, 1),
                    };
                    ChildInfo {
                        mv,
                        pn,
                        dn,
                        outcome: None,
                        depth: 0,
                        repetition_seen: false,
                        explored: false,
                    }
                }
            }
        };

        if let Some(sender) = &self.proof_event_sender
            && let Some(outcome) = info.outcome
            && outcome != Outcome::Draw
        {
            let mut path = self.move_stack.clone();
            path.push(mv);
            let _ = sender.send(ProofEvent::NodeProven(NodeProven::new(
                path,
                pos.hash(),
                outcome,
                info.depth,
            )));
        }

        pos.undo_move(mv);
        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;
    use atomic_movegen::board::StateInfo;
    use atomic_movegen::types::Square;

    #[test]
    fn evaluate_child_terminal_win_for_parent() {
        // White rook can capture the black commoner on e8.
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        let mv = Move::make_move(Square::E1, Square::E8);
        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);

        assert_eq!(
            info.outcome,
            Some(Outcome::Loss),
            "capturing the commoner ends the game"
        );
        assert_eq!(
            info.pn, 0,
            "terminal winning child has pn 0 at the AND side"
        );
        assert_eq!(
            info.dn, INF,
            "terminal winning child has dn INF at the AND side"
        );
    }

    #[test]
    fn evaluate_child_repetition_is_draw() {
        let mut search = Search::new(1);
        // Build a non-terminal position whose move leads to a position that is
        // already on the search path by pushing the child's repetition key before
        // evaluating it.
        let mut pos = Position::from_fen("4k3/8/8/4K3/8/8/8/4R3 w - - 0 1").unwrap();

        // Move the white king from e5 to d5; it is quiet and non-terminal.
        let mv = Move::make_move(Square::E5, Square::D5);
        pos.do_move(mv);
        let child_rep_key = pos.repetition_key();
        pos.undo_move(mv);
        search.path_stack.push(child_rep_key);

        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);

        assert_eq!(
            info.outcome,
            Some(Outcome::Draw),
            "repeating position is a draw"
        );
        assert!(info.repetition_seen, "repetition flag should be set");
    }

    #[test]
    fn evaluate_child_rule50_expired_repetition_is_not_repetition_seen() {
        // The repetition key ignores the halfmove clock, so a rule50-expired
        // child can repeat a position on the search path. `outcome()` ran
        // before the repetition check in the original code, so such a child
        // was always reported as a rule50 terminal draw with
        // `repetition_seen = false`; the reordered terminal check must keep
        // that semantics (the GHI draw-suppression and selection tie-breaks
        // depend on the flag).
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/4K3/8/8/8/4R3 w - - 100 1").unwrap();

        let mv = Move::make_move(Square::E5, Square::D5);
        pos.do_move(mv);
        let child_rep_key = pos.repetition_key();
        pos.undo_move(mv);
        search.path_stack.push(child_rep_key);

        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);

        assert_eq!(info.outcome, Some(Outcome::Draw), "rule50 draw");
        assert_eq!(info.depth, 0, "rule50 terminal is depth 0");
        assert!(
            !info.repetition_seen,
            "rule50 terminal outranks the repetition check, flag must stay false"
        );
    }

    #[test]
    fn evaluate_child_rule50_checkmate_is_loss() {
        // A rule50-expired child that is also checkmate is a Loss: the
        // moves-empty branch outranks rule50, so the fast path must not
        // conclude a Draw before movegen. White plays Qb3-b2, delivering
        // mate to the black commoner on a1 with the halfmove clock reaching
        // 100 (same terminal position as the `fifty_move_checkmate_is_loss`
        // position test, reached as a child).
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("7K/8/8/8/8/1Q6/8/k7 w - - 99 1").unwrap();
        let mv = Move::make_move(Square::B3, Square::B2);
        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
        assert_eq!(
            info.outcome,
            Some(Outcome::Loss),
            "rule50 checkmate must stay a Loss"
        );
        assert_eq!(info.depth, 0);
        assert!(!info.repetition_seen);
    }

    #[test]
    fn evaluate_child_uses_solved_tt_entry() {
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
        // g1h1 is a quiet rook move to a non-terminal child.
        let mv = Move::make_move(Square::G1, Square::H1);
        pos.do_move(mv);
        let child_key = pos.hash();
        pos.undo_move(mv);

        // Store a solved Loss for the child position. The remaining-depth guard
        // requires remaining_depth == u32::MAX for an unbounded max_depth.
        search.tt.store(
            child_key,
            Move::NONE,
            u8::MAX,
            0,
            Some(Outcome::Loss),
            INF,
            0,
            1,
            u32::MAX,
        );

        // Force the TT lookup without any legal-move generation at the child.
        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
        assert_eq!(
            info.outcome,
            Some(Outcome::Loss),
            "TT-solved Loss should be reused"
        );
        assert_eq!(info.depth, 1, "TT depth should be preserved");
    }

    #[test]
    fn evaluate_all_children_stops_at_winning_child() {
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        let mut moves = MoveList::new();
        let mut state = StateInfo::new();
        pos.legal_moves_with_state(&mut moves, &mut state);

        // Sort so the winning capture e1e8 is tried first.
        search.sort_moves(&pos, &state, &mut moves, Move::NONE, true);
        let mut children = Vec::new();
        search.evaluate_all_children(&mut pos, &moves, u32::MAX, true, &mut children);

        assert_eq!(children.len(), moves.len());
        assert!(
            children[0].outcome == Some(Outcome::Loss),
            "first child should be the winning capture"
        );

        // Remaining children were filled with dummy unexplored entries.
        for c in &children[1..] {
            assert_eq!(c.pn, INF);
            assert_eq!(c.dn, 0);
            assert_eq!(c.outcome, None);
            assert!(!c.explored);
        }
    }

    #[test]
    fn evaluate_child_checkmate_child_classifies_loss_without_movegen() {
        // White plays Qb3-b2, delivering mate to the black commoner on a1.
        // The boolean existence query finds no legal move and the checkers
        // bit classifies the child as checkmate (Loss) — same terminal
        // position as the `fifty_move_checkmate_is_loss` position test,
        // reached as a child.
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("7K/8/8/8/8/1Q6/8/k7 w - - 0 1").unwrap();
        let mv = Move::make_move(Square::B3, Square::B2);
        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
        assert_eq!(info.outcome, Some(Outcome::Loss), "checkmate is a Loss");
        assert_eq!(info.depth, 0);
        assert!(!info.repetition_seen);
    }

    #[test]
    fn evaluate_child_stalemate_child_classifies_draw_without_movegen() {
        // Black plays Qc3-c2, stalemat-ing the white commoner on a1: no
        // legal move and empty checkers must classify as stalemate (Draw).
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("7k/8/8/8/8/2q5/8/K7 b - - 0 1").unwrap();
        let mv = Move::make_move(Square::C3, Square::C2);
        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
        assert_eq!(info.outcome, Some(Outcome::Draw), "stalemate is a Draw");
        assert_eq!(info.depth, 0);
        assert!(!info.repetition_seen);
    }

    #[test]
    fn evaluate_child_two_commoners_is_draw() {
        // A K-vs-K child (occupied == 2) with legal moves and rule50 < 100
        // must be classified as a Draw via the board-static occupied == 2
        // branch, which follows the moves-empty branch in
        // `outcome_from_state` — guarding the precedence added with the
        // existence-query terminal check. White plays a1-a2; the child is
        // black to move with the two adjacent commoners.
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("8/8/8/8/8/8/1k6/K7 w - - 0 1").unwrap();
        let mv = Move::make_move(Square::A1, Square::A2);
        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
        assert_eq!(
            info.outcome,
            Some(Outcome::Draw),
            "K vs K with legal moves is a draw"
        );
        assert_eq!(info.depth, 0);
        assert!(!info.repetition_seen);
    }

    #[test]
    fn evaluate_child_unsolved_tt_bounds_used_when_non_degenerate() {
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
        // g1h1 is a quiet rook move to a non-terminal child.
        let mv = Move::make_move(Square::G1, Square::H1);
        pos.do_move(mv);
        let child_key = pos.hash();
        pos.undo_move(mv);

        // Store non-degenerate unsolved bounds with enough remaining depth.
        // max_depth=11 gives child_max_depth=10, so remaining_depth=10 is usable.
        search
            .tt
            .store(child_key, Move::NONE, u8::MAX, 1, None, 5, 5, 0, 10);

        let info = search.evaluate_child(&mut pos, mv, 11, true);
        assert_eq!(
            info.outcome, None,
            "unsolved bounds should not report an outcome"
        );
        assert_eq!(info.pn, 5);
        assert_eq!(info.dn, 5);
    }

    #[test]
    fn evaluate_child_degenerate_tt_bounds_fall_back_to_neutral() {
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
        // g1h1 is a quiet rook move to a non-terminal child.
        let mv = Move::make_move(Square::G1, Square::H1);
        pos.do_move(mv);
        let child_key = pos.hash();
        pos.undo_move(mv);

        // pn == 0 is degenerate and should not be reused as an unsolved bound.
        search
            .tt
            .store(child_key, Move::NONE, u8::MAX, 1, None, 0, 5, 0, 10);

        let info = search.evaluate_child(&mut pos, mv, 11, true);
        assert_eq!(info.outcome, None);
        assert_eq!(info.pn, 1);
        assert_eq!(info.dn, 1);
    }

    #[test]
    fn dfpn_entry_generates_into_pooled_slot() {
        // The frame-local slot lifecycle: each `dfpn` entry generates its own
        // legal moves into the pool slot at its depth and returns it at frame
        // exit. After a bounded search, the root slot (depth 1, no prefix)
        // holds the root position's legal-move list, a searched child's slot
        // (depth 2) is filled, and index 0 (never a frame depth) stays empty
        // — no parent-level pre-fill happens anywhere.
        let fen = "r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35";
        let mut search = Search::new(1);
        let mut pos = Position::from_fen(fen).unwrap();
        let (outcome, _, _) = search.search_depth(&mut pos, 2);
        assert_eq!(outcome, Outcome::Draw, "depth-2 bound is not decisive");

        let mut fresh = MoveList::new();
        pos.legal_moves(&mut fresh);
        assert!(
            search.precompute_pool.len() >= 3,
            "root and child frames allocated their slots"
        );
        assert_eq!(
            search.precompute_pool[0].moves.len(),
            0,
            "depth 0 is never a frame depth"
        );
        let root_slot = &search.precompute_pool[1].moves;
        assert_eq!(
            root_slot.len(),
            fresh.len(),
            "the root frame's slot holds the root position's legal moves"
        );
        assert!(
            fresh
                .as_slice()
                .iter()
                .all(|m| root_slot.as_slice().contains(m)),
            "the root frame's slot covers every legal move (order differs: the frame sorts in place)"
        );
        assert!(
            !search.precompute_pool[2].moves.is_empty(),
            "a searched child's frame filled its own slot"
        );
    }

    #[test]
    fn dfpn_frame_local_slots_deterministic() {
        // Behavioral check: the frame-local slot plumbing must not perturb
        // the search — solving the same position twice with fresh `Search`
        // objects yields identical work counters (and catches pool state
        // leaking through the slot lifecycle).
        let fen = "r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35";
        let mut counts = Vec::new();
        for _ in 0..2 {
            let mut search = Search::new(1);
            let mut pos = Position::from_fen(fen).unwrap();
            let (outcome, pv, _) = search.solve(&mut pos);
            counts.push((outcome, pv.len(), search.child_evaluations()));
        }
        assert_eq!(counts[0], counts[1], "identical solve, identical counts");
        assert_eq!(counts[0].0, Outcome::Loss, "dec44 is a loss for black");
    }

    #[test]
    fn precompute_pool_stays_within_active_depth_bound() {
        // Memory bound: with one slot per active depth (branching no longer
        // multiplies the pool, as in the plan2-era per-child slots), the
        // pooled movegen storage after a full solve must stay far below the
        // few-MB budget that plan2 documented for m22-scale searches. Note
        // the first-outcome phase searches unbounded depth, so the pool
        // reflects the deepest recursion ever reached, not the last chunk's
        // `max_depth_reached`.
        let fen = "r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35";
        let mut search = Search::new(1);
        let mut pos = Position::from_fen(fen).unwrap();
        let _ = search.solve(&mut pos);
        let pool_len = search.precompute_pool.len();
        let pool_bytes = pool_len * std::mem::size_of::<ChildPrecompute>();
        assert!(
            pool_bytes < 1_000_000,
            "pooled movegen storage must stay well under the old per-child budget, got {pool_bytes} bytes"
        );
    }
}
