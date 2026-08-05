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
use crate::proof_tree::{NodeProven, ProofMessage};

use super::ordering::StaticAtomicScorer;
use super::tt::TranspositionTable;

const DEFAULT_EPSILON: f64 = 0.125;
const TIMEOUT_SECS: u64 = 5;
const DEFAULT_MAX_PV_PLIES: usize = 1000;

/// Reason the search stopped, recorded for the pre-exit hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitReason {
    Timeout,
    Quit,
    MemoryLimit,
    Complete,
}

impl std::fmt::Display for ExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExitReason::Timeout => write!(f, "Timeout"),
            ExitReason::Quit => write!(f, "Quit"),
            ExitReason::MemoryLimit => write!(f, "MemoryLimit"),
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
    first_outcome_only: bool,
    timeout: Duration,
    history: [[[i32; 64]; 64]; 2],
    killers: [[Move; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH],
    history_age_counter: u64,
    max_ply: usize,
    max_depth_reached: u32,
    prefix_path: Option<(Vec<u64>, u64)>,
    linear_chunks: bool,
    chunk_increment: u64,
    chunk_multiplier_num: u64,
    chunk_multiplier_den: u64,
    stop_flag: Option<Arc<AtomicBool>>,
    memory_limited: Option<Arc<AtomicBool>>,
    proof_tree_sender: Option<std::sync::mpsc::Sender<crate::proof_tree::ProofMessage>>,
    move_stack: Vec<Move>,
    proof_path: String,
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
            first_outcome_only: false,
            timeout: Duration::from_secs(TIMEOUT_SECS),
            history: [[[0; 64]; 64]; 2],
            killers: [[Move::NONE; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH],
            history_age_counter: 0,
            max_ply: DEFAULT_MAX_PV_PLIES,
            max_depth_reached: 0,
            prefix_path: None,
            linear_chunks: false,
            chunk_increment: 500_000,
            chunk_multiplier_num: 2,
            chunk_multiplier_den: 1,
            stop_flag: None,
            memory_limited: None,
            proof_tree_sender: None,
            move_stack: Vec::new(),
            proof_path: "root".to_string(),
        }
    }

    pub fn set_first_outcome_only(&mut self, value: bool) {
        self.first_outcome_only = value;
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

    pub fn set_memory_limited(&mut self, memory_limited: Option<Arc<AtomicBool>>) {
        self.memory_limited = memory_limited;
    }

    pub fn set_proof_tree_sender(
        &mut self,
        sender: Option<std::sync::mpsc::Sender<crate::proof_tree::ProofMessage>>,
    ) {
        self.proof_tree_sender = sender;
    }

    fn clear_proof_tree(&self) {
        if let Some(sender) = &self.proof_tree_sender {
            let _ = sender.send(crate::proof_tree::ProofMessage::Clear);
        }
    }

    fn emit_proof_node(&self, outcome: Outcome, depth: u32) {
        if outcome == Outcome::Draw {
            return;
        }
        if let Some(sender) = &self.proof_tree_sender {
            let mv = self.move_stack.last().copied().unwrap_or(Move::NONE);
            let _ = sender.send(ProofMessage::NodeProven(NodeProven {
                path: self.proof_path.clone(),
                mv,
                outcome,
                depth,
            }));
        }
    }

    pub fn exit_reason(&self) -> ExitReason {
        if self
            .memory_limited
            .as_ref()
            .is_some_and(|f| f.load(Ordering::Acquire))
        {
            ExitReason::MemoryLimit
        } else if self
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

    /// Run a single bounded, work-chunked `dfpn` search for `max_depth` plies.
    ///
    /// For finite `max_depth` this stops as soon as the bounded tree is exhausted
    /// (the search cannot consume its full work budget). For `max_depth =
    /// u32::MAX` it keeps increasing the work budget until a decisive outcome is
    /// found, time expires, or the budget saturates.
    fn bounded_search(&mut self, pos: &mut Position, max_depth: u32) -> (Outcome, Vec<Move>) {
        let mut outcome = Outcome::Draw;
        let mut chunk = 500_000u64;
        let mut last_child_evals_before;

        while !self.time_exceeded() && chunk > 0 {
            self.reset_search_state();
            last_child_evals_before = self.child_evals;
            outcome = self.dfpn(pos, INF, INF, max_depth, chunk, true);
            if outcome != Outcome::Draw {
                break;
            }

            let work_done = self.child_evals - last_child_evals_before;
            if work_done < chunk {
                // The search did not use its full work budget, so the bounded
                // tree was exhausted without finding a decisive line. More work
                // cannot change the outcome at this depth.
                break;
            }

            chunk = if self.linear_chunks {
                chunk.saturating_add(self.chunk_increment)
            } else {
                ((chunk as u128 * self.chunk_multiplier_num as u128)
                    / self.chunk_multiplier_den as u128) as u64
            };
            self.log_chunk(work_done, chunk, "bounded_search");
        }

        let pv = if outcome == Outcome::Draw {
            self.extract_pv(pos)
        } else {
            self.extract_pv_checked(pos, outcome, None)
                .unwrap_or_else(|| self.extract_pv(pos))
        };
        (outcome, pv)
    }

    pub fn search_depth(
        &mut self,
        pos: &mut Position,
        max_depth: u32,
    ) -> (Outcome, Vec<Move>, u64) {
        self.begin_run();
        self.clear_proof_tree();
        let (outcome, pv) = self.bounded_search(pos, max_depth);
        (outcome, pv, self.nodes)
    }

    /// Run a bounded OR-node win search with a pre-populated repetition path.
    ///
    /// This is used by the `verify_ppv` example to check defender replies while
    /// preserving the history of the supplied PPV prefix. The repetition keys
    /// `prefix_keys` are the positions *before* `pos` is pushed onto the path
    /// stack; `prefix_path_code` is the XOR of `zobrist::path_random(...)` for
    /// all moves that led to `pos` (including the final move into `pos`), using
    /// the same 1-indexed depths as `dfpn`.
    pub fn search_depth_with_prefix(
        &mut self,
        pos: &mut Position,
        max_depth: u32,
        prefix_keys: &[u64],
        prefix_path_code: u64,
    ) -> (Outcome, u32, u64) {
        let saved_prefix = self.prefix_path.take();
        self.prefix_path = Some((prefix_keys.to_vec(), prefix_path_code));
        self.begin_run();

        let (outcome, pv) = self.bounded_search(pos, max_depth);
        let depth = if outcome == Outcome::Win {
            pv.len() as u32
        } else {
            0
        };

        self.prefix_path = saved_prefix;
        (outcome, depth, self.nodes)
    }

    /// Solve a position, returning the decisive outcome and the shortest PV
    /// found within the configured timeout.
    pub fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        self.solve_with_progress(pos, |_, _| {})
    }

    /// Solve a position and call `on_progress` for every newly found decisive
    /// line. The final returned PV is the shortest line discovered before the
    /// timeout or the first outcome if `first_outcome_only` is set.
    pub fn solve_with_progress<F>(
        &mut self,
        pos: &mut Position,
        mut on_progress: F,
    ) -> (Outcome, Vec<Move>, u64)
    where
        F: FnMut(Outcome, &[Move]),
    {
        self.begin_run();
        self.clear_proof_tree();

        // 1. First decisive outcome (work-chunked, unbounded depth).
        let (mut outcome, mut pv) = self.bounded_search(pos, u32::MAX);
        if outcome != Outcome::Draw || !pv.is_empty() {
            on_progress(outcome, &pv);
        }

        // 2. Iteratively tighten the bound by two plies, unless the user asked
        //    for the first outcome only.
        let mut n = pv.len() as u32;
        while !self.first_outcome_only && outcome != Outcome::Draw && n > 2 && !self.time_exceeded()
        {
            let bound = n - 2;
            let (new_outcome, new_pv) = self.bounded_search(pos, bound);
            if new_outcome == Outcome::Draw || new_pv.len() as u32 >= n {
                break;
            }
            outcome = new_outcome;
            pv = new_pv;
            n = pv.len() as u32;
            on_progress(outcome, &pv);
        }

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
        self.move_stack.clear();
        self.proof_path = "root".to_string();
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
        self.move_stack.clear();
        self.proof_path = "root".to_string();
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
        if let Some(flag) = &self.memory_limited
            && flag.load(Ordering::Acquire)
        {
            return true;
        }
        Instant::now() >= self.deadline
    }
}
