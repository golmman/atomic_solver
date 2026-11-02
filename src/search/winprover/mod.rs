//! Opt-in attacker-only win prover front-end (winprover item 3).
//!
//! A win-only AND/OR proof-number search that runs *before* the general
//! solver in [`crate::search::dfpn::Search::solve_with_progress`]. It returns a
//! [`WinProof`] only when it proves the root is a `Win` for the side to move;
//! every other result falls back to the unchanged general search.
//!
//! # Soundness contract
//!
//! - The front-end owns a **scratch [`TranspositionTable`]**
//!   ([`WIN_FRONT_END_TT_MB`]); the production table is never probed, stored,
//!   or serialized. This is what makes a failed front-end run fall back to a
//!   bit-identical general search.
//! - It emits `ProofEvent::NodeProven` for every proven `Win`/`Loss` node, with
//!   the same path/hash/outcome/depth semantics as `dfpn`. A failed run may
//!   be followed by duplicate events from the general search; the proof-tree
//!   worker tolerates re-proving.
//! - Repetition (a position already on the current path) and rule50 (via
//!   `Position::outcome()`) are **Draws**, never wins. A reputation-dependent
//!   Draw is never TT-stored; a terminal Draw is kept only in the frame's
//!   child table.
//! - Child evaluations are billed against the front-end cap and the run's
//!   global child-eval budget. The wall deadline and stop flag are safety
//!   nets; the deterministic bound is the cap.
//! - No general-solver heuristic is read: no move-class numbers, quiet
//!   grouping, EF-PN, static tactics, history/killers, pre-phase, cheap probe,
//!   refinement, or repetition cache.
//!
//! The front-end never claims a `Loss` or a `Draw`: those remain the general
//! solver's product result.

mod children;
mod search;
mod simulation;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::proof_event::{NodeProven, ProofEvent};
use crate::search::dfpn::fraction::fraction_from_f64;
use crate::search::ordering::StaticAtomicScorer;
use crate::search::tt::TranspositionTable;
use crate::zobrist::INF;

/// Scratch TT size. A bounded, documented exception to the search CLI's
/// "RAM = TT only" statement; dropped when the front-end returns and never
/// serialized into `--tt-dump-path` snapshots.
pub const WIN_FRONT_END_TT_MB: usize = 32;

/// Default deterministic child-eval cap for the front-end.
pub const DEFAULT_WIN_FRONT_END_BUDGET: u64 = 1_000_000;

/// Same default ε as the general solver, so the `1+ε` thresholds match.
const DEFAULT_EPSILON: f64 = 0.125;

/// Re-read the wall clock once per this many nodes (the cap is the real
/// deterministic bound).
const CLOCK_SAMPLE_INTERVAL: u64 = 4096;

/// A replay-validated winning proof from the front-end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinProof {
    /// The principal line. Informational: the proof-tree layer remains
    /// authoritative. Replayed legally to a terminal win before this value is
    /// constructed.
    pub pv: Vec<Move>,
    /// Proven depth in plies (`pv.len()`).
    pub depth: u32,
}

/// Attacker-only win prover. Create one per root; the scratch TT must never
/// be shared across roots (its unsolved bounds are root-attacker relative).
pub struct WinProver {
    pub(super) tt: TranspositionTable,
    /// Repetition keys on the current path (root to parent).
    pub(super) path_stack: Vec<u64>,
    /// Moves on the current path, for `ProofEvent` paths.
    pub(super) move_stack: Vec<Move>,
    /// Whether a refuted defence's proof is replayed on its open siblings.
    /// Default `true`; `--no-win-front-end-simulation` maps here.
    pub(super) simulation_enabled: bool,
    pub(super) nodes: u64,
    pub(super) child_evals: u64,
    /// Replays attempted (one per sibling position), successful replays, and
    /// child evaluations spent inside the replay. All are subsets of
    /// [`WinProver::child_evaluations`].
    pub(super) simulation_attempts: u64,
    pub(super) simulation_successes: u64,
    pub(super) simulation_evaluations: u64,
    budget: u64,
    deadline: Instant,
    stop_flag: Option<Arc<AtomicBool>>,
    pub(super) scorer: StaticAtomicScorer,
    pub(super) epsilon_num: u64,
    pub(super) epsilon_den: u64,
    proof_event_sender: Option<std::sync::mpsc::Sender<ProofEvent>>,
    clock_sample_at: u64,
    last_clock: Instant,
}

