//! Sequential DF-PN+ solver for atomic chess.
//!
//! The state definition and exit/PV types live here; `config` holds the
//! configuration surface, and `run` holds the search driver and solve entry
//! points. The split is by change reason, not for performance: module
//! placement is codegen-neutral in this build (`lto = true`,
//! `codegen-units = 1`).
//!
//! Pacing uses a deterministic restart schedule (`RestartSchedule::SHIPPED`:
//! base 500_000 child evals, geometric ×2, no cutoff) — a diversification
//! device, not a budget. The hard child-eval budget is enforced per node
//! (`child_eval_budget_exceeded` at the frame-loop head), independent of pacing.

mod cheap_probe;
mod children;
mod config;
mod core;
mod fraction;
mod history;
mod move_class;
mod pv;
mod quiet_group;
mod repetition_cache;
mod restart_schedule;
mod run;
mod selection;

use fraction::fraction_from_f64;
pub use restart_schedule::{Growth, RestartSchedule};

#[cfg(test)]
mod tests;

use children::{ChildInfo, ChildPrecompute};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::Move;

use crate::position::{Outcome, Position};
use crate::proof_event::{NodeProven, ProofEvent};

use super::ordering::StaticAtomicScorer;
use super::preflight::PreflightReport;
use super::tt::TranspositionTable;

use repetition_cache::RepetitionCache;

const DEFAULT_EPSILON: f64 = 0.125;
const TIMEOUT_SECS: u64 = 5;
const DEFAULT_MAX_PV_PLIES: usize = 1000;
const DEFAULT_REFINE_CAP_FACTOR: f64 = 0.25;
/// Floor for the per-refinement-round child-eval cap, so searches with tiny
/// first-outcome work stay effectively uncapped.
const MIN_REFINE_ROUND_EVALS: u64 = 1_000_000;
/// Wall-clock sampler: `time_exceeded` re-reads `Instant::now()` only once
/// every this many `dfpn` entries (`self.nodes`); between reads it reuses the
/// cached `Instant`. See [`Search::time_exceeded`] for the contract.
const CLOCK_SAMPLE_INTERVAL: u64 = 4096;

/// Path-presence Bloom filter slots (512 B at `u16` per slot). A zero slot is
/// a sound negative for `path_contains`; a nonzero slot falls back to the
/// linear scan, so a false positive costs a scan, never a wrong result.
const PATH_BLOOM_SLOTS: usize = 256;

/// Reason the search stopped, recorded for the pre-exit hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitReason {
    Timeout,
    Quit,
    /// Memory-limit abort driven by `set_memory_limited`. Not wired by the
    /// search CLI anymore (the search is resource-bounded: RAM = TT only);
    /// this remains a reconstruct-side contract — the reconstruction walker
    /// (`src/reconstruct/walker.rs`) sets the flag for the *builder's*
    /// budget and aborts local prefix-solves on it.
    MemoryLimit,
    BudgetExhausted,
    Complete,
}

impl std::fmt::Display for ExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExitReason::Timeout => write!(f, "Timeout"),
            ExitReason::Quit => write!(f, "Quit"),
            ExitReason::MemoryLimit => write!(f, "MemoryLimit"),
            ExitReason::BudgetExhausted => write!(f, "BudgetExhausted"),
            ExitReason::Complete => write!(f, "Complete"),
        }
    }
}

