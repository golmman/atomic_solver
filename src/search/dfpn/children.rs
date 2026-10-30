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
use super::eval_guide::ChildScale;
use super::move_class;
use super::quiet_group;
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
    /// The delayed plain-move group (provari plan3). A group is a disjunction
    /// of hidden moves at an OR node: `mv == Move::NONE`, never solved
    /// (`outcome == None`), and replaced by its evaluated members when opened
    /// (see [`Search::open_quiet_group`]). At most one per frame.
    pub group: bool,
}

impl ChildInfo {
    /// A child whose outcome is decided: bounds derived from the outcome at
    /// the child's side (`child_is_or` is the *child's* node kind, i.e.
    /// `!is_or_node` of the parent).
    fn solved(
        mv: Move,
        outcome: Outcome,
        depth: u32,
        repetition_seen: bool,
        child_is_or: bool,
    ) -> Self {
        let (pn, dn) = outcome.pn_dn_for(child_is_or);
        Self {
            mv,
            pn,
            dn,
            outcome: Some(outcome),
            depth,
            repetition_seen,
            explored: false,
            group: false,
        }
    }

    /// A non-terminal child carrying TT bounds (or the neutral `(1, 1)`).
    fn unsolved(mv: Move, pn: u64, dn: u64) -> Self {
        Self {
            mv,
            pn,
            dn,
            outcome: None,
            depth: 0,
            repetition_seen: false,
            explored: false,
            group: false,
        }
    }

    /// The delayed plain-move group (provari plan3): one unsolved child that
    /// stands for every hidden plain move at an OR node. It is never moved
    /// into and never solved; `pn`/`dn` are starting estimates, not proofs.
    pub(super) fn group(pn: u64, dn: u64) -> Self {
        Self {
            mv: Move::NONE,
            pn,
            dn,
            outcome: None,
            depth: 0,
            repetition_seen: false,
            explored: false,
            group: true,
        }
    }

    /// A move that was never evaluated: the early-decisive cut in
    /// `evaluate_all_children` fills the remaining slots with it (OR-side
    /// neutral bounds, no outcome).
    fn unevaluated(mv: Move) -> Self {
        Self {
            mv,
            pn: INF,
            dn: 0,
            outcome: None,
            depth: 0,
            repetition_seen: false,
            explored: false,
            group: false,
        }
    }

