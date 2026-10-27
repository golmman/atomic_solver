//! Deterministic restart schedule for `bounded_search`.
//!
//! When the current slice of child evaluations is spent, the recursion
//! unwinds to the root and re-enters `dfpn` with `INF`/`INF` thresholds: only
//! transposition-table entries survive, frame state is rebuilt. This is a
//! *diversification* device (it changes which corridor is explored next), not
//! a resumption mechanism. Pacing only — it never enforces a budget.
//!
//! The schedule is deliberately small: a base slice, a growth rule, and an
//! optional cutoff after which restarts stop. The shipped default reproduces
//! the historical constants exactly (`base 500_000`, geometric ×2, no
//! cutoff); see [`RestartSchedule::SHIPPED`].

use super::fraction_from_f64;

/// How the restart slice grows after each completed slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Growth {
    /// Multiply the previous slice by `num / den` (`u128`-widened).
    Geometric { num: u64, den: u64 },
    /// Add `step` to the previous slice (saturating).
    Linear { step: u64 },
}

/// Deterministic restart schedule for [`super::Search`]'s work-chunk loop.
///
/// `next` grows the slice; `first_slice` supplies the initial slice. A cutoff
/// of `Some(c)` disables further restarts once the run has spent at least `c`
/// child evaluations (`u64::MAX` slices thereafter). This makes
/// `{ base: u64::MAX, cutoff: None }` and `{ base: 500_000, cutoff:
/// Some(0) }` equivalent spellings of "never restart": the cutoff fires
/// before the first slice is spent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestartSchedule {
    base: u64,
    growth: Growth,
    cutoff: Option<u64>,
}

impl RestartSchedule {
    /// The shipped policy: `500_000` base, geometric ×2, no cutoff. An exact
    /// reproduction of the historical constants.
    pub const SHIPPED: Self = Self {
        base: 500_000,
        growth: Growth::Geometric { num: 2, den: 1 },
        cutoff: None,
    };

    /// Never restart: the first slice is unbounded and the cutoff keeps it
    /// that way.
    pub const NEVER: Self = Self {
        base: u64::MAX,
        growth: Growth::Geometric { num: 1, den: 1 },
        cutoff: Some(0),
    };

    /// A geometric schedule with the given base and growth factor
    /// (`factor >= 1.0`; `fraction_from_f64` keeps the multiplication exact).
    #[must_use]
    pub fn geometric(base: u64, factor: f64) -> Self {
        assert!(
            factor >= 1.0,
            "restart growth factor must be >= 1.0, got {factor}"
        );
        let (num, den) = fraction_from_f64(factor);
        Self {
            base,
            growth: Growth::Geometric { num, den },
            cutoff: None,
        }
    }

    /// A linear schedule with the given base and increment.
    #[must_use]
    pub fn linear(base: u64, step: u64) -> Self {
        Self {
            base,
            growth: Growth::Linear { step },
            cutoff: None,
        }
    }

    /// Return the schedule with restarts disabled once `child_evals >= cutoff`.
    #[must_use]
    pub const fn with_cutoff(mut self, cutoff: Option<u64>) -> Self {
        self.cutoff = cutoff;
        self
    }

    /// The initial slice: `u64::MAX` when the cutoff fires immediately,
    /// otherwise `base`.
    #[must_use]
    pub fn first_slice(&self) -> u64 {
        if self.cutoff.is_some_and(|c| c == 0) {
            u64::MAX
        } else {
            self.base
        }
    }

    /// The next slice after `chunk` was spent, given `spent` cumulative child
    /// evaluations. Returns `u64::MAX` once the cutoff has fired.
    #[must_use]
    pub fn next(&self, chunk: u64, spent: u64) -> u64 {
        if self.cutoff.is_some_and(|c| spent >= c) {
            u64::MAX
        } else {
            match self.growth {
                Growth::Linear { step } => chunk.saturating_add(step),
                Growth::Geometric { num, den } => {
                    ((u128::from(chunk) * u128::from(num)) / u128::from(den)) as u64
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_matches_legacy_formula() {
        // Today's two formulas, verbatim.
        let legacy_geometric = |mut chunk: u64| {
            let mut out = Vec::new();
            for _ in 0..12 {
                chunk = (u128::from(chunk) * 2) as u64;
                out.push(chunk);
            }
            out
        };
        let mut chunk = 500_000u64;
        let mut grown = Vec::new();
        for _ in 0..12 {
            chunk = RestartSchedule::SHIPPED.next(chunk, u64::MAX);
            grown.push(chunk);
        }
        assert_eq!(grown, legacy_geometric(500_000));

        // Linear growth: `saturating_add`.
        let schedule = RestartSchedule::linear(500_000, 500_000);
        let mut chunk = 500_000u64;
        let mut grown = Vec::new();
        for _ in 0..12 {
            chunk = schedule.next(chunk, u64::MAX);
            grown.push(chunk);
        }
        let expected: Vec<u64> = (2..=13).map(|i| 500_000u64 * i).collect();
        assert_eq!(grown, expected);
    }

    #[test]
    fn never_and_zero_cutoff_are_equivalent() {
        let zero_cutoff = RestartSchedule::SHIPPED.with_cutoff(Some(0));
        assert_eq!(
            zero_cutoff.first_slice(),
            RestartSchedule::NEVER.first_slice()
        );
        assert_eq!(
            zero_cutoff.next(500_000, 500_000),
            RestartSchedule::NEVER.next(500_000, 500_000)
        );
        assert_eq!(zero_cutoff.first_slice(), u64::MAX);
        assert_eq!(zero_cutoff.next(1, 0), u64::MAX);
    }

    #[test]
    fn never_restarts_keeps_the_first_slice_unbounded() {
        assert_eq!(RestartSchedule::NEVER.next(1, 10), u64::MAX);
        assert_eq!(RestartSchedule::SHIPPED.first_slice(), 500_000);
    }
}
