//! Sequential DF-PN+ solver for atomic chess.
//!
//! This module is intentionally larger than 10 KB because the search loop,
//! public entry points, and move ordering all live in a single `Search`
//! implementation for performance and to avoid extra cross-module coupling.
//!
//! Pacing uses a deterministic restart schedule (`RestartSchedule::SHIPPED`:
//! base 500_000 child evals, geometric ×2, no cutoff) — a diversification
//! device, not a budget. The hard child-eval budget is enforced per node
//! (`child_eval_budget_exceeded` at the frame-loop head), independent of pacing.

mod children;
mod core;
mod fraction;
mod history;
mod pv;
mod repetition_cache;
mod restart_schedule;
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
use crate::zobrist::INF;

use super::ordering::StaticAtomicScorer;
use super::preflight::{self, PreflightExit, PreflightReport};
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
    path_stack: Vec<u64>,
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
            repetition_cache: RepetitionCache::new(),
            preflight_enabled: true,
            preflight_report: None,
        }
    }

    /// Enable or disable the detector-gated bounded pre-phase (plan13).
    /// Enabled by default; the `--no-preflight` CLI flag maps here.
    pub fn set_preflight_enabled(&mut self, enabled: bool) {
        self.preflight_enabled = enabled;
    }

    /// Report of the pre-phase hook of the last
    /// [`Search::solve`]/[`Search::solve_with_progress`]/[`Search::search_depth`]
    /// call: whether it decided the root (and as what) or deferred (and why).
    /// `None` before the first call in a run.
    #[must_use]
    pub fn preflight_report(&self) -> Option<PreflightReport> {
        self.preflight_report
    }

    pub fn set_first_outcome_only(&mut self, value: bool) {
        self.first_outcome_only = value;
    }

    pub fn set_scorer(&mut self, scorer: StaticAtomicScorer) {
        self.scorer = scorer;
    }

    pub fn scorer(&self) -> &StaticAtomicScorer {
        &self.scorer
    }

    pub fn set_timeout(&mut self, seconds: u64) {
        self.timeout = Duration::from_secs(seconds);
    }

    /// Bound the search by cumulative child evaluations instead of wall time.
    /// `u64::MAX` (the default) means unbounded.
    ///
    /// The budget is enforced per node: every `dfpn` frame loop head checks
    /// [`Search::child_eval_budget_exceeded`], and the `bounded_search` loop
    /// condition checks it between slices. The restart schedule is pacing
    /// only and never enforces the budget. The budget can be crossed by at
    /// most the remainder of one `evaluate_all_children` sweep (fewer than
    /// 256 child evaluations): unwinding ancestors re-check at their loop
    /// heads and a freshly entered child re-checks before its first sweep.
    ///
    /// A budget-exhausted search returns `Outcome::Draw`, stores only
    /// unsolved transposition entries, and reports
    /// [`ExitReason::BudgetExhausted`] from [`Search::exit_reason`]; it is
    /// never reported by [`Search::time_exceeded`], which stays exclusively
    /// about wall time.
    pub fn set_child_eval_budget(&mut self, budget: u64) {
        self.child_eval_budget = budget;
    }

    /// Set the per-refinement-round work cap from a factor of the
    /// first-outcome phase's child evaluations.
    ///
    /// Each refinement round in [`Search::solve_with_progress`] is capped at
    /// `max(MIN_REFINE_ROUND_EVALS, factor * first_outcome_evals)` child
    /// evaluations; a round that hits the cap without proving a shorter
    /// decisive line is abandoned and refinement stops. This bounds the cost
    /// of futile refinement rounds deterministically, without wall-clock
    /// dependence.
    ///
    /// `factor` must be non-negative. A factor of `0.0` disables capping
    /// entirely (pre-cap behavior); internally this is represented by setting
    /// `refine_cap_min = u64::MAX`, which makes every round cap effectively
    /// unlimited.
    pub fn set_refine_cap_factor(&mut self, factor: f64) {
        assert!(
            factor >= 0.0,
            "refine cap factor must be >= 0.0, got {factor}"
        );
        if factor == 0.0 {
            self.refine_cap_min = u64::MAX;
        } else {
            let (num, den) = fraction_from_f64(factor);
            self.refine_cap_factor_num = num;
            self.refine_cap_factor_den = den;
            self.refine_cap_min = MIN_REFINE_ROUND_EVALS;
        }
    }

    /// Force a concrete per-round child-eval cap, bypassing the factor
    /// computation. Test-only helper used to make the cap bind on small
    /// positions.
    #[cfg(test)]
    pub(crate) fn set_refine_round_cap_for_test(&mut self, cap: u64) {
        self.refine_cap_factor_num = 0;
        self.refine_cap_factor_den = 1;
        self.refine_cap_min = cap;
    }

    /// Child evaluations spent by the first-outcome phase of the last
    /// [`Search::solve`]/[`Search::solve_with_progress`] call.
    #[must_use]
    pub fn first_outcome_evaluations(&self) -> u64 {
        self.first_outcome_evals
    }

    /// Number of refinement rounds attempted by the last solve (successful or
    /// not). Zero when refinement never ran.
    #[must_use]
    pub fn refinement_rounds(&self) -> u32 {
        self.refinement_rounds
    }

    /// Total child evaluations spent across all refinement rounds of the last
    /// solve.
    #[must_use]
    pub fn refinement_evaluations(&self) -> u64 {
        self.refinement_evals
    }

    /// How the PV returned by the last [`Search::solve`] /
    /// [`Search::solve_with_progress`] call relates to optimality. See
    /// [`PvStatus`] for the exact meaning of each variant; in particular,
    /// [`PvStatus::ProvenShortest`] is a statement about PV *length* within
    /// the solver's own search semantics, not a validation of the returned
    /// move sequence.
    #[must_use]
    pub fn pv_status(&self) -> PvStatus {
        self.pv_status
    }

    /// Whether the last refinement round of the last solve was cut by the
    /// per-round work cap rather than by a global resource limit. Only
    /// meaningful when [`Search::pv_status`] is [`PvStatus::Unproven`].
    #[must_use]
    pub fn last_refine_round_cap_cut(&self) -> bool {
        self.last_round_cap_cut
    }

    /// The per-round child-eval cap for the next refinement round.
    fn refinement_round_cap(&self) -> u64 {
        if self.refine_cap_min == u64::MAX {
            return u64::MAX;
        }
        let scaled = (u128::from(self.first_outcome_evals) * u128::from(self.refine_cap_factor_num)
            / u128::from(self.refine_cap_factor_den)) as u64;
        scaled.max(self.refine_cap_min)
    }

    /// Whether the cumulative child-evaluation budget set by
    /// [`Search::set_child_eval_budget`] has been spent. Always `false` when no
    /// budget was set (`u64::MAX`).
    #[must_use]
    pub fn child_eval_budget_exceeded(&self) -> bool {
        self.child_evals >= self.child_eval_budget
    }

    pub fn set_max_ply(&mut self, max_ply: usize) {
        self.max_ply = max_ply.max(1);
    }

    /// Replace the deterministic restart schedule used by the work-chunk
    /// loop. See [`RestartSchedule`]; this is a pacing/diversification policy,
    /// not a budget.
    pub fn set_restart_schedule(&mut self, schedule: RestartSchedule) {
        self.restart_schedule = schedule;
    }

    /// The current restart schedule (seeded with [`RestartSchedule::SHIPPED`]).
    #[must_use]
    pub fn restart_schedule(&self) -> RestartSchedule {
        self.restart_schedule
    }

    /// Number of restart boundaries crossed in the current run. Zero for a run
    /// that finished in its first slice (for example
    /// [`RestartSchedule::NEVER`]).
    #[must_use]
    pub fn restarts(&self) -> u64 {
        self.restarts
    }

    pub fn set_epsilon(&mut self, epsilon: f64) {
        assert!(
            (0.0..=1.0).contains(&epsilon),
            "epsilon must be in [0.0, 1.0], got {epsilon}"
        );
        let (num, den) = fraction_from_f64(1.0 + epsilon);
        self.epsilon_num = num;
        self.epsilon_den = den;
    }

    /// Set the TT bucket-index salt (the noise-channel knob behind the
    /// salt-seeded statistical gate). `0` (the default) keeps the shipped
    /// indexing bit-for-bit; `s > 0` remaps which keys share a bucket without
    /// changing probe/store semantics (see
    /// [`TranspositionTable::set_salt`]).
    ///
    /// Must be called before the first solve of this `Search` (the table must
    /// never be probed/stored under two different salts).
    pub fn set_salt(&mut self, salt: u64) {
        self.tt.set_salt(salt);
    }

    pub fn set_stop_flag(&mut self, stop_flag: Option<Arc<AtomicBool>>) {
        self.stop_flag = stop_flag;
    }

    /// Wire the memory-limit abort flag. Not used by the search CLI (which no
    /// longer builds proof trees); the reconstruct walker sets it for the
    /// builder's budget — see [`ExitReason::MemoryLimit`].
    pub fn set_memory_limited(&mut self, memory_limited: Option<Arc<AtomicBool>>) {
        self.memory_limited = memory_limited;
    }

    pub fn set_proof_event_sender(&mut self, sender: Option<std::sync::mpsc::Sender<ProofEvent>>) {
        self.proof_event_sender = sender;
    }

    /// Read-only access to the transposition table, for snapshot/debug use.
    ///
    /// This exposes the raw table contents (all generations, native bucket
    /// order via [`TranspositionTable::entries`]); the search itself only ever
    /// probes current-generation entries. Intended for the TT snapshot writer
    /// and debugging tools, not for search logic.
    #[must_use]
    pub fn tt(&self) -> &TranspositionTable {
        &self.tt
    }

    /// Mutable transposition-table access for tooling that pre-populates the
    /// table from a snapshot (see [`crate::tt_snapshot`] and
    /// [`crate::reconstruct`]). Seeding uses the public
    /// [`TranspositionTable::store`]; the search must not depend on seeded
    /// entries being present (they are ordinary entries: evictable, and
    /// subject to the same probe semantics as live ones).
    pub fn tt_mut(&mut self) -> &mut TranspositionTable {
        &mut self.tt
    }

    /// Aggregate transposition-table statistics after a search.
    ///
    /// Tuple fields are: `(buckets, live_entries, solved_entries, unsolved_entries, generation)`.
    #[must_use]
    pub fn tt_stats(&self) -> (usize, usize, usize, usize, u32) {
        self.tt.stats()
    }

    /// Distribution of stored `best_child` values among live TT entries.
    ///
    /// Useful for debugging proof-tree/GHI path-code usage.
    #[must_use]
    pub fn tt_best_child_counts(&self) -> Vec<(u8, usize)> {
        self.tt.best_child_counts()
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

    /// The reason the search stopped (timeout, quit, memory limit, child-eval
    /// budget exhausted, or complete).
    #[must_use]
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
        } else if self.child_eval_budget_exceeded() {
            ExitReason::BudgetExhausted
        } else if Instant::now() >= self.deadline {
            ExitReason::Timeout
        } else {
            ExitReason::Complete
        }
    }

    /// Run a single bounded, work-chunked `dfpn` search for `max_depth` plies.
    ///
    /// `round_work_cap` bounds the cumulative child evaluations spent by this
    /// call (a per-refinement-round cap). `u64::MAX` means uncapped, which is
    /// what the non-refinement entry points use.
    fn bounded_search(
        &mut self,
        pos: &mut Position,
        max_depth: u32,
        round_work_cap: u64,
    ) -> (Outcome, Vec<Move>, RoundTermination) {
        let mut outcome = Outcome::Draw;
        // Default for a loop-condition exit: the only ways to leave the loop
        // without hitting a `break` below are `time_exceeded()`,
        // `child_eval_budget_exceeded()`, or (pathologically, via chunk
        // overflow) `chunk == 0` — all resource cuts.
        let mut termination = RoundTermination::ResourceCut;
        let mut chunk = self.restart_schedule.first_slice();
        let mut last_child_evals_before;
        let round_evals_start = self.child_evals;

        while !self.time_exceeded() && !self.child_eval_budget_exceeded() && chunk > 0 {
            self.reset_search_state();
            last_child_evals_before = self.child_evals;
            // Pacing only: the restart slice is capped by the remaining
            // per-round refinement cap. The global child-eval budget is NOT
            // shrunk into this slice — it is enforced per node inside `dfpn`
            // (`child_eval_budget_exceeded`), which keeps the unbudgeted path
            // bit-identical to the historical `call_max_work == chunk`.
            let remaining_budget = self.child_eval_budget.saturating_sub(self.child_evals);
            let round_remaining =
                round_work_cap.saturating_sub(self.child_evals - round_evals_start);
            let call_slice = chunk.min(round_remaining);
            if call_slice == 0 {
                // Round cap (or a pathological zero chunk) exhausted without
                // a decisive line; further chunks cannot change the outcome.
                // Attribute the cut to the strictly smaller remaining
                // constraint; on a tie the budget was spent too, so report
                // the resource cut.
                termination = if round_remaining < remaining_budget {
                    RoundTermination::CapCut
                } else {
                    RoundTermination::ResourceCut
                };
                break;
            }
            outcome = self.dfpn(pos, INF, INF, max_depth, call_slice, true);
            if outcome != Outcome::Draw {
                termination = RoundTermination::Decisive;
                break;
            }

            let work_done = self.child_evals - last_child_evals_before;
            if work_done < call_slice || call_slice == u64::MAX {
                // The search did not use its full slice (or the slice was
                // unbounded). That is natural exhaustion of the bounded tree
                // — unless a resource limit also fired during the final
                // slice, in which case the cut must never be claimed as
                // exhaustion.
                termination = if self.time_exceeded() || self.child_eval_budget_exceeded() {
                    RoundTermination::ResourceCut
                } else {
                    RoundTermination::Exhausted
                };
                break;
            }

            let next_chunk = self.restart_schedule.next(chunk, self.child_evals);
            if next_chunk > 0 {
                self.restarts += 1;
            }
            chunk = next_chunk;
            self.log_chunk(work_done, chunk, "bounded_search");
        }

        // The PV is an informational best-effort line from the TT's best_move
        // chain.  It is not guaranteed to be a valid proof; proof generation is
        // the responsibility of the proof-tree layer.
        let pv = self.extract_pv(pos);
        (outcome, pv, termination)
    }

    pub fn search_depth(
        &mut self,
        pos: &mut Position,
        max_depth: u32,
    ) -> (Outcome, Vec<Move>, u64) {
        self.begin_run();
        // Pre-phase (plan13): the certificate bound is the fixed `max_depth`.
        if let Some((outcome, pv)) = self.preflight_phase(pos, Some(max_depth)) {
            self.pv_status = if outcome == Outcome::Draw {
                PvStatus::None
            } else {
                PvStatus::PreflightProof
            };
            return (outcome, pv, self.nodes);
        }
        let (outcome, pv, _termination) = self.bounded_search(pos, max_depth, u64::MAX);
        (outcome, pv, self.nodes)
    }

    /// Run a bounded OR-node win search with a pre-populated repetition path.
    ///
    /// The repetition keys `prefix_keys` are the positions *before* `pos` is
    /// pushed onto the path stack. This is used by the `verify_ppv` example to
    /// check defender replies while preserving the history of the supplied PPV
    /// prefix.
    ///
    /// The pre-phase (plan13) deliberately does not run here: its root-only
    /// closure does not model the repetition context of the supplied prefix
    /// path, so the bounded search below runs instead.
    pub fn search_depth_with_prefix(
        &mut self,
        pos: &mut Position,
        max_depth: u32,
        prefix_keys: &[u64],
    ) -> (Outcome, u32, u64) {
        let saved_prefix = self.prefix_path.take();
        self.prefix_path = Some(prefix_keys.to_vec());
        self.begin_run();

        let (outcome, pv, _termination) = self.bounded_search(pos, max_depth, u64::MAX);
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
    ///
    /// Before the DF-PN loop, the detector-gated bounded pre-phase (plan13)
    /// may decide the root directly; a claim returns the outcome plus the
    /// certificate's principal-line PV (see [`PvStatus::PreflightProof`] and
    /// `Search::preflight_report`). On any deferral the ordinary search below
    /// runs bit-identically to a pre-phase-disabled run.
    ///
    /// The returned PV is proven shortest (in the [`PvStatus::ProvenShortest`]
    /// sense: no shorter decisive line exists within the solver's search
    /// semantics) if and only if [`Search::pv_status`] returns
    /// `ProvenShortest` after the call.
    pub fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        self.solve_with_progress(pos, |_, _| {})
    }

    /// Solve a position and call `on_progress` for every newly found decisive
    /// line. The final returned PV is the shortest line discovered before the
    /// timeout or the first outcome if `first_outcome_only` is set.
    ///
    /// The PV is proven shortest (in the [`PvStatus::ProvenShortest`] sense:
    /// no shorter decisive line exists within the solver's search semantics)
    /// if and only if [`Search::pv_status`] returns `ProvenShortest` after the
    /// call.
    pub fn solve_with_progress<F>(
        &mut self,
        pos: &mut Position,
        mut on_progress: F,
    ) -> (Outcome, Vec<Move>, u64)
    where
        F: FnMut(Outcome, &[Move]),
    {
        self.begin_run();

        // 0. Detector-gated bounded pre-phase (plan13): decides small-space
        //    regions (occupied <= 3 men, no pawns/castling) via a
        //    region-closure fixpoint with replay-verified certificates, and
        //    defers everywhere else, bit-identically. Claims consume no TT
        //    entries and return directly (no refinement rounds run).
        if let Some((outcome, pv)) = self.preflight_phase(pos, None) {
            self.pv_status = if outcome == Outcome::Draw {
                PvStatus::None
            } else {
                PvStatus::PreflightProof
            };
            return (outcome, pv, self.nodes);
        }

        // 1. First decisive outcome (work-chunked, unbounded depth).
        let (mut outcome, mut pv, _first_phase) = self.bounded_search(pos, u32::MAX, u64::MAX);
        if outcome != Outcome::Draw || !pv.is_empty() {
            on_progress(outcome, &pv);
        }
        self.first_outcome_evals = self.child_evals;

        // 2. Iteratively tighten the bound by two plies, unless the user asked
        //    for the first outcome only.
        let mut n = pv.len() as u32;
        // Iterative refinement is best-effort: it uses the informational PV
        // length from `extract_pv` to set a shorter bound, but it does not
        // validate that the new PV is a sound proof.
        //
        // Status bookkeeping: each round either improves the PV or terminates
        // the loop with an explicit status; the fallback below covers a
        // loop-condition exit (or the loop never running).
        let mut improved = false;
        // Status decided by the last round's termination (set at every break
        // site); `None` means the loop exited via its own condition.
        let mut round_status: Option<PvStatus> = None;
        while !self.first_outcome_only
            && outcome != Outcome::Draw
            && n > 2
            && !self.time_exceeded()
            && !self.child_eval_budget_exceeded()
        {
            let bound = n - 2;
            // Each round is work-capped so a round that can never succeed (a
            // bounded tree too large to exhaust) is abandoned deterministically
            // instead of running until the global deadline. A cap-cut round
            // returns Draw with unsolved TT stores, indistinguishable from a
            // work-chunk cutoff, so the loop's non-improving check stops
            // refinement with the best result so far.
            let round_cap = self.refinement_round_cap();
            let (new_outcome, new_pv, termination) = self.bounded_search(pos, bound, round_cap);
            self.refinement_rounds += 1;
            self.refinement_evals = self.child_evals - self.first_outcome_evals;
            if new_outcome == Outcome::Draw {
                round_status = Some(match termination {
                    RoundTermination::Exhausted => {
                        // The bounded tree at `bound = n - 2` was fully
                        // explored with no decisive line: within the solver's
                        // search semantics no win of length <= n - 2 exists,
                        // so the n-ply PV is proven shortest (length-wise).
                        PvStatus::ProvenShortest
                    }
                    RoundTermination::CapCut => {
                        self.last_round_cap_cut = true;
                        PvStatus::Unproven
                    }
                    _ => PvStatus::Unproven,
                });
                break;
            }
            if new_pv.len() as u32 >= n {
                // Defensive: a decisive-but-not-shorter round cannot improve
                // the PV; keep the best line so far, unproven.
                round_status = Some(PvStatus::Unproven);
                break;
            }
            outcome = new_outcome;
            pv = new_pv;
            n = pv.len() as u32;
            improved = true;
            on_progress(outcome, &pv);
        }

        let status = round_status.unwrap_or_else(|| {
            // The loop exited via its condition (or never ran).
            if outcome == Outcome::Draw {
                PvStatus::None
            } else if n <= 2 {
                // A 1-move win cannot be shortened.
                PvStatus::ProvenShortest
            } else if improved {
                // The loop condition cut refinement between rounds after at
                // least one improving round: a shorter win may exist.
                PvStatus::Unproven
            } else {
                // No improving round ever ran (first_outcome_only, or a
                // resource cut before the first round): the PV is the
                // first-outcome line.
                PvStatus::FirstOutcome
            }
        });
        self.pv_status = status;

        (outcome, pv, self.nodes)
    }

    fn begin_run(&mut self) {
        self.reset_search_state();
        // Per-search repetition cache (plan9): cleared once per run, before
        // every entry point (`search_depth`, `search_depth_with_prefix`,
        // `solve_with_progress` all route through here). `reset_search_state`
        // re-seeds `path_stack` from `prefix_path`, but the cache is keyed by
        // (position, ancestor set) pairs recorded during the run itself, so it
        // must be dropped regardless of prefix presence.
        self.repetition_cache.clear();
        self.nodes = 0;
        self.child_evals = 0;
        self.first_outcome_evals = 0;
        self.refinement_rounds = 0;
        self.refinement_evals = 0;
        self.restarts = 0;
        self.pv_status = PvStatus::None;
        self.last_round_cap_cut = false;
        self.start = Instant::now();
        self.deadline = self.start + self.timeout;
        // Clock sampler: read once immediately, then every CLOCK_SAMPLE_INTERVAL.
        self.last_clock.set(self.start);
        self.clock_sample_at.set(0);
        self.preflight_report = None;
    }

    /// Run the detector-gated bounded pre-phase (plan13) before the DF-PN
    /// loop and record its report.
    ///
    /// The hook is a no-op (report `disabled`/`detector`, zero evals) unless
    /// the pre-phase is enabled and the detector predicate holds. Child
    /// evaluations consumed by the pre-phase are added to the run's
    /// `child_evals` counter and count against `child_eval_budget` (R4); on
    /// budget exhaustion the pre-phase defers and the ordinary search
    /// immediately reports `ExitReason::BudgetExhausted`, so the documented
    /// budget contract holds verbatim. The pre-phase never consults the wall
    /// clock; the global timeout stays the search's own.
    fn preflight_phase(
        &mut self,
        pos: &mut Position,
        bound: Option<u32>,
    ) -> Option<(Outcome, Vec<Move>)> {
        let mut claimed: Option<(Outcome, Vec<Move>)> = None;
        let report = if !self.preflight_enabled {
            PreflightReport::deferred("disabled", 0, 0)
        } else if !preflight::detector_applies(pos.board()) {
            PreflightReport::deferred("detector", 0, 0)
        } else {
            let budget = self.child_eval_budget.saturating_sub(self.child_evals);
            let stop = self.stop_flag.as_deref();
            match preflight::run(pos, bound, budget, stop) {
                PreflightExit::Claimed(claim) => {
                    self.child_evals += claim.evals;
                    // Defense in depth on top of the replay verifier: the
                    // principal-line PV itself must replay legally from the
                    // root to a terminal with the claimed outcome at exactly
                    // the claimed rank (cheap for certificate-length lines).
                    let pv_ok = claim.outcome == Outcome::Draw
                        || self.validate_pv(&claim.pv, pos, claim.outcome, Some(claim.rank));
                    if pv_ok {
                        claimed = Some((claim.outcome, claim.pv));
                        PreflightReport::decided(
                            claim.outcome,
                            claim.evals,
                            claim.region,
                            claim.rank,
                        )
                    } else {
                        PreflightReport::deferred("cert-fail", claim.evals, claim.region)
                    }
                }
                PreflightExit::Deferred {
                    reason,
                    evals,
                    region,
                } => {
                    self.child_evals += evals;
                    PreflightReport::deferred(reason, evals, region)
                }
            }
        };
        self.preflight_report = Some(report);
        claimed
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

    fn reset_search_state(&mut self) {
        self.path_stack.clear();
        self.move_stack.clear();
        self.max_depth_reached = 0;
        if let Some(keys) = &self.prefix_path {
            self.path_stack = keys.clone();
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
