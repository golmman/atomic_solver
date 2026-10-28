//! DF-PN child evaluation.
//!
//! This file is larger than 20 KB because `ChildPrecompute` (the pooled
//! frame movegen slots consumed by each `dfpn` entry), `ChildInfo`
//! construction, terminal detection with its cost-ordered fast paths, TT
//! reuse, the TT pre-probe, proof-tree event emission, and the slot-allocator
//! invariants all share the same `Position` move/undo sequence; splitting them
//! would scatter the staleness invariants that make the slot reuse safe.
//!
//! # TT pre-probe (performance plan3)
//!
//! `evaluate_all_children` computes every child's `(tt_key, repetition_key,
//! rule50)` triple up front with the non-mutating [`Position::keys_after`] (in
//! the same pass that issues the TT prefetches), and `evaluate_child_keyed`
//! consults the TT *before* the child move is played: a live outcome-less
//! entry, or a solved entry with no best-move hint, decides the child, so the
//! make/unmake, the terminal existence query and a second probe are skipped
//! entirely. Everything else falls back to the shipped make/unmake path, which
//! reuses the entry the pre-probe already read (exactly one probe per child);
//! hinted solved entries reuse it there before the terminal classification.
//! The equivalence argument and its load-bearing outcome-less non-terminal
//! guarantee are documented at the `evaluate_child_keyed` ladder and at
//! `evaluate_child_played`.

#![allow(clippy::similar_names)]

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::proof_event::{NodeProven, ProofEvent};

