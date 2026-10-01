//! The ladder step of the plan6 selection mechanism (§2): the rotation
//! among eligible nodes and the eligibility derivation. The decision table
//! (when a rung may fire) is in [`super::decision`]; the pacing state in
//! `crate::proofdb::pns::pacing`. Tests via `decision/tests.rs` +
//! `decision/tests_pacing.rs` (through the parent modules' includes).
//!
//! **Rung-eligible** = censored (`passes ≥ 1`) and no virgin child (with
//! `eligibility = "always"`: every censored node). A child is virgin iff
//! it was never visited: an unexpanded legal child, a known-open record at
//! `passes_failed = 0`, or an unvisited open row — while virgin children
//! exist, harvesting them is strictly better than re-searching the parent
//! (§2 insight 1: expansion is the parent's proof work). Proven-row
//! children and mid-session decided children are non-virgin. Eligibility
//! is re-derived from the ledger + queue state on every visit (no new
//! persistent state, no number propagation — plan6 §6 non-goals).

use super::super::config::{Eligibility, Rotation};
use super::super::{Child, Kind, Pns, Res};
use super::{Selection, VisitKind};

/// The rotation scan's accumulator: `(sort key, node index, queue tuple)`.
type RungBest = Option<((u64, usize, String), usize, (u64, usize, String))>;

impl Pns {
    /// The ladder step: the rotation-selected eligible node, or `None` if
    /// nothing is ladderable (no eligible node, or all rung-capped).
    ///
    /// # Errors
    /// A tree/replay defect while deriving eligibility (propagated from
    /// [`Pns::expand`]).
    pub(super) fn next_rung(&mut self) -> Res<Option<Selection>> {
        if !self.pacing.reserve_remains() {
            return Ok(None);
        }
        // Snapshot the queue entries: the eligibility derivation below
        // needs `&mut self` (lazy child expansion). Selection criteria are
        // total, so the scan order does not matter.
        let entries: Vec<(u64, usize, String)> = self.queue.iter().cloned().collect();
        let mut best: RungBest = None;
        for entry in entries {
            let (num, ply, key) = (&entry.0, &entry.1, &entry.2);
            if self.decided.contains(key) {
                continue;
            }
            let Some(&idx) = self.by_key.get(key) else {
                continue;
            };
            if self.nodes[idx].passes == 0 {
                continue; // virgin (number-1 pool): expansion territory
            }
            let done = self.pacing.rung_visits.get(key).copied().unwrap_or(0);
            if done >= self.pacing.cfg.max_rung_passes {
                continue; // per-node per-session rung cap
            }
            if !self.rung_eligible(idx)? {
                continue;
            }
            let sort = match self.pacing.cfg.rotation {
                Rotation::FewestPasses => (u64::from(self.nodes[idx].passes), *ply, key.clone()),
                Rotation::NumberPlyPath => (*num, *ply, key.clone()),
            };
            if best.as_ref().is_none_or(|(b, _, _)| sort < *b) {
                best = Some((sort, idx, entry));
            }
        }
        let Some((_, idx, entry)) = best else {
            return Ok(None);
        };
        // The rung visit takes the node out of the queue (as any visit
        // does); the censor re-inserts it at its raised number.
        self.queue.remove(&entry);
        let key = self.nodes[idx].key.clone();
        let (pass, work_before) = {
            let (p, w) = self
                .ledger
                .get(&key)
                .map_or((0, 0), |e| (e.passes_failed, e.work_done));
            (p + 1, w)
        };
        let budget = self.pacing.cfg.rung_growth.budget(pass, self.base_budget);
        self.pacing
            .rung_visits
            .entry(key.clone())
            .and_modify(|v| *v += 1)
            .or_insert(1);
        Ok(Some(Selection {
            key,
            path: self.nodes[idx].path.clone(),
            ply: self.nodes[idx].ply,
            from_ledger: self.nodes[idx].kind == Kind::Ledger,
            pass,
            number: self.effective(idx),
            work_before,
            budget,
            kind: VisitKind::Rung,
        }))
    }

    /// Rung eligibility (plan6 §2): a censored node earns a revisit only
    /// when its whole exposed line is opened — no virgin child. May expand
    /// the node lazily (idempotent, no observable selection effect: an
    /// unclassified child stays number 1 either way).
    fn rung_eligible(&mut self, idx: usize) -> Res<bool> {
        if self.pacing.cfg.eligibility == Eligibility::Always {
            return Ok(true);
        }
        if self.nodes[idx].children.is_empty() {
            self.expand(idx)?;
        }
        let parent_key = self.nodes[idx].key.clone();
        Ok(self.nodes[idx]
            .children
            .iter()
            .all(|(uci, child)| self.child_visited(&parent_key, uci, child)))
    }

    /// Was this child ever visited (non-virgin)? Virgin = an unexpanded
    /// legal child, a known-open record at `passes_failed = 0`, or an
    /// unvisited open row. Proven rows and mid-session decided children are
    /// non-virgin.
    fn child_visited(&self, parent_key: &str, uci: &str, child: &Child) -> bool {
        let key = Pns::child_key(parent_key, uci);
        match child {
            // Stale classification (the child was exposed by an earlier
            // censor after this parent's numbers were derived): the ledger
            // is authoritative.
            Child::Unvisited => self.child_visited_by_key(&key),
            Child::LedgerNode(i) => self.decided.contains(&key) || self.nodes[*i].passes >= 1,
            Child::Row(i) => {
                !matches!(self.nodes[*i].kind, Kind::OpenRow)
                    || self.decided.contains(&key)
                    || self.ledger.get(&key).is_some_and(|e| e.passes_failed >= 1)
            }
        }
    }

    /// Ledger/queue-state authority on whether `key` was ever visited: a
    /// record at `passes_failed ≥ 1` (censored) or a mid-session decision.
    fn child_visited_by_key(&self, key: &str) -> bool {
        self.decided.contains(key) || self.ledger.get(key).is_some_and(|e| e.passes_failed >= 1)
    }
}
