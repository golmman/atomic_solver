//! Sequential DF-PN+ solver for atomic chess.

mod children;
mod core;
mod history;
mod pv;
mod selection;
mod simulate;

#[cfg(test)]
mod tests;

pub use crate::zobrist::INF;
pub use core::outcome_from_pn_dn;

use std::time::{Duration, Instant};

use atomic_movegen::types::Move;

use crate::position::{Outcome, Position};

use super::ordering::StaticAtomicScorer;
use super::tt::TranspositionTable;

const DEFAULT_EPSILON: f64 = 0.125;
const TIMEOUT_SECS: u64 = 5;
const DEFAULT_MAX_PV_PLIES: usize = 1000;

/// Convert the f64 value `1.0 + epsilon` into an exact reduced `num/den` fraction.
///
/// `epsilon` is constrained to `[0.0, 1.0]`, so `v` is in `[1.0, 2.0]` and is a
/// normal dyadic rational.  The returned numerator and denominator fit in `u64`
/// and are reduced by their greatest common divisor.
fn epsilon_fraction(v: f64) -> (u64, u64) {
    let bits = v.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = bits & 0xfffffffffffff;
    let mut num = (1u64 << 52) | mantissa;
    let mut den = 1u64;

    let exp = exponent - 1075; // 1023 (bias) + 52 (fraction bits)
    if exp >= 0 {
        num <<= exp as u32;
    } else {
        den = 1u64 << (-exp) as u32;
    }

    let g = gcd(num, den);
    (num / g, den / g)
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

pub struct Search {
    tt: TranspositionTable,
    path_stack: Vec<u64>,
    path_code: u64,
    nodes: u64,
    child_evals: u64,
    start: Instant,
    deadline: Instant,
    epsilon_num: u64,
    epsilon_den: u64,
    scorer: StaticAtomicScorer,
    refine_shortest: bool,
    timeout: Duration,
    last_pv: Vec<Move>,
    history: [[[i32; 64]; 64]; 2],
    killers: [[Move; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH],
    history_age_counter: u64,
    max_ply: usize,
    bootstrap_success_depth: Option<u32>,
    bootstrap_fail_depth: u32,
}

impl Search {
    pub fn new(tt_mb: usize) -> Self {
        let (epsilon_num, epsilon_den) = epsilon_fraction(1.0 + DEFAULT_EPSILON);
        Self {
            tt: TranspositionTable::with_mb(tt_mb),
            path_stack: Vec::new(),
            path_code: 0,
            nodes: 0,
            child_evals: 0,
            start: Instant::now(),
            deadline: Instant::now(),
            epsilon_num,
            epsilon_den,
            scorer: StaticAtomicScorer,
            refine_shortest: false,
            timeout: Duration::from_secs(TIMEOUT_SECS),
            last_pv: Vec::new(),
            history: [[[0; 64]; 64]; 2],
            killers: [[Move::NONE; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH],
            history_age_counter: 0,
            max_ply: DEFAULT_MAX_PV_PLIES,
            bootstrap_success_depth: None,
            bootstrap_fail_depth: 0,
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
        let (num, den) = epsilon_fraction(1.0 + epsilon);
        self.epsilon_num = num;
        self.epsilon_den = den;
    }

    pub fn search_depth(
        &mut self,
        pos: &mut Position,
        max_depth: u32,
    ) -> (Outcome, Vec<Move>, u64) {
        self.begin_run();
        let outcome = self.dfpn(pos, INF, INF, max_depth, true);
        let pv = self.extract_pv(pos);
        (outcome, pv, self.nodes)
    }

    /// Run the solver to a decisive outcome or the configured timeout.
    ///
    /// Internally this bootstraps with an iteratively doubling depth bound and,
    /// if necessary, falls back to an unbounded search.  The result and the
    /// depth bounds found during the search are recorded for the follow-up
    /// `find_ppv` and `refine_sppv` stages.
    pub fn solve_outcome(&mut self, pos: &mut Position) -> Outcome {
        self.begin_run();
        let saved_refine = self.refine_shortest;
        self.refine_shortest = false;

        let mut outcome = Outcome::Draw;
        let mut max_depth = 1u32;
        let mut success_depth: Option<u32> = None;
        let mut fail_depth = 0u32;

        while max_depth <= 64 {
            self.reset_search_state();
            outcome = self.dfpn(pos, INF, INF, max_depth, true);
            if self.time_exceeded() {
                break;
            }
            if outcome != Outcome::Draw {
                success_depth = Some(max_depth);
                break;
            }
            fail_depth = max_depth;
            max_depth = max_depth.saturating_mul(2);
        }

        if !self.time_exceeded() && success_depth.is_none() {
            self.reset_search_state();
            self.tt.new_generation();
            self.reset_history_and_killers();
            outcome = self.dfpn(pos, INF, INF, u32::MAX, true);
            if outcome != Outcome::Draw {
                if let Some(depth) = self
                    .tt
                    .probe(pos.hash())
                    .and_then(|e| e.best_result_for_path(0).map(|(.., d)| d))
                {
                    success_depth = Some(depth);
                    fail_depth = depth.saturating_sub(1);
                } else {
                    success_depth = Some(u32::MAX);
                    fail_depth = 0;
                }
            }
        }

        self.refine_shortest = saved_refine;
        self.bootstrap_success_depth = success_depth;
        self.bootstrap_fail_depth = fail_depth;
        outcome
    }

    /// Find and verify a Proof PV (PPV) for `outcome`.
    ///
    /// A PPV has winning attacker moves and defender replies that maximize the
    /// length of the defense.  The returned PV is validated to reach the
    /// expected terminal outcome.
    pub fn find_ppv(&mut self, pos: &mut Position, outcome: Outcome) -> Option<Vec<Move>> {
        if self.time_exceeded() {
            return None;
        }

        if let Some(depth) = self.bootstrap_success_depth {
            self.reset_search_state();
            let saved_refine = self.refine_shortest;
            self.refine_shortest = false;
            self.dfpn(pos, INF, INF, depth, true);
            self.refine_shortest = saved_refine;
            if self.time_exceeded() {
                return None;
            }
        }

        let pv = self
            .extract_ppv(pos, outcome)
            .unwrap_or_else(|| self.extract_pv(pos));
        if pv.is_empty() {
            return None;
        }
        self.last_pv = pv;
        Some(self.last_pv.clone())
    }

    /// Iteratively refine the PPV toward the Shortest PPV (SPPV).
    ///
    /// For each strictly shorter PPV discovered, `on_shorter` is called with
    /// the new line.  The transposition table and move-ordering history from
    /// the earlier stages are reused; only path-dependent state is reset between
    /// probes.
    pub fn refine_sppv<F>(&mut self, pos: &mut Position, outcome: Outcome, mut on_shorter: F)
    where
        F: FnMut(&[Move]),
    {
        let start_depth = self
            .bootstrap_success_depth
            .unwrap_or(self.last_pv.len() as u32);
        let mut current_best_len = self.last_pv.len() as u32;
        let mut lo = self.bootstrap_fail_depth;
        let mut hi = start_depth;

        while hi > lo + 1 && !self.time_exceeded() {
            let probe = hi - 1;
            self.reset_search_state();
            let saved_refine = self.refine_shortest;
            self.refine_shortest = true;
            let o = self.dfpn(pos, INF, INF, probe, true);
            self.refine_shortest = saved_refine;

            if self.time_exceeded() {
                break;
            }

            if o == outcome {
                if let Some(pv) = self.extract_pv_checked(pos, outcome, None) {
                    let pv_len = pv.len() as u32;
                    if pv_len < current_best_len {
                        self.last_pv = pv;
                        current_best_len = pv_len;
                        on_shorter(&self.last_pv);
                    } else if pv_len == current_best_len {
                        self.last_pv = pv;
                    }
                }
                hi = probe;
            } else {
                lo = probe;
            }
        }
    }

    /// Solve in a single call, returning the final outcome and PV.
    ///
    /// This is a convenience wrapper around the staged API.  When
    /// `refine_shortest` is false, it performs a single unbounded search for
    /// backward compatibility and speed on shallow positions.  When true, it
    /// runs `solve_outcome`, `find_ppv`, and `refine_sppv`.
    pub fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        if !self.refine_shortest {
            self.begin_run();
            let outcome = self.dfpn(pos, INF, INF, u32::MAX, true);
            if outcome != Outcome::Draw {
                if let Some(depth) = self
                    .tt
                    .probe(pos.hash())
                    .and_then(|e| e.best_result_for_path(0).map(|(.., d)| d))
                {
                    self.bootstrap_success_depth = Some(depth);
                    self.bootstrap_fail_depth = depth.saturating_sub(1);
                } else {
                    self.bootstrap_success_depth = Some(u32::MAX);
                    self.bootstrap_fail_depth = 0;
                }
            }
            let pv = self
                .extract_pv_checked(pos, outcome, None)
                .unwrap_or_else(|| self.extract_pv(pos));
            self.last_pv = pv.clone();
            return (outcome, pv, self.nodes);
        }

        let outcome = self.solve_outcome(pos);
        if outcome == Outcome::Draw {
            return (outcome, self.extract_pv(pos), self.nodes);
        }

        let _ = self.find_ppv(pos, outcome);
        self.refine_sppv(pos, outcome, |_| {});
        let pv = if self.last_pv.is_empty() {
            self.extract_pv(pos)
        } else {
            self.last_pv.clone()
        };

        (outcome, pv, self.nodes)
    }

    fn begin_run(&mut self) {
        self.nodes = 0;
        self.child_evals = 0;
        self.start = Instant::now();
        self.deadline = self.start + self.timeout;
        self.path_stack.clear();
        self.path_code = 0;
        self.last_pv.clear();
    }

    pub fn child_evaluations(&self) -> u64 {
        self.child_evals
    }

    fn reset_search_state(&mut self) {
        self.path_stack.clear();
        self.path_code = 0;
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

    fn reset_history_and_killers(&mut self) {
        self.history = [[[0; 64]; 64]; 2];
        self.killers = [[Move::NONE; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH];
        self.history_age_counter = 0;
    }

    pub fn time_exceeded(&self) -> bool {
        Instant::now() >= self.deadline
    }
}
