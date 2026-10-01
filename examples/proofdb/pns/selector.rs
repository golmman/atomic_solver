//! The live PNS queue state machine (plan4 D1): [`Selection`] pop,
//! censor-time bump + frontier exposure, and decided-bookkeeping. The tree
//! and numbers live in the parent module; the batch driver (search calls,
//! stop conditions) in [`crate::proofdb::batch`].
//!
//! Queue discipline (plan4 §2, checkpoint amendment): `(effective number,
//! ply, path)` ascending in a `BTreeSet`, so pops are fully deterministic
//! and ties resolve before any run. A censored visit (a) bumps the node's
//! ledger record (`passes_failed += 1`, work added) and re-inserts it at
//! its raised effective number, and (b) records every not-yet-recorded
//! legal child as a fresh known-open record (`passes_failed = 0` →
//! effective number 1) and inserts it — freshly exposed children are
//! explored before the censored node's doubled revisit (the layer
//! discipline). Row children are never touched by exposure: open rows are
//! already jobs (or decided), proven rows are settled. The ledger is saved
//! atomically after every censoring (crash-safe between jobs).
//!
//! Tests run in `tests/proofdb.rs` via the parent module's include.

use super::{Child, Kind, Pns, PnsNode, Res};

/// One selection: the popped job plus its census metadata.
pub struct Selection {
    /// Space-joined UCI path (empty for the root).
    pub key: String,
    pub path: Vec<String>,
    pub ply: usize,
    /// `true` = ledger-known-open node (no DB row), `false` = open row.
    pub from_ledger: bool,
    /// 1-based visit rung (`passes_failed + 1`); the budget is the ladder's
    /// `2^(pass-1) × base`.
    pub pass: u32,
    /// The node's effective number at selection time (census field).
    pub number: u64,
    /// Ledger `work_done` before this visit (census field; 0 on first).
    pub work_before: u64,
    /// Child-eval budget for this visit (the geometric ladder).
    pub budget: u64,
}

impl Pns {
    /// Pop the best node: `(effective number, ply, path)` ascending — OR
    /// nodes by pn, AND nodes by dn. Returns `None` when the queue is empty
    /// (batch exhausted). Popped-but-unvisited nodes (a stop fired first)
    /// keep their ledger state; the next session re-selects them.
    pub fn pop(&mut self) -> Option<Selection> {
        loop {
            let (num, _ply, key) = self.queue.pop_first()?;
            let idx = *self.by_key.get(&key)?;
            if self.decided.contains(&key) {
                continue; // decided mid-session after a sibling's exposure
            }
            // Defensive: the effective number is recomputed at pop; a
            // mismatch re-queues at the current number (no re-propagation
            // exists, so this should never fire).
            let eff = self.effective(idx);
            if eff != num {
                self.queue.insert((eff, self.nodes[idx].ply, key));
                continue;
            }
            let (pass, work_before, from_ledger) = {
                let n = &self.nodes[idx];
                let (p, w) = self
                    .ledger
                    .get(&key)
                    .map_or((0, 0), |e| (e.passes_failed, e.work_done));
                (p + 1, w, n.kind == Kind::Ledger)
            };
            let k = u64::from(pass - 1).min(63);
            let budget = (1u64 << k).saturating_mul(self.base_budget);
            return Some(Selection {
                key,
                path: self.nodes[idx].path.clone(),
                ply: self.nodes[idx].ply,
                from_ledger,
                pass,
                number: num,
                work_before,
                budget,
            });
        }
    }

    /// Record a censored visit: bump the node's ledger record, re-insert it
    /// at its raised effective number, expose its unexpanded legal children
    /// as fresh known-open records entering the queue at number 1, and save
    /// the ledger atomically (the exposed frontier survives a crash).
    ///
    /// # Errors
    /// A queue/state defect, an expand failure, or a ledger save failure.
    pub fn on_censored(&mut self, key: &str, child_evals: u64) -> Res<()> {
        let idx = self
            .by_key
            .get(key)
            .copied()
            .ok_or_else(|| format!("censored node {key:?} not in tree"))?;
        let entry = self.ledger.bump(key, child_evals);
        self.nodes[idx].passes = entry.passes_failed;
        let eff = self.effective(idx);
        let ply = self.nodes[idx].ply;
        if !self.queue.insert((eff, ply, key.to_string())) {
            return Err(format!("queue already holds censored node {key:?}"));
        }
        self.expand(idx)?;
        let node_ply = self.nodes[idx].ply;
        let node_path = self.nodes[idx].path.clone();
        let fresh: Vec<String> = self.nodes[idx]
            .children
            .iter()
            .filter_map(|(uci, c)| match c {
                // Rows: open rows are already jobs (or decided mid-session),
                // proven rows are settled. Existing ledger records are
                // already queued, decided, or excluded — all correctly left
                // alone.
                Child::Row(_) | Child::LedgerNode(_) => None,
                Child::Unvisited => Some(uci.clone()),
            })
            .collect();
        for uci in fresh {
            let mut child_path = node_path.clone();
            child_path.push(uci.clone());
            let child_key = if node_path.is_empty() {
                uci
            } else {
                format!("{key} {uci}")
            };
            if !self.ledger.insert_fresh(&child_key) {
                continue; // already known-open (defensive; unvisited implies new)
            }
            let cidx = self.nodes.len();
            self.nodes.push(PnsNode {
                path: child_path,
                key: child_key.clone(),
                ply: node_ply + 1,
                kind: Kind::Ledger,
                active: true,
                passes: 0,
                children: Vec::new(),
                nums: None,
            });
            self.by_key.insert(child_key.clone(), cidx);
            if !self.queue.insert((1, node_ply + 1, child_key.clone())) {
                return Err(format!("queue already holds fresh child {child_key:?}"));
            }
            self.census.jobs_ledger += 1;
        }
        self.ledger.save(&self.ledger_path)
    }

    /// Record a decisive visit: the node left the frontier (its shard
    /// enters the manifest; the merger's next run makes it a proven row,
    /// and the next session's lineage gate drops its record). Its ledger
    /// record is kept until then; the in-session queue never re-selects it.
    pub fn on_decided(&mut self, key: &str) {
        self.decided.insert(key.to_string());
    }

    /// Was `key` decided during this session?
    #[must_use]
    pub fn is_decided(&self, key: &str) -> bool {
        self.decided.contains(key)
    }
}

#[cfg(test)]
mod tests;
