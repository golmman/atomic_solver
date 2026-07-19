//! Sequential DF-PN+ solver for atomic chess.

mod children;
mod core;
mod history;
mod pv;
mod selection;
mod simulate;

#[cfg(test)]
mod tests;

pub use core::outcome_from_pn_dn;

use std::collections::HashSet;
use std::time::{Duration, Instant};

use atomic_movegen::types::Move;

use crate::position::{Outcome, Position};
use crate::zobrist;

use super::ordering::{MoveScorer, StaticAtomicScorer};
use super::tt::TranspositionTable;

pub const INF: u64 = zobrist::INF;
const DEFAULT_EPSILON: f64 = 0.25;
const TIMEOUT_SECS: u64 = 5;
pub(crate) const DEFAULT_MAX_PV_PLIES: usize = 1000;

pub struct Search {
    pub(crate) tt: TranspositionTable,
    pub(crate) path: HashSet<u64>,
    pub(crate) path_stack: Vec<u64>,
    pub(crate) path_code: u64,
    pub(crate) nodes: u64,
    pub(crate) start: Instant,
    pub(crate) deadline: Instant,
    pub(crate) epsilon: f64,
    pub(crate) scorer: Box<dyn MoveScorer>,
    pub(crate) refine_shortest: bool,
    pub(crate) timeout: Duration,
    pub(crate) last_pv: Vec<Move>,
    pub(crate) history: [[[i32; 64]; 64]; 2],
    pub(crate) killers: [[Move; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH],
    pub(crate) history_age_counter: u64,
    pub(crate) max_ply: usize,
}

impl Search {
    pub fn new(tt_mb: usize) -> Self {
        Self {
            tt: TranspositionTable::with_mb(tt_mb),
            path: HashSet::new(),
            path_stack: Vec::new(),
            path_code: 0,
            nodes: 0,
            start: Instant::now(),
            deadline: Instant::now(),
            epsilon: DEFAULT_EPSILON,
            scorer: Box::new(StaticAtomicScorer),
            refine_shortest: false,
            timeout: Duration::from_secs(TIMEOUT_SECS),
            last_pv: Vec::new(),
            history: [[[0; 64]; 64]; 2],
            killers: [[Move::NONE; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH],
            history_age_counter: 0,
            max_ply: DEFAULT_MAX_PV_PLIES,
        }
    }

    pub fn refine_shortest(&mut self, value: bool) {
        self.refine_shortest = value;
    }

    pub fn set_timeout(&mut self, seconds: u64) {
        self.timeout = Duration::from_secs(seconds);
    }

    pub fn set_max_ply(&mut self, max_ply: usize) {
        self.max_ply = max_ply.max(1);
    }

    pub fn twin_stats(&self) -> (u64, u64) {
        self.tt.twin_stats()
    }

    pub fn peak_twins(&self) -> u8 {
        self.tt.peak_twins()
    }

    pub fn set_epsilon(&mut self, epsilon: f64) {
        assert!(
            (0.0..=1.0).contains(&epsilon),
            "epsilon must be in [0.0, 1.0], got {epsilon}"
        );
        self.epsilon = epsilon;
    }

    pub fn search_depth(
        &mut self,
        pos: &mut Position,
        max_depth: u32,
    ) -> (Outcome, Vec<Move>, u64) {
        self.nodes = 0;
        self.start = Instant::now();
        self.deadline = self.start + self.timeout;
        self.path.clear();
        self.path_stack.clear();
        self.path_code = 0;
        self.last_pv.clear();
        let outcome = self.dfpn(pos, INF, INF, max_depth, true);
        let pv = self.extract_pv(pos);
        (outcome, pv, self.nodes)
    }

    pub fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        self.nodes = 0;
        self.start = Instant::now();
        self.deadline = self.start + self.timeout;
        self.path.clear();
        self.path_stack.clear();
        self.path_code = 0;
        self.last_pv.clear();

        if self.refine_shortest {
            // Bootstrap: find any decisive result with a small depth budget,
            // doubling the budget until the position is solved. Do not refine
            // during bootstrap; we only need a winning outcome to start from.
            let saved_refine = self.refine_shortest;
            self.refine_shortest = false;

            let mut bootstrap_outcome = Outcome::Draw;
            let mut max_depth = 1u32;
            while max_depth <= 64 {
                self.reset_search_state();
                self.tt.clear();
                bootstrap_outcome = self.dfpn(pos, INF, INF, max_depth, true);
                if bootstrap_outcome != Outcome::Draw || self.time_exceeded() {
                    break;
                }
                max_depth = max_depth.saturating_mul(2);
            }

            self.refine_shortest = saved_refine;

            if self.time_exceeded() {
                let pv = self.last_pv.clone();
                (bootstrap_outcome, pv, self.nodes)
            } else {
                let (outcome, pv, _) = self.solve_refined(pos);
                if outcome == Outcome::Draw && bootstrap_outcome != Outcome::Draw {
                    (bootstrap_outcome, self.last_pv.clone(), self.nodes)
                } else {
                    (outcome, pv, self.nodes)
                }
            }
        } else {
            let outcome = self.dfpn(pos, INF, INF, u32::MAX, true);
            let pv = self
                .extract_pv_checked(pos, outcome, None)
                .unwrap_or_else(|| {
                    eprintln!("warning: returning unvalidated PV");
                    self.extract_pv(pos)
                });
            (outcome, pv, self.nodes)
        }
    }

