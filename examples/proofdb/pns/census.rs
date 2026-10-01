//! The per-reason census of the job-set extraction (plan4 decision 6, gate
//! H1), moved to its own module in plan6 to keep `pns.rs` within the file
//! size convention. The config echo (plan6 D1) is appended to the census
//! line by the CLI, so every arm's transcript documents its effective
//! selection config.

use super::Exclusion;

/// Per-reason census of the job-set extraction (gate H1).
#[derive(Debug, Default)]
pub struct PnsCensus {
    pub rows_total: usize,
    /// All open DB rows (active and inactive).
    pub open_rows: usize,
    /// Ledger records surviving the lineage gate (new ledger nodes built).
    pub ledger_records: usize,
    /// Records dropped because their path is now a proven DB row.
    pub ledger_dropped: usize,
    /// Excluded frontier nodes per reason, split (rows, ledger).
    pub excluded: [(usize, usize); 3],
    /// Jobs: active undecided nodes, split open rows / ledger records.
    pub jobs_rows: usize,
    pub jobs_ledger: usize,
}

impl PnsCensus {
    pub(crate) fn excl_add(&mut self, e: Exclusion, ledger: bool) {
        let i = match e {
            Exclusion::ProvenAncestor => 0,
            Exclusion::ImpliedWin => 1,
            Exclusion::ImpliedLoss => 2,
        };
        if ledger {
            self.excluded[i].1 += 1;
        } else {
            self.excluded[i].0 += 1;
        }
    }

    /// The per-reason excluded counts as `(rows, ledger)`.
    #[must_use]
    pub fn excluded_of(&self, e: Exclusion) -> (usize, usize) {
        match e {
            Exclusion::ProvenAncestor => self.excluded[0],
            Exclusion::ImpliedWin => self.excluded[1],
            Exclusion::ImpliedLoss => self.excluded[2],
        }
    }

    #[must_use]
    pub fn jobs(&self) -> usize {
        self.jobs_rows + self.jobs_ledger
    }

    /// The one-line session-start census (gate H1: per-reason exclusion
    /// counts; decision 6: gate drops).
    #[must_use]
    pub fn describe(&self, base_budget: u64) -> String {
        use Exclusion as E;
        let (pa_r, pa_l) = self.excluded_of(E::ProvenAncestor);
        let (iw_r, iw_l) = self.excluded_of(E::ImpliedWin);
        let (il_r, il_l) = self.excluded_of(E::ImpliedLoss);
        format!(
            "pns: rows {} (open {}), ledger records {} ({} new, {} dropped as decided); \
             exclusions proven-ancestor r{pa_r}/l{pa_l}, implied-win r{iw_r}/l{iw_l}, \
             implied-loss r{il_r}/l{il_l}; jobs {} (rows {}, ledger {}); base budget {base_budget}",
            self.rows_total,
            self.open_rows,
            self.ledger_records + self.ledger_dropped,
            self.ledger_records,
            self.ledger_dropped,
            self.jobs(),
            self.jobs_rows,
            self.jobs_ledger,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn census_describe_and_counts() {
        let mut c = PnsCensus {
            rows_total: 10,
            open_rows: 3,
            ledger_records: 4,
            ledger_dropped: 1,
            ..PnsCensus::default()
        };
        c.excl_add(Exclusion::ProvenAncestor, false);
        c.excl_add(Exclusion::ImpliedLoss, true);
        c.jobs_rows = 2;
        c.jobs_ledger = 3;
        assert_eq!(c.jobs(), 5);
        assert_eq!(c.excluded_of(Exclusion::ProvenAncestor), (1, 0));
        assert_eq!(c.excluded_of(Exclusion::ImpliedWin), (0, 0));
        assert_eq!(c.excluded_of(Exclusion::ImpliedLoss), (0, 1));
        let line = c.describe(4_000_000);
        assert!(line.starts_with("pns: rows 10 (open 3), ledger records 5 (4 new, 1 dropped"));
        assert!(line.contains("jobs 5 (rows 2, ledger 3)"));
        assert!(line.ends_with("base budget 4000000"));
    }
}