impl WinProver {
    /// Create a front-end with a scratch TT, a deterministic child-eval cap,
    /// a wall deadline, and an optional stop flag. `budget == 0` makes
    /// [`WinProver::solve`] a no-op.
    #[must_use]
    pub fn new(
        scorer: StaticAtomicScorer,
        budget: u64,
        deadline: Instant,
        stop_flag: Option<Arc<AtomicBool>>,
    ) -> Self {
        let (epsilon_num, epsilon_den) = fraction_from_f64(1.0 + DEFAULT_EPSILON);
        Self {
            tt: TranspositionTable::with_mb(WIN_FRONT_END_TT_MB),
            path_stack: Vec::new(),
            move_stack: Vec::new(),
            simulation_enabled: true,
            nodes: 0,
            child_evals: 0,
            simulation_attempts: 0,
            simulation_successes: 0,
            simulation_evaluations: 0,
            budget,
            deadline,
            stop_flag,
            scorer,
            epsilon_num,
            epsilon_den,
            proof_event_sender: None,
            clock_sample_at: 0,
            last_clock: Instant::now(),
        }
    }

    /// Use the general solver's exact `1+ε` fraction (avoids re-deriving it
    /// from a float and keeps the two searches threshold-identical).
    pub(crate) fn set_epsilon_fraction(&mut self, num: u64, den: u64) {
        self.epsilon_num = num;
        self.epsilon_den = den;
    }

    /// Override the `1+ε` threshold (the default already matches the general
    /// solver).
    pub fn set_epsilon(&mut self, epsilon: f64) {
        assert!(
            (0.0..=1.0).contains(&epsilon),
            "epsilon must be in [0.0, 1.0], got {epsilon}"
        );
        let (num, den) = fraction_from_f64(1.0 + epsilon);
        self.epsilon_num = num;
        self.epsilon_den = den;
    }

    pub fn set_proof_event_sender(&mut self, sender: Option<std::sync::mpsc::Sender<ProofEvent>>) {
        self.proof_event_sender = sender;
    }

    /// Enable or disable sibling proof replay (the first form, default on).
    pub fn set_simulation(&mut self, enabled: bool) {
        self.simulation_enabled = enabled;
    }

    /// Child evaluations spent by the front-end.
    #[must_use]
    pub fn child_evaluations(&self) -> u64 {
        self.child_evals
    }

    /// Frames visited by the front-end.
    #[must_use]
    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    /// Sibling replays attempted by the last run.
    #[must_use]
    pub fn simulation_attempts(&self) -> u64 {
        self.simulation_attempts
    }

    /// Sibling replays that proved the sibling.
    #[must_use]
    pub fn simulation_successes(&self) -> u64 {
        self.simulation_successes
    }

    /// Child evaluations spent inside sibling replays (included in
    /// [`WinProver::child_evaluations`]).
    #[must_use]
    pub fn simulation_evaluations(&self) -> u64 {
        self.simulation_evaluations
    }