/// How the PV of the last solve relates to optimality.
///
/// This qualifies the PV *length*, not its validity: the returned line is the
/// informational best-effort PV from the transposition table and is never
/// validated as a proof by `Search` (that is the proof-tree layer's job).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PvStatus {
    /// No decisive outcome: there is no PV to qualify.
    None,
    /// PV came from the first-outcome phase (including
    /// `first_outcome_only` runs). Length is informational.
    FirstOutcome,
    /// The last refinement round ended in natural exhaustion at
    /// `bound = pv_len - 2` (the bounded tree was fully explored with no
    /// decisive line), or the returned win is 1 move long. Within the
    /// solver's search semantics, no shorter win exists. This says nothing
    /// about whether the returned move sequence is a valid proof.
    ProvenShortest,
    /// The last refinement round was cut by the per-round work cap or by a
    /// global resource limit, or ended decisive-but-not-shorter. A shorter
    /// win may exist.
    Unproven,
    /// PV extracted from a replay-verified pre-phase certificate (plan13):
    /// a winning, cycle-free line with exact ranks, so the length is the
    /// exact mate distance for the region-closure architecture. This
    /// qualifies the PV length like the other variants, never the validity
    /// of the line as a proof (the certificate was verified, but by the
    /// pre-phase's own replay, not by the proof-tree layer). Never returned
    /// for a pre-phase Draw claim: draws keep `None` (no decisive outcome,
    /// no PV to qualify) and are reported via `Search::preflight_report()`.
    PreflightProof,
}

/// Why `bounded_search` returned. Only meaningful when `outcome == Draw` for
/// the non-`Decisive` variants; a decisive return always reports `Decisive`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RoundTermination {
    /// A decisive outcome was found.
    Decisive,
    /// The bounded tree at `max_depth` was fully explored with no decisive
    /// line (the final slice was not fully spent and no resource limit fired).
    Exhausted,
    /// The per-round work cap (`round_work_cap`) stopped the round before
    /// exhaustion (`call_slice == 0` reached via the round cap).
    CapCut,
    /// Wall time, the stop flag, the memory flag, or the global child-eval
    /// budget stopped the round before exhaustion. The budget is enforced per
    /// node; the restart slice only paces the search. Attribution is derived
    /// at loop exit: a resource limit that fires while the final slice is
    /// under-spent is `ResourceCut`, unless the round cap is the strictly
    /// smaller remaining constraint (then `CapCut`; ties stay `ResourceCut`).
    ResourceCut,
}

