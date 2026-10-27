//! Cheap bounded DF-PN probe: a *screening gate* before the pre-phase.
//!
//! Motivation (performance report §9/§10, performance plan1): the
//! detector-gated pre-phase (`crate::search::preflight`) pays a fixed closure
//! cost (~2.3–5.9M child evals, ~0.28–0.80 s) on every detected root —
//! including the easy 3-man wins that the ordinary DF-PN search proves in a
//! few hundred child evaluations. This gate runs a *scratch* bounded search
//! first: if a decisive outcome falls out of a small child-eval cap, the
//! closure is skipped and the ordinary search produces the product result.
//!
//! The probe is a **gate, not a claim source**:
//!
//! - It never returns an outcome, PV, rank, or report of its own; the
//!   ordinary search after a `probe-solved` deferral is bit-identical to a
//!   `--no-preflight` run.
//! - A `Draw` return (resource cut *or* a proven draw) always defers to the
//!   pre-phase. Draws are never inferred from the probe.
//! - It uses a **scratch `Search` with its own scratch TT** (dropped when the
//!   gate returns), so the production transposition table is untouched (the
//!   pre-phase's no-TT soundness contract is not widened here).
//! - It emits no `ProofEvent`s (the scratch search has no sender), so the
//!   documented pre-phase R2 gap is unchanged.
//! - Terminal roots never reach it: their closure is one position, and the
//!   pre-phase claims them directly (`region == 1`).
//!
//! Determinism: for a fixed `(FEN, config, cap)` the probe is deterministic.
//! Its wall-clock timeout is a safety net only; the deterministic bound is
//! [`CHEAP_PROBE_EVAL_CAP`].

use super::*;

use crate::search::preflight;

/// Child-eval cap for the pre-preflight screening probe.
///
/// Calibrated (performance plan1, `docs/plans/performance/measurements/plan1/`)
/// against a 15-case 3-man corpus: the easy-win class solves in 1–293 evals
/// (KQ vs K), while every draw (KR/KN/KB vs K, cyclic rook, adjacency
/// immunity) and both hard wins (the `KQvK` ladder root/step 3) exhaust a
/// 2,000-eval cap at this TT size. The separating margin is therefore
/// 293 → 2,000 (6.8×); the probe is a gate, so a too-high cap only costs a
/// little extra work on the classes that continue to the pre-phase.
pub(super) const CHEAP_PROBE_EVAL_CAP: u64 = 2_000;
/// Scratch TT for the probe: 4 MB covers the cap comfortably and is dropped
/// immediately. It never aliases the production table.
pub(super) const CHEAP_PROBE_TT_MB: usize = 4;
/// Wall safety net; the child-eval cap is the deterministic bound.
pub(super) const CHEAP_PROBE_TIMEOUT_SECS: u64 = 1;

/// Result of the screening gate (see [`Search::cheap_probe_gate`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProbeGate {
    /// The gate ran: the pre-phase was enabled, the detector fired, the root
    /// was non-terminal, and a non-zero child-eval budget remained.
    pub ran: bool,
    /// The probe returned a decisive Win/Loss within the cap. A `Draw`
    /// return (resource cut or a proven draw) is *not* `decided`; it defers
    /// to the pre-phase.
    pub decided: bool,
    /// Child evals the probe consumed (0 when it did not run).
    pub evals: u64,
}

impl ProbeGate {
    const SKIPPED: Self = Self {
        ran: false,
        decided: false,
        evals: 0,
    };
}

impl Search {
    /// Run the cheap screening probe for `pos` and report its outcome.
    ///
    /// Callers add [`ProbeGate::evals`] to the run's `child_evals` and skip
    /// [`Search::preflight_phase`] when [`ProbeGate::decided`] is set (see
    /// `solve_with_progress`). The probe itself is a throwaway `Search`: same
    /// epsilon, move scorer, `max_ply`, restart schedule and TT salt as the
    /// production run, but a scratch [`CHEAP_PROBE_TT_MB`] table, the
    /// pre-phase disabled (no recursion), `first_outcome_only` set, and the
    /// child-eval budget clamped to `min(CHEAP_PROBE_EVAL_CAP, remaining)`.
    pub(super) fn cheap_probe_gate(&self, pos: &Position) -> ProbeGate {
        if !self.preflight_enabled
            || pos.outcome().is_some()
            || !preflight::detector_applies(pos.board())
        {
            return ProbeGate::SKIPPED;
        }
        let remaining = self
            .child_eval_budget
            .saturating_sub(self.child_evals)
            .min(CHEAP_PROBE_EVAL_CAP);
        if remaining == 0 || self.time_exceeded() {
            return ProbeGate::SKIPPED;
        }

        let mut probe = Search::new(CHEAP_PROBE_TT_MB);
        probe.preflight_enabled = false;
        probe.first_outcome_only = true;
        probe.max_ply = self.max_ply;
        probe.epsilon_num = self.epsilon_num;
        probe.epsilon_den = self.epsilon_den;
        probe.scorer = self.scorer.clone();
        probe.restart_schedule = self.restart_schedule;
        probe.set_salt(self.tt.salt());
        probe.set_timeout(CHEAP_PROBE_TIMEOUT_SECS);
        probe.set_child_eval_budget(remaining);
        probe.set_stop_flag(self.stop_flag.clone());

        let mut probe_pos = pos.clone();
        let (outcome, _pv, _nodes) = probe.solve(&mut probe_pos);
        let evals = probe.child_evaluations();
        drop(probe);

        ProbeGate {
            ran: true,
            decided: outcome != Outcome::Draw,
            evals,
        }
    }
}
