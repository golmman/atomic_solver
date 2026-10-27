//! `Search` run driver: bounded search, solve entry points, and run setup.
//!
//! Split out of `dfpn/mod.rs` by change reason (cleanup plan7) so that the
//! search loop and entry points evolve separately from configuration and
//! state. Module placement is codegen-neutral here (`lto = true`,
//! `codegen-units = 1`).

use super::*;

use crate::search::preflight::{self, PreflightExit};
use crate::zobrist::INF;

impl Search {
    /// The per-round child-eval cap for the next refinement round.
    fn refinement_round_cap(&self) -> u64 {
        if self.refine_cap_min == u64::MAX {
            return u64::MAX;
        }
        let scaled = (u128::from(self.first_outcome_evals) * u128::from(self.refine_cap_factor_num)
            / u128::from(self.refine_cap_factor_den)) as u64;
        scaled.max(self.refine_cap_min)
    }

    /// Run a single bounded, work-chunked `dfpn` search for `max_depth` plies.
    ///
    /// `round_work_cap` bounds the cumulative child evaluations spent by this
    /// call (a per-refinement-round cap). `u64::MAX` means uncapped, which is
    /// what the non-refinement entry points use.
    pub(super) fn bounded_search(
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

    pub(super) fn begin_run(&mut self) {
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
}