pub struct Search {
    tt: TranspositionTable,
    /// Repetition keys on the current search path (root to parent). Mutating
    /// this vector directly would desynchronize the incremental bookkeeping
    /// (`path_context_hash`, `path_bloom`): every mutation must go through
    /// [`Search::path_push`] / [`Search::path_pop`] (or
    /// [`Search::reset_search_state`]).
    path_stack: Vec<u64>,
    /// Order-independent context hash of `path_stack`
    /// (`RepetitionCache::context_hash`), maintained incrementally by
    /// `path_push`/`path_pop` so the repetition-cache key costs O(1) per node.
    path_context_hash: u64,
    /// Path-presence Bloom filter indexed by the low 8 bits of the repetition
    /// key. Counts, not bits, so duplicate keys (which the no-duplicate path
    /// invariant does not currently produce) stay correct.
    path_bloom: [u16; PATH_BLOOM_SLOTS],
    /// Test-only census counter: `path_contains` calls decided by a zero Bloom
    /// slot without the linear scan. Reset per run in `begin_run`.
    #[cfg(test)]
    path_bloom_negatives: std::cell::Cell<u64>,
    nodes: u64,
    child_evals: u64,
    start: Instant,
    deadline: Instant,
    epsilon_num: u64,
    epsilon_den: u64,
    scorer: StaticAtomicScorer,
    first_outcome_only: bool,
    timeout: Duration,
    child_eval_budget: u64,
    refine_cap_factor_num: u64,
    refine_cap_factor_den: u64,
    refine_cap_min: u64,
    first_outcome_evals: u64,
    refinement_rounds: u32,
    refinement_evals: u64,
    pv_status: PvStatus,
    last_round_cap_cut: bool,
    history: [[[i32; 64]; 64]; 2],
    killers: [[Move; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH],
    history_age_counter: u64,
    max_ply: usize,
    max_depth_reached: u32,
    prefix_path: Option<Vec<u64>>,
    restart_schedule: RestartSchedule,
    /// Restart boundaries crossed in the current run (reset by `begin_run`).
    restarts: u64,
    stop_flag: Option<Arc<AtomicBool>>,
    memory_limited: Option<Arc<AtomicBool>>,
    /// Next `self.nodes` value at which `time_exceeded` re-reads the wall
    /// clock (`Cell` because the hot call site takes `&self`).
    clock_sample_at: std::cell::Cell<u64>,
    /// Last wall-clock reading taken by `time_exceeded`.
    last_clock: std::cell::Cell<Instant>,
    proof_event_sender: Option<std::sync::mpsc::Sender<ProofEvent>>,
    move_stack: Vec<Move>,
    /// Per-depth pool of `ChildInfo` tables (`dfpn` frames borrow their
    /// depth's vector with `mem::take` and return it at frame exit).
    child_pool: Vec<Vec<ChildInfo>>,
    /// Per-depth pool of frame movegen slots (see `ChildPrecompute`): the
    /// entry of the `dfpn` frame at depth `d` generates its own legal moves
    /// into `precompute_pool[d]` and returns it at frame exit.
    precompute_pool: Vec<ChildPrecompute>,
    /// Per-depth pool of make/unmake scratch `StateInfo` slots for
    /// `evaluate_child` (see the `ChildPrecompute` "Eval-scratch slots"
    /// docs): the child evaluation at frame depth `d` plays/unplays its probe
    /// move on `eval_state_pool[d]` without touching the undo stack.
    eval_state_pool: Vec<StateInfo>,
    /// Reusable `(move, score)` scratch for `sort_moves`.
    sort_scratch: Vec<(Move, i32)>,
    /// Per-child key scratch for `evaluate_all_children`'s keys pre-pass
    /// (plan3): `(tt_key, repetition_key, rule50)` of the position after each
    /// child move, computed without playing it. See `Search::evaluate_child`.
    keys_scratch: Vec<(u64, u64, u16)>,
    /// Per-child scratch for the quiet-grouping plainness partition (provari
    /// plan3): `plain_scratch[i]` classifies `moves[i]` for the current
    /// `evaluate_all_children` sweep. Only the written prefix is read.
    plain_scratch: Vec<bool>,
    /// Test-only census counter (plan3): child evaluations decided by the
    /// pre-probe ladder without a make/unmake. Reset per run in `begin_run`.
    #[cfg(test)]
    preprobe_skips: u64,
    /// Per-search cache of repetition-dependent draw proofs (plan9), outside
    /// the TT: keyed by (full position hash, order-independent ancestor
    /// repetition-key context hash); stores only `Outcome::Draw` payloads
    /// (the proven depth). Cleared once per run in [`Search::begin_run`] —
    /// never per work chunk, never per refinement round — and never
    /// serialized into TT snapshots or proof artifacts.
    repetition_cache: RepetitionCache,
    /// Whether the detector-gated bounded pre-phase (plan13) may run before
    /// the DF-PN loop. Default `true`; disabled by the `--no-preflight` CLI
    /// flag.
    preflight_enabled: bool,
    /// Whether OR-node moves are given provari-style starting proof numbers
    /// (`move_class`): the initial estimate for a newly seen non-terminal
    /// child follows the move's class (check/double threat/threat/weak
    /// threat/hanging/quiet). Default `false`; the flag is an experimental
    /// heuristic, not a correctness switch.
    move_class_pn: bool,
    /// Whether OR-node plain moves are hidden behind one delayed group child
    /// (`quiet_group`), opened only when DF-PN selects it. Default `false`;
    /// an experimental heuristic, not a correctness switch.
    quiet_grouping: bool,
    /// Report of the last run's pre-phase hook, `None` when the hook has not
    /// run in the current run.
    preflight_report: Option<PreflightReport>,
}

impl Search {
    #[must_use]
    pub fn new(tt_mb: usize) -> Self {
        let (epsilon_num, epsilon_den) = fraction_from_f64(1.0 + DEFAULT_EPSILON);
        let (refine_cap_factor_num, refine_cap_factor_den) =
            fraction_from_f64(DEFAULT_REFINE_CAP_FACTOR);
        Self {
            tt: TranspositionTable::with_mb(tt_mb),
            path_stack: Vec::new(),
            path_context_hash: 0,
            path_bloom: [0; PATH_BLOOM_SLOTS],
            #[cfg(test)]
            path_bloom_negatives: std::cell::Cell::new(0),
            nodes: 0,
            child_evals: 0,
            start: Instant::now(),
            deadline: Instant::now(),
            epsilon_num,
            epsilon_den,
            scorer: StaticAtomicScorer::default(),
            first_outcome_only: false,
            timeout: Duration::from_secs(TIMEOUT_SECS),
            child_eval_budget: u64::MAX,
            refine_cap_factor_num,
            refine_cap_factor_den,
            refine_cap_min: MIN_REFINE_ROUND_EVALS,
            first_outcome_evals: 0,
            refinement_rounds: 0,
            refinement_evals: 0,
            pv_status: PvStatus::None,
            last_round_cap_cut: false,
            history: [[[0; 64]; 64]; 2],
            killers: [[Move::NONE; history::KILLER_SLOTS]; history::MAX_KILLER_DEPTH],
            history_age_counter: 0,
            max_ply: DEFAULT_MAX_PV_PLIES,
            max_depth_reached: 0,
            prefix_path: None,
            restart_schedule: RestartSchedule::SHIPPED,
            restarts: 0,
            stop_flag: None,
            memory_limited: None,
            clock_sample_at: std::cell::Cell::new(0),
            last_clock: std::cell::Cell::new(Instant::now()),
            proof_event_sender: None,
            move_stack: Vec::new(),
            child_pool: Vec::new(),
            precompute_pool: Vec::new(),
            eval_state_pool: Vec::new(),
            sort_scratch: Vec::new(),
            keys_scratch: Vec::new(),
            plain_scratch: Vec::new(),
            #[cfg(test)]
            preprobe_skips: 0,
            repetition_cache: RepetitionCache::new(),
            move_class_pn: false,
            quiet_grouping: false,
            preflight_enabled: true,
            preflight_report: None,
        }
    }

