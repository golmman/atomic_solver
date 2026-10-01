//! The pacing + rationing state (plan6 §2): the config, the session cap,
//! and the per-session counters (`V` visits, `R` reserve child-evals,
//! per-ply-layer expansion visits, per-node rung visits). One instance per
//! session, owned by [`super::selector::Pns`](crate::proofdb::pns::Pns);
//! the decision function in [`super::selector::decision`] reads it.

use std::collections::HashMap;

use super::config::PnsConfig;

/// The pacing + rationing state (one instance per session; plan6 §2).
#[derive(Debug)]
pub struct Pacing {
    /// The effective config (echoed on the `pns:` line).
    pub cfg: PnsConfig,
    /// The session cap (`--max-total-evals`; 0 = unlimited).
    pub session_cap: u64,
    /// Visits so far, expansion + rung (the pacing counter `V`).
    pub visits: u64,
    /// Child-evals actually spent on rung visits so far (`R`).
    pub reserve_spent: u64,
    /// Expansion visits per ply layer so far.
    pub layer_visits: HashMap<usize, usize>,
    /// Rung visits per node key this session (the `max_rung_passes` cap).
    pub rung_visits: HashMap<String, u32>,
}

impl Pacing {
    #[must_use]
    pub fn new(cfg: PnsConfig, session_cap: u64) -> Self {
        Self {
            cfg,
            session_cap,
            visits: 0,
            reserve_spent: 0,
            layer_visits: HashMap::new(),
            rung_visits: HashMap::new(),
        }
    }

    /// The rung reserve in child-evals: `reserve_share × session_cap`;
    /// an unlimited session cap (0) makes the reserve unlimited (rungs
    /// stay bounded by eligibility, `max_rung_passes`, and the interleave).
    /// A zero `reserve_share` means no rungs regardless.
    #[must_use]
    pub fn reserve_budget(&self) -> u64 {
        if self.cfg.reserve_share <= 0.0 {
            0
        } else if self.session_cap == 0 {
            u64::MAX
        } else {
            ((self.session_cap as f64) * self.cfg.reserve_share) as u64
        }
    }

    /// Is a rung visit still affordable? (Caps, not quotas: the check is
    /// `R < reserve_budget`; the final rung may overshoot by its own
    /// budget's remainder — the session cap still bounds everything.)
    #[must_use]
    pub fn reserve_remains(&self) -> bool {
        self.reserve_spent < self.reserve_budget()
    }

    /// The layer visit cap as a ply-layer counter bound (`0` = unlimited →
    /// `usize::MAX`).
    #[must_use]
    pub fn layer_cap(&self) -> usize {
        if self.cfg.layer_visit_cap == 0 {
            usize::MAX
        } else {
            usize::try_from(self.cfg.layer_visit_cap).unwrap_or(usize::MAX)
        }
    }

    /// Charge a completed rung visit's actual child-evals to the reserve.
    pub fn charge_reserve(&mut self, evals: u64) {
        self.reserve_spent = self.reserve_spent.saturating_add(evals);
    }
}
