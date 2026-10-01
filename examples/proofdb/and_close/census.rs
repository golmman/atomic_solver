//! The `and-close` session-start census (plan8 D1/§2): the job-set
//! composition line, the exclusion line (the plan4 decisions 9/10
//! exclusions), and the gradient-head echo. Split from [`super`] for the
//! 10 KB file-size convention; the sequence builder remains there.

use super::AndCloseOrder;

/// How many gradient rows the census echoes (the plan8 §1 head runs to the
/// root's 13; ten rows covers the pinned table with margin).
const GRADIENT_HEAD: usize = 10;

/// The session-start census (plan8 §2; gate H1 pins the numbers).
#[derive(Debug, Default)]
pub struct AndCloseCensus {
    /// Open DB rows, total (active + excluded).
    pub rows_open: usize,
    /// Active open rows whose missing replies form the job set.
    pub active_rows: usize,
    /// Excluded open rows per reason: (proven-ancestor, implied-win,
    /// implied-loss) — the decisions 9/10 exclusions.
    pub excluded: [usize; 3],
    /// Missing replies with no ledger record or a pass-0 record.
    pub replies_fresh: usize,
    /// Missing replies ledger-censored at least once (`passes_failed ≥ 1`).
    pub replies_censored: usize,
}

impl AndCloseCensus {
    /// The full job set (fresh + ledger-censored).
    #[must_use]
    pub fn replies(&self) -> usize {
        self.replies_fresh + self.replies_censored
    }

    /// The one-line session-start census (gate H1 pins the prefix).
    #[must_use]
    pub fn describe(&self, order: AndCloseOrder, base: u64) -> String {
        format!(
            "and-close: active rows {}, replies {} (fresh {}, ledger-censored {}); \
             order {}; base budget {}",
            self.active_rows,
            self.replies(),
            self.replies_fresh,
            self.replies_censored,
            order.as_str(),
            base,
        )
    }

    /// The exclusion-census line (decisions 9/10, reported for H1).
    #[must_use]
    pub fn describe_exclusions(&self) -> String {
        format!(
            "and-close-excluded: open rows {} (active {}); proven-ancestor {}, \
             implied-win {}, implied-loss {}",
            self.rows_open, self.active_rows, self.excluded[0], self.excluded[1], self.excluded[2],
        )
    }
}

/// The gradient-head census line (plan8 §2: "plus the gradient head
/// listing — pinned by §1").
#[must_use]
pub fn describe_gradient(gradient: &[(String, usize)]) -> String {
    let head: Vec<String> = gradient
        .iter()
        .take(GRADIENT_HEAD)
        .map(|(p, n)| {
            if p.is_empty() {
                format!("root:{n}")
            } else {
                format!("{p}:{n}")
            }
        })
        .collect();
    format!("and-close-gradient: {}", head.join(" "))
}