    fn emit_proof_node(&self, pos: &Position, outcome: Outcome, depth: u32) {
        if outcome == Outcome::Draw {
            return;
        }
        if let Some(sender) = &self.proof_event_sender {
            let event = ProofEvent::NodeProven(NodeProven::new(
                self.move_stack.clone(),
                pos.hash(),
                outcome,
                depth,
            ));
            let _ = sender.send(event);
        }
    }

    /// Total number of nodes (positions) visited by the search.
    #[must_use]
    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    /// Total number of child position evaluations performed.
    #[must_use]
    pub fn child_evaluations(&self) -> u64 {
        self.child_evals
    }

    /// Test-only census counter (plan3): child evaluations whose result came
    /// from the pre-probe ladder (a live TT entry) without playing the child
    /// move. Mechanism evidence for the pre-probe; not part of the output
    /// contract and not present in release builds.
    #[cfg(test)]
    pub(super) fn preprobe_skips(&self) -> u64 {
        self.preprobe_skips
    }

    /// Reset the path bookkeeping to the prefix path (the only path reset).
    ///
    /// This is the single place that seeds `path_stack`, `path_context_hash`
    /// and `path_bloom` together; a reset that touched only one of them would
    /// make the Bloom filter report false negatives (missed repetitions).
    fn reset_search_state(&mut self) {
        self.path_stack.clear();
        self.path_bloom.fill(0);
        self.path_context_hash = 0;
        self.move_stack.clear();
        self.max_depth_reached = 0;
        if let Some(keys) = &self.prefix_path {
            self.path_stack = keys.clone();
            for &key in &self.path_stack {
                let slot = Self::path_bloom_slot(key);
                self.path_bloom[slot] += 1;
            }
            self.path_context_hash = RepetitionCache::context_hash(&self.path_stack);
        }
    }

