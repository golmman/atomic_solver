//! `Search` configuration surface: setters, accessors, and exits.
//!
//! Split out of `dfpn/mod.rs` by change reason (cleanup plan7) so that
//! configuration additions do not touch the search driver or the state
//! definition.

use super::*;

impl Search {
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

    /// Enable or disable provari-style move-class starting proof numbers for
    /// OR-node children (see the `move_class` module). Disabled by default;
    /// the `--move-class-pn` CLI flag maps here. This changes only the initial
    /// estimate of a newly seen non-terminal child: terminal/solved children,
    /// learned or TT-reused bounds, proof events, and budget accounting are
    /// untouched.
    pub fn set_move_class_pn(&mut self, enabled: bool) {
        self.move_class_pn = enabled;
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

    /// Restart boundaries crossed in the current run (reset by `begin_run`).
    /// A boundary at which the schedule's cutoff fires still counts: the run
    /// re-enters `dfpn` with `INF`/`INF` and an unbounded final slice. Zero for
    /// a run that finished in its first slice (for example
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
}