    /// Bounds to reuse from a live, outcome-less TT entry, or the neutral
    /// `(1, 1)`.
    ///
    /// Only non-degenerate entries are reusable: a previous work-bounded
    /// search may have stored a candidate terminal-like bound (`pn == 0 ||
    /// dn == 0`) without an outcome, and propagating such values can trick
    /// the parent search into treating an unproven node as solved. A
    /// depth-suppressed entry (`remaining_depth == u32::MAX`) and an entry
    /// solved deeper than the current bound are equally unusable.
    fn unsolved_bounds(entry: Option<&TtEntry>, child_max_depth: u32) -> (u64, u64) {
        match entry {
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
        }
    }
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
        scale: Option<ChildScale>,
    ) {
        children.clear();
        // Grouped OR-node path (provari plan3, default off). Kept out of the
        // shipped loop so the flag-off path stays byte-identical.
        if self.quiet_grouping && is_or_node {
            self.evaluate_all_children_grouped(pos, moves, max_depth, is_or_node, children, scale);
            return;
        }
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
                scale,
            );
            let decisive = info.outcome == Some(Outcome::Loss);
            children.push(info);
            if decisive {
                for j in (i + 1)..moves.len() {
                    children.push(ChildInfo::unevaluated(moves[j]));
                }
                break;
            }
        }
    }

    /// Grouped OR-node child evaluation (provari plan3): plain moves are
    /// hidden behind one delayed group and are not keyed, prefetched, probed,
    /// evaluated, billed, or reported as proof events. Non-plain moves run the
    /// shipped key pre-pass and evaluation, including the early decisive Loss
    /// cut (which suppresses the group: the parent is already won).
    fn evaluate_all_children_grouped(
        &mut self,
        pos: &mut Position,
        moves: &MoveList,
        max_depth: u32,
        is_or_node: bool,
        children: &mut Vec<ChildInfo>,
        scale: Option<ChildScale>,
    ) {
        let child_count = moves.len();
        if self.plain_scratch.len() < child_count {
            self.plain_scratch.resize(child_count, false);
        }
        let mut hidden_count = 0u64;
        for i in 0..child_count {
            let plain = quiet_group::is_plain(pos, moves[i]);
            self.plain_scratch[i] = plain;
            if plain {
                hidden_count += 1;
            }
        }

        if self.keys_scratch.len() < child_count {
            self.keys_scratch.resize(child_count, (0, 0, 0));
        }
        for i in 0..child_count {
            if self.plain_scratch[i] {
                continue;
            }
            let keys = pos.keys_after(moves[i]);
            self.tt.prefetch(keys.0);
            self.keys_scratch[i] = keys;
        }

        for i in 0..child_count {
            if self.plain_scratch[i] {
                continue;
            }
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
                scale,
            );
            let decisive = info.outcome == Some(Outcome::Loss);
            children.push(info);
            if decisive {
                for j in (i + 1)..child_count {
                    if !self.plain_scratch[j] {
                        children.push(ChildInfo::unevaluated(moves[j]));
                    }
                }
                return;
            }
        }

        if hidden_count > 0 {
            // The group is a fresh OR-node child, so the parent's scale applies
            // to its starting numbers exactly as to a fresh member (provari
            // scales its group child the same way).
            let hidden = hidden_count.clamp(1, INF);
            let (pn, dn) = match scale {
                Some(scale) => scale.apply(quiet_group::GROUP_PN, hidden),
                None => (quiet_group::GROUP_PN, hidden),
            };
            children.push(ChildInfo::group(pn, dn));
        }
    }

    /// Open the selected quiet-move group: remove it, evaluate exactly the
    /// hidden plain members through the shipped [`Search::evaluate_child`]
    /// (so `child_evals`, the budget, TT reuse, and proof events keep their
    /// meaning), and append them in move-list order. An OR-node member that
    /// proves a child `Loss` ends the opening early: the parent is already
    /// won. The group is never recreated in this frame.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn open_quiet_group(
        &mut self,
        pos: &mut Position,
        moves: &MoveList,
        max_depth: u32,
        is_or_node: bool,
        children: &mut Vec<ChildInfo>,
        idx: usize,
        scale: Option<ChildScale>,
    ) {
        debug_assert!(children[idx].group, "only a group child may be opened");
        debug_assert!(is_or_node, "groups exist only at OR nodes");
        children.remove(idx);

        for i in 0..moves.len() {
            let mv = moves[i];
            if !quiet_group::is_plain(pos, mv) {
                continue;
            }
            let info = self.evaluate_child(pos, mv, max_depth, is_or_node, scale);
            let decisive = info.outcome == Some(Outcome::Loss);
            children.push(info);
            if decisive {
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
        scale: Option<ChildScale>,
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
            scale,
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
        scale: Option<ChildScale>,
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
            ChildInfo::solved(mv, Outcome::Draw, 0, true, child_is_or)
        } else if rule50_expired {
            // A rule50 terminal needs the child's existence query, so it takes
            // the played path, whose rule50 branch returns before any TT use
            // (as shipped) — hence `entry: None`.
            self.evaluate_child_played(
                pos,
                mv,
                max_depth,
                child_is_or,
                true,
                child_key,
                None,
                scale,
            )
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
                ChildInfo::solved(mv, resolved.outcome, resolved.depth, false, child_is_or)
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
                // This branch established `outcome.is_none()` above, so the
                // helper's guard is provably a no-op here; keep it in the
                // helper (do not "simplify" either conjunct away).
                let (pn, dn) = ChildInfo::unsolved_bounds(entry.as_ref(), child_max_depth);
                ChildInfo::unsolved(mv, pn, dn)
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
                    scale,
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
        scale: Option<ChildScale>,
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
            let info = ChildInfo::solved(mv, resolved.outcome, resolved.depth, false, child_is_or);
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
            ChildInfo::solved(mv, Outcome::Loss, 0, false, child_is_or)
        } else if pos.commoners(pos.side_to_move().flip()).is_empty() {
            ChildInfo::solved(mv, Outcome::Win, 0, false, child_is_or)
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
            ChildInfo::solved(mv, outcome, 0, false, child_is_or)
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
                ChildInfo::solved(mv, resolved.outcome, resolved.depth, false, child_is_or)
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
                    ChildInfo::solved(mv, outcome, 0, false, child_is_or)
                } else {
                    // Unsolved bounds are only consulted after the movegen-based
                    // terminal check says non-terminal, like the old code where
                    // the outcome check preceded the TT probe.
                    let (pn, dn) = ChildInfo::unsolved_bounds(entry, child_max_depth);
                    // Freshness is decided exactly as plan2 defined it for
                    // move-class numbers: the bounds are still the neutral
                    // `(1, 1)`. Solved, repetition, rule50 and TT-reused
                    // children returned above, so neither heuristic can
                    // override an outcome or a learned bound.
                    let fresh = pn == 1 && dn == 1;
                    // Move-class starting proof numbers (provari plan2,
                    // default off): only for a fresh OR-node child whose
                    // parent is OR (`!child_is_or`). A starting value is a
                    // positive, finite bound in `[1, 15]`; `dn` stays 1.
                    let (pn, dn) = if fresh && self.move_class_pn && !child_is_or {
                        (move_class::starting_proof_number(pos, mv), 1)
                    } else {
                        (pn, dn)
                    };
                    // EF-PN scaling (provari plan5, default off): the parent's
                    // hand evaluation scales a *fresh* child, composing with
                    // the plan2 class number on the OR branch and applying to
                    // the neutral `(1, 1)` on the AND branch.
                    let (pn, dn) = if fresh && let Some(scale) = scale {
                        scale.apply(pn, dn)
                    } else {
                        (pn, dn)
                    };
                    ChildInfo::unsolved(mv, pn, dn)
                }
            }
        };

        pos.undo_move_with_scratch(mv, &scratch);
        self.eval_state_pool[scratch_depth] = scratch;
        info
    }
}

#[cfg(test)]
mod tests;