    fn log_chunk(&mut self, work_done: u64, next_chunk: u64, label: &str) {
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

    /// Bloom slot for one repetition key: the low 8 bits of the key.
    ///
    /// Distribution is a performance property, not a correctness one: a
    /// collision only costs a fallback linear scan.
    #[inline]
    fn path_bloom_slot(key: u64) -> usize {
        (key as usize) & (PATH_BLOOM_SLOTS - 1)
    }

    /// Whether `key` is on the current search path.
    ///
    /// Semantics are exactly `path_stack.contains(&key)`: a zero Bloom slot is
    /// a sound negative (the key cannot be present), while a nonzero slot
    /// falls back to the linear scan, so collisions only cost a scan.
    #[inline]
    pub(super) fn path_contains(&self, key: u64) -> bool {
        if self.path_bloom[Self::path_bloom_slot(key)] == 0 {
            #[cfg(test)]
            self.path_bloom_negatives
                .set(self.path_bloom_negatives.get() + 1);
            return false;
        }
        self.path_stack.contains(&key)
    }

    /// Push one repetition key onto the path, updating the Bloom counts and
    /// the incremental context hash.
    #[inline]
    pub(super) fn path_push(&mut self, key: u64) {
        let slot = Self::path_bloom_slot(key);
        debug_assert!(self.path_bloom[slot] < u16::MAX, "path deeper than u16");
        self.path_bloom[slot] += 1;
        self.path_context_hash = self
            .path_context_hash
            .wrapping_add(repetition_cache::mix_key(key));
        self.path_stack.push(key);
        self.max_depth_reached = self.max_depth_reached.max(self.path_stack.len() as u32);
    }

    /// Pop the last repetition key, undoing exactly what `path_push` added.
    #[inline]
    pub(super) fn path_pop(&mut self) {
        if let Some(key) = self.path_stack.pop() {
            let slot = Self::path_bloom_slot(key);
            debug_assert!(self.path_bloom[slot] > 0, "Bloom counter underflow");
            self.path_bloom[slot] -= 1;
            self.path_context_hash = self
                .path_context_hash
                .wrapping_sub(repetition_cache::mix_key(key));
        }
    }

    /// Test-only invariant check for the incremental path bookkeeping.
    ///
    /// Verifies that the context hash equals the reference fold and that the
    /// Bloom counts match the multiset of path keys exactly (which also
    /// implies that every zero-slot key is absent). Never called on the hot
    /// path.
    #[cfg(test)]
    fn assert_path_bookkeeping_consistent(&self) {
        assert_eq!(
            self.path_context_hash,
            RepetitionCache::context_hash(&self.path_stack),
            "incremental context hash diverged from the fold"
        );
        let mut expected = [0u16; PATH_BLOOM_SLOTS];
        for &key in &self.path_stack {
            expected[Self::path_bloom_slot(key)] += 1;
        }
        assert_eq!(
            self.path_bloom, expected,
            "Bloom counts diverged from the path multiset"
        );
    }

    /// Whether the wall-clock deadline, stop flag, or memory flag has fired.
    ///
    /// All call sites keep calling this; the `stop_flag` / `memory_limited`
    /// atomics are loaded on **every** call, but the `Instant::now()` read is
    /// sampled: the clock is only re-read when `self.nodes` has advanced by
    /// `CLOCK_SAMPLE_INTERVAL` dfpn entries since the last read, and the
    /// cached `Instant` is compared against the deadline otherwise.
    ///
    /// # Contract
    ///
    /// - Sampling only *delays* wall-deadline detection by at most
    ///   `CLOCK_SAMPLE_INTERVAL` dfpn entries: [`ExitReason::Timeout`]
    ///   granularity is coarse (previous behavior was exact). Wall-clock
    ///   tests use generous margins, so this is acceptable.
    /// - Budget mode is unaffected: [`Search::child_eval_budget_exceeded`] is
    ///   a pure integer compare, and [`ExitReason`] checks `BudgetExhausted`
    ///   before `Timeout`.
    #[must_use]
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
        if self.nodes >= self.clock_sample_at.get() {
            self.last_clock.set(Instant::now());
            self.clock_sample_at.set(self.nodes + CLOCK_SAMPLE_INTERVAL);
        }
        self.last_clock.get() >= self.deadline
    }
}
