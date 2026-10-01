//! The N-way ledger union (plan7 §2): the standing-state merge mechanism
//! for proofdb work ledgers.
//!
//! Ledger records are *selection state only* (`work_done`, `passes_failed`
//! per known-open frontier path — no outcomes, no facts), so unioning
//! ledgers cannot create, destroy, or contradict a fact: the soundness
//! surface (shards + manifest + DB) is untouched. The rule per path key
//! (pre-registered, plan7 §2): take the record with the highest
//! `passes_failed`, ties broken by the higher `work_done` — equivalently
//! `passes_failed = max(inputs)` and `work_done = max(inputs at the
//! winning pass)`. Total, deterministic, order-independent, associative
//! (N-way = pairwise iteration), idempotent.
//!
//! Base compatibility is the operator's contract: the ledger format
//! carries no base stamp, so callers pin input digests (the CLI's
//! `--expect`, verified before the merge). A base-stamp field is a
//! report7 finding, not a format change here (plan7 §2.4).
//!
//! Use: batch 3's arm 2 (the standing ledger unioned with the committed
//! plan6 arm snapshots, recovering the censor knowledge the
//! standing-ledger advance dropped — report6 finding 4) and item 6's
//! per-worker ledger merge: the same N-way primitive.

use std::collections::BTreeMap;

use super::ledger::{Ledger, LedgerEntry};

/// Merge statistics (order-independent per-input attribution).
#[derive(Debug, Default)]
pub struct UnionStats {
    /// Number of input ledgers.
    pub inputs: usize,
    /// Records in the union.
    pub union_records: usize,
    /// Per input: records in the input.
    pub input_records: Vec<usize>,
    /// Per input: paths no *other* input knows (first-seen coverage).
    pub new_paths: Vec<usize>,
    /// Per input: records whose `passes_failed` strictly exceeds every
    /// other input's at the same path (the input is the sole knowledge
    /// source for the censor count).
    pub sole_pass_upgrades: Vec<usize>,
}

/// Does `a` win over `b` under the §2 rule (lexicographic max on
/// `(passes_failed, work_done)`)?
fn wins(a: LedgerEntry, b: LedgerEntry) -> bool {
    (a.passes_failed, a.work_done) > (b.passes_failed, b.work_done)
}

/// The N-way union (the §2 rule). Order-independent and idempotent: the
/// result depends only on the set of input records, not on their order or
/// repetition (an input equal to the union changes nothing).
#[must_use]
pub fn union(ledgers: &[Ledger]) -> (Ledger, UnionStats) {
    let mut merged: BTreeMap<String, LedgerEntry> = BTreeMap::new();
    for led in ledgers {
        for (k, e) in led.iter() {
            match merged.get(k) {
                None => {
                    merged.insert(k.clone(), *e);
                }
                Some(cur) if wins(*e, *cur) => {
                    merged.insert(k.clone(), *e);
                }
                Some(_) => {}
            }
        }
    }
    let mut st = UnionStats {
        inputs: ledgers.len(),
        union_records: merged.len(),
        input_records: ledgers.iter().map(Ledger::len).collect(),
        new_paths: vec![0; ledgers.len()],
        sole_pass_upgrades: vec![0; ledgers.len()],
    };
    for (i, li) in ledgers.iter().enumerate() {
        for (k, e) in li.iter() {
            let mut known_elsewhere = false;
            let mut max_other_pass = 0u32;
            for (j, lj) in ledgers.iter().enumerate() {
                if i != j
                    && let Some(o) = lj.get(k)
                {
                    known_elsewhere = true;
                    max_other_pass = max_other_pass.max(o.passes_failed);
                }
            }
            if !known_elsewhere {
                st.new_paths[i] += 1;
            } else if e.passes_failed > max_other_pass {
                st.sole_pass_upgrades[i] += 1;
            }
        }
    }
    (Ledger::from_entries(merged), st)
}