    fn solve_refined(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        // Depth-bounded refinement: first find any win/loss without a depth bound
        // to get an initial PV, then binary search the smallest depth bound that
        // still yields the same outcome. Start each probe with a clean search
        // state so that stale history/killer data from the bootstrap or previous
        // probes does not misdirect the depth-bounded search.
        let saved_refine = self.refine_shortest;
        self.refine_shortest = false;
        self.reset_search_state();
        self.tt.clear();
        self.reset_history_and_killers();

        let outcome = self.dfpn(pos, INF, INF, u32::MAX, true);
        let best_outcome = outcome;
        let best_depth = self
            .tt
            .probe(pos.hash())
            .and_then(|e| e.best_result_for_path(0).map(|(.., depth)| depth))
            .unwrap_or(u32::MAX);

        let full_depth = if best_depth == u32::MAX {
            None
        } else {
            Some(best_depth)
        };

        if let Some(first_pv) = self.extract_pv_checked(pos, outcome, full_depth) {
            self.last_pv = first_pv;
        }

        let full_depth_pv = self.last_pv.clone();

        if outcome != Outcome::Draw && best_depth > 1 && best_depth != u32::MAX {
            let mut lo = 1;
            let mut hi = best_depth;

            while lo < hi && !self.time_exceeded() {
                let mid = (lo + hi) / 2;
                self.reset_search_state();
                self.tt.clear();
                self.reset_history_and_killers();
                let o = self.dfpn(pos, INF, INF, mid, true);

                if self.time_exceeded() {
                    break;
                }

                if o == outcome {
                    hi = mid;
                    if let Some(pv) = self.extract_pv_checked(pos, outcome, None) {
                        self.last_pv = pv;
                    }
                } else {
                    lo = mid + 1;
                }
            }

            // Validate the binary-search answer at the exact depth. If it is
            // inconsistent (e.g. due to timeout or TT noise), fall back to the
            // full-depth PV instead of a possibly wrong shorter one.
            self.reset_search_state();
            self.tt.clear();
            self.reset_history_and_killers();
            let o = self.dfpn(pos, INF, INF, lo, true);
            if o == outcome {
                if let Some(pv) = self.extract_pv_checked(pos, outcome, None) {
                    // The final result is printed by the CLI caller; just keep
                    // the validated PV so it can be returned.
                    self.last_pv = pv.to_vec();
                }
            } else {
                self.last_pv = full_depth_pv;
            }
        }

        self.refine_shortest = saved_refine;

        let pv = if !self.last_pv.is_empty() {
            self.last_pv.clone()
        } else {
            self.extract_pv(pos)
        };
        (best_outcome, pv, self.nodes)
    }

    pub(crate) fn reset_search_state(&mut self) {
        self.path.clear();
        self.path_stack.clear();
        self.path_code = 0;
    }

    pub(crate) fn reset_history_and_killers(&mut self) {
        self.history = [[[0; 64]; 64]; 2];
        self.killers = [[Move::NONE; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH];
        self.history_age_counter = 0;
    }

    pub(crate) fn time_exceeded(&self) -> bool {
        Instant::now() >= self.deadline
    }
}
