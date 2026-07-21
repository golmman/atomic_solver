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

const DEFAULT_EPSILON: f64 = 0.25;
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
}

impl Search {
    pub fn new(tt_mb: usize) -> Self {
        let (epsilon_num, epsilon_den) = epsilon_fraction(1.0 + DEFAULT_EPSILON);
        Self {
            tt: TranspositionTable::with_mb(tt_mb),
            path_stack: Vec::new(),
            path_code: 0,
            nodes: 0,
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

    pub fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        self.begin_run();

        if self.refine_shortest {
            // Bootstrap: find any decisive result with a small depth budget,
            // doubling the budget until the position is solved. Do not refine
            // during bootstrap; we only need a winning outcome to start from.
            let saved_refine = self.refine_shortest;
            self.refine_shortest = false;

            let mut bootstrap_outcome = Outcome::Draw;
            let mut max_depth = 1u32;
            let mut success_depth: Option<u32> = None;
            let mut fail_depth = 0u32;

            while max_depth <= 64 {
                self.reset_search_state();
                self.tt.clear();
                bootstrap_outcome = self.dfpn(pos, INF, INF, max_depth, true);
                if self.time_exceeded() {
                    break;
                }
                if bootstrap_outcome != Outcome::Draw {
                    success_depth = Some(max_depth);
                    break;
                }
                fail_depth = max_depth;
                max_depth = max_depth.saturating_mul(2);
            }

            self.refine_shortest = saved_refine;

            if self.time_exceeded() {
                let pv = self.extract_pv(pos);
                (bootstrap_outcome, pv, self.nodes)
            } else if let Some(success) = success_depth {
                if let Some(pv) = self.extract_pv_checked(pos, bootstrap_outcome, None) {
                    self.last_pv = pv;
                } else {
                    self.last_pv = self.extract_pv(pos);
                }
                self.solve_refined(pos, bootstrap_outcome, success, fail_depth)
            } else {
                // Bootstrap did not find a decisive result; fall back to an
                // unbounded search with binary refinement.
                self.reset_search_state();
                self.tt.clear();
                self.reset_history_and_killers();
                self.solve_refined_unbounded(pos)
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

    fn solve_refined(
        &mut self,
        pos: &mut Position,
        best_outcome: Outcome,
        success_depth: u32,
        fail_depth: u32,
    ) -> (Outcome, Vec<Move>, u64) {
        // Iterative deepening downward from the bootstrap success depth.
        // Reuse the transposition table and move-ordering history between
        // probes; clear only the path-dependent state.
        let mut best_pv = self.last_pv.clone();
        let mut lo = fail_depth;
        let mut hi = success_depth;

        while hi > lo + 1 && !self.time_exceeded() {
            let probe = hi - 1;
            self.reset_search_state();
            let outcome = self.dfpn(pos, INF, INF, probe, true);

            if self.time_exceeded() {
                break;
            }

            if outcome == best_outcome {
                hi = probe;
                if let Some(pv) = self.extract_pv_checked(pos, outcome, None) {
                    self.last_pv = pv;
                    best_pv = self.last_pv.clone();
                }
            } else {
                lo = probe;
            }
        }

        let pv = if best_pv.is_empty() {
            self.extract_pv(pos)
        } else {
            best_pv
        };
        (best_outcome, pv, self.nodes)
    }

    fn solve_refined_unbounded(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        // Fallback: first find any win/loss without a depth bound to get an
        // initial PV, then binary search the smallest depth bound that still
        // yields the same outcome. Start each probe with a clean search state.
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

            // Validate the binary-search answer at the exact depth.
            self.reset_search_state();
            self.tt.clear();
            self.reset_history_and_killers();
            let o = self.dfpn(pos, INF, INF, lo, true);
            if o == outcome {
                if let Some(pv) = self.extract_pv_checked(pos, outcome, None) {
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

    fn begin_run(&mut self) {
        self.nodes = 0;
        self.start = Instant::now();
        self.deadline = self.start + self.timeout;
        self.path_stack.clear();
        self.path_code = 0;
        self.last_pv.clear();
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

    fn time_exceeded(&self) -> bool {
        Instant::now() >= self.deadline
    }
}