    /// Try to prove a root `Win` for the side to move.
    ///
    /// Returns `None` for a terminal root (the general solver owns terminals),
    /// for any non-Win result, and when the cap/deadline/stop flag fires
    /// before the proof completes. A returned proof's principal line is
    /// replayed and only returned when it reaches a terminal win.
    pub fn solve(&mut self, pos: &Position) -> Option<WinProof> {
        if pos.outcome().is_some() {
            return None;
        }
        if self.budget == 0 || self.stop() {
            return None;
        }
        self.tt.clear();
        self.path_stack.clear();
        self.move_stack.clear();
        self.nodes = 0;
        self.child_evals = 0;
        self.simulation_attempts = 0;
        self.simulation_successes = 0;
        self.simulation_evaluations = 0;
        self.last_clock = Instant::now();
        self.clock_sample_at = 0;

        let mut root = pos.clone();
        if self.dfpn(&mut root, INF, INF, true) != Outcome::Win {
            return None;
        }
        let pv = self.extract_pv(pos);
        if !self.validate_win(&pv, pos) {
            return None;
        }
        Some(WinProof {
            depth: pv.len() as u32,
            pv,
        })
    }

    /// Whether the deterministic cap, the stop flag, or the wall deadline has
    /// fired.
    pub(super) fn stop(&mut self) -> bool {
        if let Some(flag) = &self.stop_flag
            && flag.load(Ordering::Acquire)
        {
            return true;
        }
        if self.child_evals >= self.budget {
            return true;
        }
        if self.nodes >= self.clock_sample_at {
            self.last_clock = Instant::now();
            self.clock_sample_at = self.nodes + CLOCK_SAMPLE_INTERVAL;
        }
        self.last_clock >= self.deadline
    }

    pub(super) fn path_contains(&self, key: u64) -> bool {
        self.path_stack.contains(&key)
    }

    pub(super) fn path_push(&mut self, key: u64) {
        self.path_stack.push(key);
    }

    pub(super) fn path_pop(&mut self) {
        self.path_stack.pop();
    }

    pub(super) fn emit_proof_node(&self, pos: &Position, outcome: Outcome, depth: u32) {
        self.emit_proof_node_with_path(&self.move_stack, pos, outcome, depth);
    }

    /// Emit a proven node on an explicit path. The simulation uses this
    /// helper because its replay path is not the search's `move_stack`.
    pub(super) fn emit_proof_node_with_path(
        &self,
        path: &[Move],
        pos: &Position,
        outcome: Outcome,
        depth: u32,
    ) {
        if outcome == Outcome::Draw {
            return;
        }
        if let Some(sender) = &self.proof_event_sender {
            let _ = sender.send(ProofEvent::NodeProven(NodeProven::new(
                path.to_vec(),
                pos.hash(),
                outcome,
                depth,
            )));
        }
    }

    /// Follow the scratch TT's solved `best_move` chain from the root.
    fn extract_pv(&self, pos: &Position) -> Vec<Move> {
        let mut pv = Vec::new();
        let mut current = pos.clone();
        let mut expected = Some(Outcome::Win);
        for _ in 0..1000 {
            if current.outcome().is_some() {
                break;
            }
            let Some(entry) = self.tt.probe(current.hash()) else {
                break;
            };
            let Some((mv, outcome, _)) = entry.best_result() else {
                break;
            };
            if Some(outcome) != expected || mv == Move::NONE {
                break;
            }
            let mut moves = MoveList::new();
            current.legal_moves(&mut moves);
            if !moves.as_slice().contains(&mv) {
                break;
            }
            current.do_move(mv);
            pv.push(mv);
            expected = expected.map(Outcome::flip);
        }
        pv
    }

    /// Replay `pv` from `pos` and require a legal line reaching a terminal win
    /// for the root side. A one-ply win ends with the opponent to move and a
    /// `Loss`; the general parity rule covers both.
    fn validate_win(&self, pv: &[Move], pos: &Position) -> bool {
        let mut current = pos.clone();
        for &mv in pv {
            let mut moves = MoveList::new();
            current.legal_moves(&mut moves);
            if !moves.as_slice().contains(&mv) {
                return false;
            }
            current.do_move(mv);
        }
        let expected = if pv.len().is_multiple_of(2) {
            Outcome::Win
        } else {
            Outcome::Loss
        };
        current.outcome() == Some(expected)
    }
}
