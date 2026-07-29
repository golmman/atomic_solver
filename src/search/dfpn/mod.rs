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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use atomic_movegen::types::Move;

use crate::position::{Outcome, Position};

use super::ordering::StaticAtomicScorer;
use super::tt::TranspositionTable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProofMode {
    /// Stop as soon as the result is proven.
    Outcome,
    /// Defender replies are longest; attacker moves are any winning move.
    Ppv,
    /// Fully minimax: shortest attacker wins, longest defender replies.
    Sppv,
}

const DEFAULT_EPSILON: f64 = 0.125;
const TIMEOUT_SECS: u64 = 5;
const DEFAULT_MAX_PV_PLIES: usize = 1000;

/// Reason the search stopped, recorded for the pre-exit hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitReason {
    Timeout,
    Quit,
    Complete,
}

impl std::fmt::Display for ExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExitReason::Timeout => write!(f, "Timeout"),
            ExitReason::Quit => write!(f, "Quit"),
            ExitReason::Complete => write!(f, "Complete"),
        }
    }
}

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
    proof_mode: ProofMode,
    timeout: Duration,
    last_pv: Vec<Move>,
    history: [[[i32; 64]; 64]; 2],
    killers: [[Move; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH],
    history_age_counter: u64,
    max_ply: usize,
    max_depth_reached: u32,
    bootstrap_success_depth: Option<u32>,
    bootstrap_fail_depth: u32,
    // Optional repetition-path prefix for bounded searches that are run in the
    // context of a longer line (e.g. verifying a defender reply against a PPV).
    prefix_path: Option<(Vec<u64>, u64)>,
    linear_chunks: bool,
    chunk_increment: u64,
    chunk_multiplier_num: u64,
    chunk_multiplier_den: u64,
    stop_flag: Option<Arc<AtomicBool>>,
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
            proof_mode: ProofMode::Outcome,
            timeout: Duration::from_secs(TIMEOUT_SECS),
            last_pv: Vec::new(),
            history: [[[0; 64]; 64]; 2],
            killers: [[Move::NONE; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH],
            history_age_counter: 0,
            max_ply: DEFAULT_MAX_PV_PLIES,
            max_depth_reached: 0,
            bootstrap_success_depth: None,
            bootstrap_fail_depth: 0,
            prefix_path: None,
            linear_chunks: false,
            chunk_increment: 500_000,
            chunk_multiplier_num: 2,
            chunk_multiplier_den: 1,
            stop_flag: None,
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

    pub fn set_linear_chunks(&mut self, linear: bool) {
        self.linear_chunks = linear;
    }

    pub fn set_chunk_increment(&mut self, increment: u64) {
        self.chunk_increment = increment.max(1);
    }

    pub fn set_chunk_multiplier(&mut self, num: u64, den: u64) {
        assert!(den > 0, "chunk multiplier denominator must be positive");
        assert!(num > 0, "chunk multiplier numerator must be positive");
        let g = gcd(num, den);
        self.chunk_multiplier_num = num / g;
        self.chunk_multiplier_den = den / g;
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

    pub fn set_stop_flag(&mut self, stop_flag: Option<Arc<AtomicBool>>) {
        self.stop_flag = stop_flag;
    }

    pub fn exit_reason(&self) -> ExitReason {
        if self
            .stop_flag
            .as_ref()
            .is_some_and(|f| f.load(Ordering::Acquire))
        {
            ExitReason::Quit
        } else if Instant::now() >= self.deadline {
            ExitReason::Timeout
        } else {
            ExitReason::Complete
        }
    }

    pub fn search_depth(
        &mut self,
        pos: &mut Position,
        max_depth: u32,
    ) -> (Outcome, Vec<Move>, u64) {
        self.begin_run();
        self.proof_mode = ProofMode::Outcome;
        let outcome = self.dfpn(pos, INF, INF, max_depth, u64::MAX, true);
        let pv = self.extract_pv(pos);
        (outcome, pv, self.nodes)
    }

    /// Run a bounded OR-node win search with a pre-populated repetition path.
    ///
    /// This is used by the `verify_ppv` example to check defender replies while
    /// preserving the history of the supplied PPV prefix. The repetition keys
    /// `prefix_keys` are the positions *before* `pos` is pushed onto the path
    /// stack; `prefix_path_code` is the XOR of `zobrist::path_random(...)` for
    /// the prefix moves using the same 1-indexed depths as `dfpn`.
    ///
    /// Internally this runs the staged solver (with shortest-PV refinement) on
    /// the child position and returns a Win only if the shortest proven win is
    /// within `max_depth` plies.
    pub fn search_depth_with_prefix(
        &mut self,
        pos: &mut Position,
        max_depth: u32,
        prefix_keys: &[u64],
        prefix_path_code: u64,
    ) -> (Outcome, u32, u64) {
        let saved_prefix = self.prefix_path.take();
        let saved_refine = self.refine_shortest;
        self.prefix_path = Some((prefix_keys.to_vec(), prefix_path_code));
        self.refine_shortest = true;

        let (outcome, pv, nodes) = self.solve(pos);

        self.prefix_path = saved_prefix;
        self.refine_shortest = saved_refine;

        let depth = pv.len() as u32;
        if outcome == Outcome::Win && depth <= max_depth {
            (Outcome::Win, depth, nodes)
        } else {
            (Outcome::Draw, 0, nodes)
        }
    }

    /// Run the solver to a decisive outcome or the configured timeout.
    ///
    /// Internally this uses a pure work-bounded iterative-deepening loop.  The
    /// depth bound is effectively unbounded (`u32::MAX`); the search is stopped
    /// and resumed by doubling work chunks, reusing the transposition table
    /// between chunks.  This prioritizes proving decisive outcomes for deep
    /// positions.  A concrete decisive PV depth is recorded for the follow-up
    /// `find_ppv` and `refine_sppv` stages.
    pub fn solve_outcome(&mut self, pos: &mut Position) -> Outcome {
        self.begin_run();
        self.proof_mode = ProofMode::Outcome;

        let mut outcome = Outcome::Draw;
        let mut chunk = 500_000u64;
        let mut success_depth: Option<u32> = None;
        let mut last_child_evals_before = 0u64;

        while !self.time_exceeded() {
            self.reset_search_state();
            last_child_evals_before = self.child_evals;
            outcome = self.dfpn(pos, INF, INF, u32::MAX, chunk, true);

            if outcome != Outcome::Draw {
                if let Some(entry) = self.tt.probe(pos.hash())
                    && entry.outcome.is_some()
                {
                    success_depth = Some(entry.depth);
                }
                if success_depth.is_none()
                    && let Some(pv) = self.extract_pv_checked(pos, outcome, None)
                {
                    success_depth = Some(pv.len() as u32);
                }
                if success_depth.is_none() {
                    // Last-resort cap so the follow-up stages have a finite bound.
                    success_depth = Some(self.max_ply as u32);
                }
                break;
            }

            let work_done = self.child_evals - last_child_evals_before;
            if self.linear_chunks {
                chunk = chunk.saturating_add(self.chunk_increment);
            } else {
                chunk = ((chunk as u128 * self.chunk_multiplier_num as u128)
                    / self.chunk_multiplier_den as u128) as u64;
            }
            self.log_chunk(work_done, chunk, "solve_outcome");
            if chunk == u64::MAX {
                break;
            }
        }

        // If the work loop ran out of budget without a decisive result, spend the
        // remaining wall-clock time on a single unbounded search.  Keep the table
        // and history from the work chunks; only reset path state.
        if outcome == Outcome::Draw && !self.time_exceeded() {
            self.reset_search_state();
            let work_done = self.child_evals - last_child_evals_before;
            self.log_chunk(work_done, u64::MAX, "solve_outcome_fallback");
            outcome = self.dfpn(pos, INF, INF, u32::MAX, u64::MAX, true);

            if outcome != Outcome::Draw {
                if let Some(entry) = self.tt.probe(pos.hash())
                    && entry.outcome.is_some()
                {
                    success_depth = Some(entry.depth);
                }
                if success_depth.is_none()
                    && let Some(pv) = self.extract_pv_checked(pos, outcome, None)
                {
                    success_depth = Some(pv.len() as u32);
                }
                if success_depth.is_none() {
                    success_depth = Some(self.max_ply as u32);
                }
            }
        }

        self.bootstrap_success_depth = success_depth;
        // A pure work-bounded loop has no reliable "deepest searched depth".
        // Zero is a safe lower bound: a non-terminal position cannot win or lose
        // in zero plies, so refinement starts from there.
        self.bootstrap_fail_depth = 0;
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
            self.proof_mode = ProofMode::Ppv;
            // A PPV needs the longest defender replies, so the proof search is
            // allowed to run until the timeout as long as a proven winning
            // depth is known.
            if !self.time_exceeded() {
                self.dfpn(pos, INF, INF, depth, u64::MAX, true);
            }
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
        let mut hi = start_depth;
        let mut lo = self.bootstrap_fail_depth;

        // If last_pv is empty, use hi as the initial best length so any proven PV
        // at a probe below hi is reported as shorter.
        let mut current_best_len = if self.last_pv.is_empty() {
            hi
        } else {
            self.last_pv.len() as u32
        };

        while hi > lo + 1 && !self.time_exceeded() {
            let probe = lo + (hi - lo) / 2;
            let mut chunk = 500_000u64;
            let mut proved_at_probe = false;

            // A few retries with doubling work avoid false negatives caused by a
            // tight budget; if the depth bound itself is too low, the retries are
            // cheap because the tree is shallow.
            for _retry in 0..3 {
                if self.time_exceeded() {
                    break;
                }
                self.reset_search_state();
                self.proof_mode = ProofMode::Sppv;
                let child_evals_before = self.child_evals;
                let o = self.dfpn(pos, INF, INF, probe, chunk, true);

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
                    proved_at_probe = true;
                    break;
                }

                let work_done = self.child_evals - child_evals_before;
                if self.linear_chunks {
                    chunk = chunk.saturating_add(self.chunk_increment);
                } else {
                    chunk = ((chunk as u128 * self.chunk_multiplier_num as u128)
                        / self.chunk_multiplier_den as u128) as u64;
                }
                self.log_chunk(work_done, chunk, "refine_sppv");
                if chunk == u64::MAX {
                    break;
                }
            }

            if self.time_exceeded() {
                break;
            }

            if proved_at_probe {
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
            self.proof_mode = ProofMode::Outcome;
            let outcome = self.dfpn(pos, INF, INF, u32::MAX, u64::MAX, true);
            if outcome != Outcome::Draw {
                if let Some(entry) = self.tt.probe(pos.hash())
                    && entry.outcome.is_some()
                {
                    self.bootstrap_success_depth = Some(entry.depth);
                }
                if self.bootstrap_success_depth.is_none()
                    && let Some(pv) = self.extract_pv_checked(pos, outcome, None)
                {
                    self.bootstrap_success_depth = Some(pv.len() as u32);
                }
                if self.bootstrap_success_depth.is_none() {
                    self.bootstrap_success_depth = Some(self.max_ply as u32);
                }
                self.bootstrap_fail_depth = 0;
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

    /// Convenience entry point that accepts an external stop flag and writes the
    /// final exit reason to `exit_reason`.
    pub fn search_with_settings(
        &mut self,
        pos: &mut Position,
        stop_flag: Option<Arc<AtomicBool>>,
        exit_reason: &mut ExitReason,
    ) -> (Outcome, Vec<Move>, u64) {
        self.set_stop_flag(stop_flag);
        let result = self.solve(pos);
        *exit_reason = self.exit_reason();
        result
    }

    fn begin_run(&mut self) {
        self.nodes = 0;
        self.child_evals = 0;
        self.start = Instant::now();
        self.deadline = self.start + self.timeout;
        self.path_stack.clear();
        self.path_code = 0;
        self.last_pv.clear();
        self.max_depth_reached = 0;
        if let Some((keys, code)) = &self.prefix_path {
            self.path_stack = keys.clone();
            self.path_code = *code;
        }
    }

    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    pub fn child_evaluations(&self) -> u64 {
        self.child_evals
    }

    fn reset_search_state(&mut self) {
        self.path_stack.clear();
        self.path_code = 0;
        self.max_depth_reached = 0;
        if let Some((keys, code)) = &self.prefix_path {
            self.path_stack = keys.clone();
            self.path_code = *code;
        }
    }

    fn log_chunk(&self, work_done: u64, next_chunk: u64, label: &str) {
        let elapsed = self.start.elapsed();
        let secs = elapsed.as_secs_f64();
        let nps = if secs > 0.0 {
            self.nodes as f64 / secs
        } else {
            0.0
        };
        eprintln!(
            "[{label}] chunk done: work_done={work_done} next_chunk={next_chunk} elapsed={secs:.3}s max_depth={} nodes={} nps={nps:.0}",
            self.max_depth_reached, self.nodes
        );
    }

    pub(super) fn path_contains(&self, key: u64) -> bool {
        self.path_stack.contains(&key)
    }

    pub(super) fn path_push(&mut self, key: u64) {
        self.path_stack.push(key);
        self.max_depth_reached = self.max_depth_reached.max(self.path_stack.len() as u32);
    }

    pub(super) fn path_pop(&mut self) {
        self.path_stack.pop();
    }

    pub fn time_exceeded(&self) -> bool {
        if let Some(flag) = &self.stop_flag
            && flag.load(Ordering::Acquire)
        {
            return true;
        }
        Instant::now() >= self.deadline
    }
}