use super::Search;
use crate::search::tt::TtEntry;
use crate::zobrist::INF;

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
///
/// # Eval-scratch slots
///
/// Besides the frame slot above, each depth also owns one make/unmake scratch
/// `StateInfo` in `Search::eval_state_pool` (`eval_state_pool[frame_depth]`),
/// used by `evaluate_child` for the
/// `do_move_with_scratch`/`undo_move_with_scratch` pair: the slot is
/// initialized once with `StateInfo::new()` and reused dirty (soundness:
/// `Board::do_move` writes every field `Board::undo_move` reads, and the undo
/// stack is not touched — see `Position::do_move_with_scratch`). The slot is
/// distinct from the frame slot's `state` (which feeds the parent's
/// movegen/`sort_moves`) and from the local existence-check `StateInfo`
/// inside `evaluate_child`; `evaluate_child` never recurses, so one slot per
/// depth cannot alias an in-flight call. Any change that makes child
/// evaluation recursive must revisit this. (Since lean plan12 the
/// TT-prefetch pre-pass in `evaluate_all_children` uses the non-mutating
/// `Position::keys_after` and holds no `StateInfo` at all; performance plan3
/// made that same pre-pass supply the pre-probe keys through
/// `Search::keys_scratch`.)
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
        // Keys pre-pass (lean plan11's TT prefetch, extended by performance
        // plan3): one non-mutating `Position::keys_after` per child both issues
        // the child's bucket prefetch — every load before the first probe, so
        // the misses overlap instead of serializing — and supplies the
        // pre-probe keys, at the same cost as the throwaway `hash_after`
        // version it replaces. `keys_scratch` only grows and its
        // `0..moves.len()` prefix is written before it is read; entries past
        // the prefix are stale-but-never-read.
        let child_count = moves.len();
        if self.keys_scratch.len() < child_count {
            self.keys_scratch.resize(child_count, (0, 0, 0));
        }
        for i in 0..child_count {
            let keys = pos.keys_after(moves[i]);
            self.tt.prefetch(keys.0);
            self.keys_scratch[i] = keys;
        }
        for i in 0..child_count {
            let mv = moves[i];
            let (child_key, child_rep_key, rule50_after) = self.keys_scratch[i];
            let info = self.evaluate_child_keyed(
                pos,
                mv,
                max_depth,
                is_or_node,
                child_key,
                child_rep_key,
                rule50_after,
            );
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

    /// Evaluate one child whose keys the caller does not have at hand (the
    /// best-child re-evaluation in `core.rs` and the unit tests).
    ///
    /// `evaluate_all_children` skips this wrapper: its keys pre-pass already
    /// computed the triple and calls [`Search::evaluate_child_keyed`] directly.
    pub(super) fn evaluate_child(
        &mut self,
        pos: &mut Position,
        mv: Move,
        max_depth: u32,
        is_or_node: bool,
    ) -> ChildInfo {
        let (child_key, child_rep_key, rule50_after) = pos.keys_after(mv);
        self.evaluate_child_keyed(
            pos,
            mv,
            max_depth,
            is_or_node,
            child_key,
            child_rep_key,
            rule50_after,
        )
    }

    /// Keyed form of [`Search::evaluate_child`]: the child's
    /// `(tt_key, repetition_key, rule50)` come from the caller, so the TT
    /// pre-probe runs **before** the child move is ever played.
    ///
    /// # The pre-probe ladder (performance plan3)
    ///
    /// The branches run in the shipped order with the make/unmake hoisted out
    /// of the way; each one reads the same inputs a played child would:
    ///
    /// 1. a path repetition (with rule50 not expired) is a `Draw` with
    ///    `repetition_seen = true`. This stays *before* the TT probe: the
    ///    shipped code classifies a path repetition as a Draw regardless of
    ///    what the table holds (first-player-loss GHI shortcut);
    /// 2. a live current-generation entry decides the child from the entry
    ///    alone when it is outcome-less (reused bounds under the shipped
    ///    degeneracy rule) or solved with no best-move hint. A hinted solved
    ///    entry falls through: the one-ply guard needs the child board, so it
    ///    is evaluated on the played path (which also reuses the entry before
    ///    the terminal classification there);
    /// 3. everything else falls through to
    ///    [`Search::evaluate_child_played`], which reuses the entry this probe
    ///    already read: one TT probe per child, never two (nothing between the
    ///    pre-probe and the played path can modify the table).
    ///
    /// # Why steps 1-2 may skip the terminal classification
    ///
    /// They skip the two commoner-extinction checks, the `occupied == 2` check
    /// and the legal-move existence query. Two different guarantees cover the
    /// two entry shapes:
    ///
    /// - **Outcome-less entries.** Every store site that writes
    ///   `outcome == None` is on a path that already established the node is
    ///   non-terminal (the depth-0 leaf store and the GHI-draw-suppressed store
    ///   in `core.rs`; the snapshot import preserves the same shape). Such a
    ///   child has legal moves, `rule50 < 100` and `occupied > 2`, so the
    ///   extinction checks, the `occupied == 2` check and the existence query
    ///   are redundant.
    /// - **Solved entries.** The stored outcome was computed for this exact
    ///   key; terminality is position-static, so replaying the terminal
    ///   classification cannot yield a different result. (The terminal store in
    ///   `core.rs` writes exactly such an authoritative outcome; it is why this
    ///   invariant is stated for *outcome-less* entries only.) A hinted solved
    ///   entry is necessarily non-terminal because terminal stores write
    ///   `Move::NONE` as the hint, which is also why its played-path shortcut
    ///   may return before the existence query.
    ///
    /// Step 2 stays gated on `!rule50_expired` because the shipped order puts
    /// the rule50 branch before any TT use; that is an ordering constraint, not
    /// a claim that a rule50-expired entry cannot exist. Store sites any future
    /// change must revisit: `core.rs`'s terminal store, its depth-0 leaf store,
    /// its GHI-draw-suppressed outcome-less (`(1, 1)`) store, and the snapshot
    /// import in `src/reconstruct`.
    ///
    /// Were a future outcome-less store site to violate the non-terminal
    /// guarantee, the fast path could return reused bounds for a terminal
    /// child; for solved entries the failure mode is a `child_evals` drift
    /// caught by the trajectory tests, not a wrong answer.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn evaluate_child_keyed(
        &mut self,
        pos: &mut Position,
        mv: Move,
        max_depth: u32,
        is_or_node: bool,
        child_key: u64,
        child_rep_key: u64,
        rule50_after: u16,
    ) -> ChildInfo {
        self.child_evals += 1;
        let child_is_or = !is_or_node;
        // The repetition key ignores rule50, while the shipped code's
        // `outcome()` ran before the repetition check: a rule50-expired child
        // (checkmate still outranks rule50 and needs movegen) is always
        // reported as a rule50 terminal with `repetition_seen = false` and no
        // TT probe. `rule50_after` is the raw clock, not the clamped key.
        let rule50_expired = rule50_after >= 100;

        let info = if !rule50_expired && self.path_contains(child_rep_key) {
            // Step 1: path repetition (see the ladder docs above).
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
            // A rule50 terminal needs the child's existence query, so it takes
            // the played path, whose rule50 branch returns before any TT use
            // (as shipped) — hence `entry: None`.
            self.evaluate_child_played(pos, mv, max_depth, child_is_or, true, child_key, None)
        } else {
            // Step 2: the single pre-probe for this child. The copied entry
            // (`TtEntry` is `Copy`) feeds both the solved-result reuse and the
            // reuse on the played path.
            let entry: Option<TtEntry> = self.tt.probe(child_key).copied();
            let child_max_depth = max_depth.saturating_sub(1);
            // Step 2a: a solved entry *without* a best move. It needs no
            // one-ply guard, so it is fully decidable from the key alone.
            //
            // A solved entry *with* a best move is deliberately NOT decided
            // here, even though the entry is at hand: the shipped one-ply
            // guard asks whether playing that move from the **child** lands on
            // a path entry, and that grandchild repetition key needs the child
            // board — upstream offers only one-ply `hash_after`, so the second
            // ply is reachable only by playing the move. Applying
            // `repetition_key_after` to the parent instead is *not* the same
            // key (it hashes an illegal move from the wrong base) and was
            // measured to fire the guard spuriously, diverging from the shipped
            // trajectory within a few thousand child evaluations. Those
            // children therefore take the played path, which runs the shipped
            // guard verbatim.
            let resolved_without_hint = entry
                .as_ref()
                .filter(|e| e.best_move == Move::NONE)
                .and_then(|e| Self::resolved_from_entry(e, child_max_depth));

            if let Some(resolved) = resolved_without_hint {
                #[cfg(test)]
                {
                    self.preprobe_skips += 1;
                }
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
            } else if entry.as_ref().is_some_and(|e| e.outcome.is_none()) {
                // #12a TT non-terminal skip, now decided *before* the move is
                // played: a live entry with `outcome == None` was only ever
                // stored by a non-terminal store site (`core.rs`'s depth-0
                // leaf store and GHI-draw-suppressed store; the terminal store
                // writes `Some`), so the position had legal moves,
                // `rule50 < 100` and `occupied > 2` — the child is non-terminal
                // by the invariant restated at the ladder docs, and the
                // existence query plus `occupied == 2` recheck can be skipped.
                // Deliberately *not* keyed off `remaining_depth` shape; the
                // degeneracy guard below already excludes suppressed
                // (`remaining_depth == u32::MAX`) entries.
                #[cfg(test)]
                {
                    self.preprobe_skips += 1;
                }
                // Only reuse bounds when they are non-degenerate: a previous
                // work-bounded search may have stored a candidate terminal-like
                // bound (pn == 0 or dn == 0) without an outcome, and
                // propagating such values can trick the parent search into
                // treating an unproven node as solved. Fall back to neutral
                // (1, 1).
                let (pn, dn) = match entry.as_ref() {
                    Some(e)
                        if e.pn > 0
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
            } else {
                // Everything else needs the played child: a solved entry with a
                // best move (step 2a's guard note), a guarded or depth-guard
                // miss, and the no-entry case. The played path runs the shipped
                // classification verbatim, with the pre-probe's entry reused
                // instead of a second probe.
                self.evaluate_child_played(
                    pos,
                    mv,
                    max_depth,
                    child_is_or,
                    false,
                    child_key,
                    entry.as_ref(),
                )
            }
        };

        // Proof-event stream, byte-identical to the played path's: the shipped
        // emission used `pos.hash()` after the make, which is `child_key` by
        // the `keys_after` contract. Draw children emit nothing.
        if let Some(outcome) = info.outcome
            && outcome != Outcome::Draw
            && let Some(sender) = &self.proof_event_sender
        {
            let mut path = self.move_stack.clone();
            path.push(mv);
            let _ = sender.send(ProofEvent::NodeProven(NodeProven::new(
                path, child_key, outcome, info.depth,
            )));
        }

        info
    }

    /// The make/unmake path of child evaluation: the shipped branch structure,
    /// entered only when the pre-probe ladder could not decide the child.
    ///
    /// `entry` is the entry the ladder already read for `child_key` — never
    /// re-probed here. `rule50_expired` reports the caller's rule50 check, in
    /// which case `entry` is `None` because the rule50 branch precedes any TT
    /// use.
    ///
    /// Hinted solved entries reach this path because the pre-probe cannot
    /// evaluate the one-ply best-move guard without the child board. Once the
    /// child is made, an authoritative live entry is returned *before* the
    /// terminal classification (the entry's outcome was computed for this
    /// exact key); only the guard can suppress it, and a guard hit falls
    /// through to the shipped path. The out-of-line fallback call itself cost
    /// ~1.5% wall on the transposition-dense cell, so the function is
    /// `inline(always)` into the ladder (measured in performance plan3).
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn evaluate_child_played(
        &mut self,
        pos: &mut Position,
        mv: Move,
        max_depth: u32,
        child_is_or: bool,
        rule50_expired: bool,
        child_key: u64,
        entry: Option<&TtEntry>,
    ) -> ChildInfo {
        // Per-frame pooled make/unmake scratch (see the `ChildPrecompute`
        // "Eval-scratch slots" docs): skips the fresh `StateInfo::new()`
        // zeroing and the 64-byte undo-stack push/pop on every evaluated
        // child. Distinct from the frame slot's `state` and from the local
        // existence-check states below; child evaluation never recurses.
        let scratch_depth = self.path_stack.len();
        if self.eval_state_pool.len() <= scratch_depth {
            self.eval_state_pool
                .resize_with(scratch_depth + 1, StateInfo::new);
        }
        let mut scratch = std::mem::take(&mut self.eval_state_pool[scratch_depth]);
        pos.do_move_with_scratch(mv, &mut scratch);
        // The pre-probe keys must describe exactly this position; a divergence
        // would silently change which children the fast path decides.
        debug_assert_eq!(child_key, pos.hash(), "pre-probe key drift");

        // A live solved entry is authoritative for this exact key: every store
        // site only writes an outcome it computed for the position, so the
        // terminal classification below is redundant for it. The pre-probe
        // already takes this shortcut for hintless entries; hinted solved
        // entries reach this path, where the shipped one-ply guard needs the
        // child board. Evaluate the guard after the make and, when it does not
        // fire, return the entry before the existence query. If it does fire,
        // fall through to the shipped path unchanged (which degenerates the
        // entry to neutral bounds).
        if let Some(e) = entry
            && let Some(resolved) = Self::resolved_from_entry(e, max_depth.saturating_sub(1))
            && (e.best_move == Move::NONE || !self.best_move_repeats_path(pos, e.best_move))
        {
            let (pn, dn) = resolved.outcome.pn_dn_for(child_is_or);
            let info = ChildInfo {
                mv,
                pn,
                dn,
                outcome: Some(resolved.outcome),
                depth: resolved.depth,
                repetition_seen: false,
                explored: false,
            };
            pos.undo_move_with_scratch(mv, &scratch);
            self.eval_state_pool[scratch_depth] = scratch;
            return info;
        }

        // Terminal-check fast path, reordered for cost (result-equivalent):
        // the commoner-extinction branches strictly precede the moves-empty
        // branch in `outcome_from_state`, so they can be evaluated without
        // movegen. The repetition branch is not repeated here: the ladder
        // already returned for path repetitions (same key, same path stack),
        // and the repeated position is non-terminal, so neither extinction
        // branch can fire for it.
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
            // The shipped solved-result reuse, guard included: the one-ply
            // best-move guard plays `best_move` from the *child* and undoes it
            // (`best_move_repeats_path`). Solved results are position-static
            // (terminality is deterministic per key), so a TT-resolved hit is
            // identical to the terminal path that used to run before the probe.
            let mut resolved = entry.and_then(|e| Self::resolved_from_entry(e, child_max_depth));
            if let Some(e) = entry
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
                // #12a TT non-terminal skip: a live entry with `outcome ==
                // None` was only ever stored by a non-terminal store site
                // (`core.rs` depth-0 leaf and GHI-draw-suppressed stores; the
                // terminal store writes `Some`), which means the position had
                // legal moves, `rule50 < 100`, and `occupied > 2`. The
                // extinction and rule50 branches above already returned, so
                // the existence query + `occupied == 2` recheck can be
                // skipped: the child is non-terminal by the invariant
                // (restated at the ladder docs). Unreachable from the
                // pre-probe ladder — it decides every live outcome-less entry
                // before the make — but kept verbatim so this path stays the
                // shipped code.
                let terminal_outcome = if entry.is_some_and(|e| e.outcome.is_none()) {
                    None
                } else {
                    // Still undecided: the terminal check is now an early-exit
                    // existence query (upstream `has_legal_move_with_state`)
                    // fed by a caller-populated `StateInfo` — no move list is
                    // generated here. Classification mirrors
                    // `outcome_from_state` precedence: moves-empty
                    // (checkmate/stalemate via the checkers bit) first, then
                    // the board-static occupied == 2 draw; the extinction and
                    // rule50 branches already returned.
                    let mut state = StateInfo::new();
                    pos.populate_state(&mut state);
                    if !pos.has_legal_move(&state) {
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
                    }
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

        pos.undo_move_with_scratch(mv, &scratch);
        self.eval_state_pool[scratch_depth] = scratch;
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
        search.path_push(child_rep_key);

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
        search.path_push(child_rep_key);

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

    // ---- performance plan3: the TT pre-probe ladder ------------------------

    /// Fixture for the pre-probe tests: a quiet, non-terminal child whose move
    /// is a plain rook step.
    const PREPROBE_FEN: &str = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";

    fn preprobe_fixture() -> (Position, Move) {
        let pos = Position::from_fen(PREPROBE_FEN).unwrap();
        let mv = Move::make_move(Square::G1, Square::H1);
        (pos, mv)
    }

    /// `ChildInfo` has no `PartialEq`; compare the production fields one by
    /// one so a divergence names its field.
    fn assert_same_child_info(fast: &ChildInfo, played: &ChildInfo, ctx: &str) {
        assert_eq!(fast.mv, played.mv, "{ctx}: mv");
        assert_eq!(fast.pn, played.pn, "{ctx}: pn");
        assert_eq!(fast.dn, played.dn, "{ctx}: dn");
        assert_eq!(fast.outcome, played.outcome, "{ctx}: outcome");
        assert_eq!(fast.depth, played.depth, "{ctx}: depth");
        assert_eq!(
            fast.repetition_seen, played.repetition_seen,
            "{ctx}: repetition_seen"
        );
        assert_eq!(fast.explored, played.explored, "{ctx}: explored");
    }

    #[test]
    fn preprobe_resolves_live_unsolved_entry_without_make() {
        // A live `outcome == None` entry decides the child from the keys alone
        // (no make/unmake, no existence query). The reused bounds and the
        // neutral consequence of a degenerate entry must both match the
        // make/unmake path for the identical entry.
        let (mut pos, mv) = preprobe_fixture();
        let fen_before = pos.fen();
        let (child_key, _, _) = pos.keys_after(mv);

        let mut search = Search::new(1);
        // `max_depth = 11` ⇒ `child_max_depth = 10`, so `remaining_depth = 10`
        // is usable and the bounds are non-degenerate.
        search
            .tt
            .store(child_key, Move::NONE, u8::MAX, 1, None, 5, 5, 0, 10);

        let fast = search.evaluate_child(&mut pos, mv, 11, true);
        assert_eq!(
            search.preprobe_skips(),
            1,
            "the pre-probe must decide a live unsolved child"
        );
        assert_eq!((fast.pn, fast.dn), (5, 5), "stored bounds are reused");
        assert_eq!(fast.outcome, None);
        assert_eq!(fast.depth, 0);
        assert!(!fast.repetition_seen && !fast.explored);
        assert_eq!(pos.fen(), fen_before, "the child move is not played");

        let entry = *search.tt.probe(child_key).expect("entry stays live");
        let played =
            search.evaluate_child_played(&mut pos, mv, 11, true, false, child_key, Some(&entry));
        assert_same_child_info(&fast, &played, "live unsolved reuse");
        assert_eq!((played.pn, played.dn), (5, 5), "played path reuses too");
        assert_eq!(pos.fen(), fen_before);
    }

    #[test]
    fn preprobe_resolves_solved_entry_and_emits_the_same_proof_event() {
        // Identical solved payloads through the two decision paths must emit
        // the identical `NodeProven` event: the pre-probe (`best_move == NONE`,
        // decided before the make) and the shipped guarded branch (a
        // `best_move` hint whose one-ply guard does not fire, decided on the
        // played child — the pre-probe cannot evaluate that guard without the
        // child board, so it deliberately falls back).
        let (pos, mv) = preprobe_fixture();
        let (child_key, _, _) = pos.keys_after(mv);
        let hint = {
            let mut child = pos.clone();
            child.do_move(mv);
            child.legal_moves_vec()[0]
        };
        assert_ne!(hint, Move::NONE);

        let mut streams = Vec::new();
        let mut skips = Vec::new();
        for best_move in [Move::NONE, hint] {
            let mut search = Search::new(1);
            search.tt.store(
                child_key,
                best_move,
                u8::MAX,
                0,
                Some(Outcome::Loss),
                INF,
                0,
                1,
                u32::MAX,
            );
            let (tx, rx) = std::sync::mpsc::channel();
            search.set_proof_event_sender(Some(tx));
            let mut child_pos = Position::from_fen(PREPROBE_FEN).unwrap();
            let info = search.evaluate_child(&mut child_pos, mv, u32::MAX, true);
            assert_eq!(info.outcome, Some(Outcome::Loss));
            assert_eq!(info.depth, 1, "the entry's proven depth is preserved");
            skips.push(search.preprobe_skips());
            drop(search);
            streams.push(rx.iter().collect::<Vec<ProofEvent>>());
        }

        assert_eq!(skips, vec![1, 0], "only the hintless entry is a skip");
        assert_eq!(streams[0].len(), 1, "one proven child ⇒ one event");
        assert_eq!(
            format!("{:?}", streams[0]),
            format!("{:?}", streams[1]),
            "the pre-probe emits the make path's event"
        );
        match &streams[0][0] {
            ProofEvent::NodeProven(event) => {
                assert_eq!(event.path, vec![mv]);
                assert_eq!(event.hash, child_key, "event hash is the child key");
                assert_eq!(event.outcome, Outcome::Loss);
                assert_eq!(event.depth, 1);
            }
            ProofEvent::Clear => panic!("the search never clears proof events"),
        }
    }

    #[test]
    fn preprobe_falls_back_on_path_repetition() {
        // A path repetition is a Draw no matter what the table holds (the
        // shipped order puts the repetition branch before the TT); the ladder
        // must keep that, and the repetition branch is not a pre-probe skip.
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/4K3/8/8/8/4R3 w - - 0 1").unwrap();
        let mv = Move::make_move(Square::E5, Square::D5);
        let (child_key, child_rep_key, _) = pos.keys_after(mv);
        search.tt.store(
            child_key,
            Move::NONE,
            u8::MAX,
            0,
            Some(Outcome::Win),
            0,
            INF,
            1,
            u32::MAX,
        );
        search.path_push(child_rep_key);

        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
        assert_eq!(
            info.outcome,
            Some(Outcome::Draw),
            "the repetition branch outranks the live solved entry"
        );
        assert!(info.repetition_seen);
        assert_eq!(info.depth, 0);
        assert_eq!(search.preprobe_skips(), 0);
    }

    #[test]
    fn preprobe_falls_back_when_rule50_expired() {
        // A rule50-expired child is concluded by the shipped rule50 branch
        // (existence query), never by the pre-probe: its key's rule50 component
        // is clamped, so a live entry for it cannot exist. Per the shipped
        // order, a repetition of the same board must not fire either.
        let mut search = Search::new(1);
        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 100 1").unwrap();
        let mv = Move::make_move(Square::G1, Square::H1);
        let (child_key, child_rep_key, rule50_after) = pos.keys_after(mv);
        assert!(rule50_after >= 100, "fixture must expire the clock");
        search.tt.store(
            child_key,
            Move::NONE,
            u8::MAX,
            0,
            Some(Outcome::Win),
            0,
            INF,
            1,
            u32::MAX,
        );
        search.path_push(child_rep_key);

        let info = search.evaluate_child(&mut pos, mv, u32::MAX, true);
        assert_eq!(info.outcome, Some(Outcome::Draw), "rule50 draw");
        assert_eq!(info.depth, 0);
        assert!(
            !info.repetition_seen,
            "rule50 outranks the repetition check"
        );
        assert_eq!(search.preprobe_skips(), 0);
    }

    #[test]
    fn preprobe_degenerate_bounds_fall_back_to_neutral() {
        // `pn == 0` bounds and suppressed (`remaining_depth == u32::MAX`)
        // entries must both reuse neutral (1, 1), exactly like the played path.
        for (pn, dn, remaining_depth) in [(0u64, 5u64, 10u32), (5, 5, u32::MAX)] {
            let (mut pos, mv) = preprobe_fixture();
            let (child_key, _, _) = pos.keys_after(mv);
            let mut search = Search::new(1);
            search.tt.store(
                child_key,
                Move::NONE,
                u8::MAX,
                1,
                None,
                pn,
                dn,
                0,
                remaining_depth,
            );

            let fast = search.evaluate_child(&mut pos, mv, 11, true);
            assert_eq!(
                search.preprobe_skips(),
                1,
                "a live unsolved entry is decided by the pre-probe"
            );
            assert_eq!((fast.pn, fast.dn), (1, 1), "degenerate bounds ⇒ neutral");
            assert_eq!(fast.outcome, None);

            let entry = *search.tt.probe(child_key).expect("entry stays live");
            let played = search.evaluate_child_played(
                &mut pos,
                mv,
                11,
                true,
                false,
                child_key,
                Some(&entry),
            );
            assert_same_child_info(&fast, &played, "degenerate entry");
        }
    }

    #[test]
    fn preprobe_best_move_guard_matches_make_path() {
        // A solved entry whose `best_move` would repeat the path must not be
        // reused. Its one-ply guard needs the *child* board (the entry's move
        // is played from the child), which no key-level computation provides,
        // so such children always take the played path — there the shipped
        // guard fires and the child degenerates to neutral bounds, while the
        // same entry without the path push resolves normally.
        let (mut pos, mv) = preprobe_fixture();
        let (child_key, _, _) = pos.keys_after(mv);
        let hint = {
            let mut child = pos.clone();
            child.do_move(mv);
            child.legal_moves_vec()[0]
        };
        let grandchild_rep_key = {
            let mut child = pos.clone();
            child.do_move(mv);
            child.do_move(hint);
            child.repetition_key()
        };
        let seed = |search: &mut Search| {
            search.tt.store(
                child_key,
                hint,
                u8::MAX,
                0,
                Some(Outcome::Loss),
                INF,
                0,
                1,
                u32::MAX,
            );
        };

        // The guard fires: neutral bounds, played path, no pre-probe skip.
        let mut search = Search::new(1);
        seed(&mut search);
        search.path_push(grandchild_rep_key);
        let guarded = search.evaluate_child(&mut pos, mv, u32::MAX, true);
        assert_eq!(guarded.outcome, None, "the guarded entry must not resolve");
        assert_eq!((guarded.pn, guarded.dn), (1, 1));
        assert_eq!(
            search.preprobe_skips(),
            0,
            "a hinted solved entry takes the played path"
        );
        let entry = *search.tt.probe(child_key).expect("entry stays live");
        let played = search.evaluate_child_played(
            &mut pos,
            mv,
            u32::MAX,
            true,
            false,
            child_key,
            Some(&entry),
        );
        assert_same_child_info(&guarded, &played, "guarded entry");

        // The guard does not fire: the same entry resolves, still on the
        // played path (the pre-probe never takes a hinted entry).
        let mut search = Search::new(1);
        seed(&mut search);
        let resolved = search.evaluate_child(&mut pos, mv, u32::MAX, true);
        assert_eq!(resolved.outcome, Some(Outcome::Loss));
        assert_eq!(resolved.depth, 1);
        assert_eq!(search.preprobe_skips(), 0);
    }

    #[test]
    fn preprobe_skip_counter_resets_per_run() {
        // The test-only census counter is per-run state, like the repetition
        // cache: `begin_run` clears it so a second run's count is its own.
        let (mut pos, mv) = preprobe_fixture();
        let (child_key, _, _) = pos.keys_after(mv);
        let mut search = Search::new(1);
        search
            .tt
            .store(child_key, Move::NONE, u8::MAX, 1, None, 5, 5, 0, 10);
        let _ = search.evaluate_child(&mut pos, mv, 11, true);
        assert_eq!(search.preprobe_skips(), 1);

        search.begin_run();
        assert_eq!(search.preprobe_skips(), 0, "a new run starts from zero");
    }
}
