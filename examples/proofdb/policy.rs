//! Coverage policies (plan3 D1): the pre-registered job sequences over
//! the full frontier ([`super::frontier`]) and the per-class screen budgets
//! (decision 5, fixed before any run — no mid-run tuning).
//!
//! - `breadth-pns` — the plan4 default: **no precomputed sequence** — the
//!   live priority queue over structural proof/disproof numbers in
//!   [`super::pns`], with the sidecar ledger in [`super::ledger`];
//! - `open-deepest` — C1 only, deepest-first (plan2's rule);
//! - `sharp-siblings` — C2 (sharpness order), then C3, then C1 deepest-first;
//!   C2 jobs screen at [`BUDGET_C2_EVALS`] (cheap territory — censor fast
//!   and move on), C1/C3 at [`BUDGET_C1_C3_EVALS`];
//! - `sharp-heavy-tail` — the same sequence, plus a censored-tail sample
//!   over the first C2 jobs at the heavy budget, run where the C2 screen
//!   ends (the batch driver in [`super::batch`] owns that placement).
//!
//! Class concatenation order is the policy; within each class the ranking
//! is fixed at extraction time. The plan3 A/B winner is recorded as the
//! default of the *legacy* group; `breadth-pns` (plan4) supersedes it as
//! the CLI default. No other behavior rides on this module.

use super::frontier::Frontier;
use super::harvest::Job;
use super::harvest::JobClass;

/// Screen budget for C2 (sharp-sibling) jobs: cheap territory — censor fast
/// and move on (pre-registered decision 5).
pub const BUDGET_C2_EVALS: u64 = 1_000_000;
/// Screen budget for C1/C3 jobs (plan2's screen budget).
pub const BUDGET_C1_C3_EVALS: u64 = 4_000_000;

/// The policy's per-class screen budget (decision 5; no mid-run tuning).
#[must_use]
pub fn budget_for(class: JobClass) -> u64 {
    match class {
        JobClass::SharpSibling => BUDGET_C2_EVALS,
        JobClass::Open | JobClass::OpenChild | JobClass::Ledger => BUDGET_C1_C3_EVALS,
    }
}

/// A coverage policy: a deterministic job sequence over the full frontier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// The plan4 default: the live breadth-first PNS priority queue over
    /// the open frontier, with the sidecar work ledger (no precomputed
    /// job sequence — see [`super::pns`]).
    BreadthPns,
    /// C1 only, deepest-first (plan2's rule).
    OpenDeepest,
    /// C2 (sharpness order), then C3, then C1 deepest-first.
    SharpSiblings,
    /// [`Policy::SharpSiblings`] plus a heavy censored-tail sample over the
    /// first C2 jobs, run where the C2 screen ends.
    SharpHeavyTail,
}

impl Policy {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BreadthPns => "breadth-pns",
            Self::OpenDeepest => "open-deepest",
            Self::SharpSiblings => "sharp-siblings",
            Self::SharpHeavyTail => "sharp-heavy-tail",
        }
    }

    /// Parse the `--policy` value.
    ///
    /// # Errors
    /// An unknown policy name.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "breadth-pns" => Ok(Self::BreadthPns),
            "open-deepest" => Ok(Self::OpenDeepest),
            "sharp-siblings" => Ok(Self::SharpSiblings),
            "sharp-heavy-tail" => Ok(Self::SharpHeavyTail),
            other => Err(format!(
                "unknown policy {other:?} (expected breadth-pns | open-deepest | \
                 sharp-siblings | sharp-heavy-tail)"
            )),
        }
    }
}

/// The deterministic job sequence of a policy: per-class budgets resolved
/// ([`budget_for`]), classes concatenated in policy order.
#[must_use]
pub fn jobs_for_policy(policy: Policy, f: &Frontier) -> Vec<Job> {
    let mut jobs = match policy {
        // Breadth-PNS selection is the live priority queue in `super::pns`;
        // it never consumes a precomputed sequence.
        Policy::BreadthPns => Vec::new(),
        Policy::OpenDeepest => f.c1.clone(),
        Policy::SharpSiblings | Policy::SharpHeavyTail => {
            f.c2.iter().chain(&f.c3).chain(&f.c1).cloned().collect()
        }
    };
    for j in &mut jobs {
        j.budget = budget_for(j.class);
    }
    jobs
}

#[cfg(test)]
mod tests;
