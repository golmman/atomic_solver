//! The structural proof/disproof numbers (plan4 §2, pre-registered):
//! bottom-up parity formula over the extended tree with saturating
//! arithmetic, and the effective selection numbers. Tests via
//! `tests/proofdb.rs`.

use super::{Child, INF, Kind, Pns};

/// Saturating sum over PNS children: a single ∞ child dominates (∞);
/// finite sums that overflow saturate to `INF - 1`, which stays "huge but
/// undecided" (never matches the decisive-∞ exclusion check).
fn sum(nums: impl IntoIterator<Item = u64>) -> u64 {
    let mut acc: u64 = 0;
    for n in nums {
        if n == INF {
            return INF;
        }
        acc = match acc.checked_add(n) {
            Some(a) => a,
            None => return INF - 1,
        };
    }
    acc
}

impl Pns {
    /// Structural (pn, dn) of a node, memoized (post-order over the tree).
    ///
    /// # Panics
    /// If `expand` fails or an undecided node has no legal moves (a
    /// rule-terminal draw — undecided in this tree model, a gap the plan
    /// does not cover; abort rather than misclassify).
    pub fn numbers(&mut self, idx: usize) -> (u64, u64) {
        if let Some(n) = self.nodes[idx].nums {
            return n;
        }
        let n = match self.nodes[idx].kind {
            Kind::ProvenWin => (0, INF),
            Kind::ProvenLoss => (INF, 0),
            Kind::OpenRow | Kind::Ledger => {
                if self.nodes[idx].children.is_empty() {
                    self.expand(idx)
                        .expect("PNS expand during number derivation");
                }
                if self.nodes[idx].children.is_empty() {
                    panic!(
                        "PNS: open node {:?} has no legal moves (rule-terminal draw — \
                         undecided in this model; aborting rather than misclassifying)",
                        self.nodes[idx].key
                    );
                }
                let child_specs: Vec<Option<(bool, usize)>> = self.nodes[idx]
                    .children
                    .iter()
                    .map(|(_, c)| match c {
                        Child::Row(i) | Child::LedgerNode(i) => {
                            let ledger_child = matches!(self.nodes[*i].kind, Kind::Ledger);
                            Some((ledger_child, *i))
                        }
                        Child::Unvisited => None,
                    })
                    .collect();
                let child_nums: Vec<(u64, u64)> = child_specs
                    .iter()
                    .map(|spec| match *spec {
                        None => (1, 1), // unvisited child
                        Some((true, i)) => {
                            // Known-open child: flat 1 + passes (§2).
                            let p = u64::from(self.nodes[i].passes);
                            (1 + p, 1 + p)
                        }
                        Some((false, i)) => self.numbers(i),
                    })
                    .collect();
                if self.nodes[idx].ply.is_multiple_of(2) {
                    // OR: pn = min, dn = sum.
                    (
                        child_nums.iter().map(|n| n.0).min().unwrap_or(INF),
                        sum(child_nums.iter().map(|n| n.1)),
                    )
                } else {
                    // AND: pn = sum, dn = min.
                    (
                        sum(child_nums.iter().map(|n| n.0)),
                        child_nums.iter().map(|n| n.1).min().unwrap_or(INF),
                    )
                }
            }
        };
        self.nodes[idx].nums = Some(n);
        n
    }

    /// The effective selection number: `max(role structural, 1 + passes)`,
    /// where the role is pn for OR (even ply) and dn for AND (odd ply).
    #[must_use]
    pub fn effective(&mut self, idx: usize) -> u64 {
        let (pn, dn) = self.numbers(idx);
        let role = if self.nodes[idx].ply.is_multiple_of(2) {
            pn
        } else {
            dn
        };
        role.max(1 + u64::from(self.nodes[idx].passes))
    }
}
