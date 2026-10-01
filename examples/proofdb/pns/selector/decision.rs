//! The plan6 selection mechanism (§2, pre-registered): eligibility,
//! pacing, and rationing. The decision function per visit, with
//! `V = visits so far (expansion + rung)` and `R = reserve child-evals
//! spent`:
//!
//! 1. if `R >= reserve_share × session_cap` → goto 4;
//! 2. if `V % interleave_k == 0` and the eligible set ≠ ∅ → LADDER;
//! 3. if no expansion visit is possible (every ply layer's cap spent, or
//!    queue empty) and the eligible set ≠ ∅ and reserve remains → LADDER
//!    (fallback drain);
//! 4. EXPAND: pop min `(effective number, ply, path)` among nodes in a ply
//!    layer with layer visits < cap; if none exists and nothing is
//!    ladderable → stop ("exhausted").
//!
//! **Rung-eligible** = censored (`passes ≥ 1`) and no virgin child (with
//! `eligibility = "always"`: every censored node) — the eligibility
//! derivation and rotation in [`rung`]. While virgin children exist,
//! harvesting them is strictly better than re-searching the parent (§2
//! insight 1: expansion is the parent's proof work). Eligibility is
//! re-derived from the ledger + queue state on every visit (no new
//! persistent state, no number propagation — plan6 §6 non-goals).
//!
//! **Rung target:** eligible node by the configured rotation —
//! `"fewest-passes"` (rotation; ties `(ply, path)`) or `"number-ply-path"`
//! (the queue order) — at budget `rung_growth(pass + 1) × base`, never
//! above `max_rung_passes` rungs per node per session. A rung visit
//! spends only the reserve; an expansion visit counts against its ply
//! layer's cap; both kinds count into `V`. Caps, not quotas: unspent
//! reserve and unmet rung slots at session end stay unspent.
//!
//! Determinism (plan6 §4.7): every tie is resolved by path before any run;
//! the pacing counters count all visits; no wall clock enters the decision.
//! The degenerate config (`reserve_share = 0, layer_visit_cap = 0`)
//! reduces the function to the plan4 selector verbatim ([`Pns::pop`]) —
//! the §2 equivalence contract, pinned by the decision-table tests.
//!
//! Tests run in `decision/tests.rs` + `decision/tests_pacing.rs`.

use super::{Pns, Res, Selection, VisitKind};

impl Pns {
    /// The plan6 decision function: the next visit (or `None` = exhausted).
    /// See the module docs for the pre-registered table.
    ///
    /// # Errors
    /// A tree/replay defect while deriving eligibility (propagated from
    /// [`Pns::expand`]).
    pub fn next(&mut self) -> Res<Option<Selection>> {
        // 1. reserve exhausted → expansion only.
        if !self.pacing.reserve_remains() {
            return Ok(self.take_expand());
        }
        // 2. the interleave slot.
        if self
            .pacing
            .visits
            .is_multiple_of(self.pacing.cfg.interleave_k)
            && let Some(sel) = self.next_rung()?
        {
            self.note_visit(&sel);
            return Ok(Some(sel));
        }
        // 3. the fallback drain (no expansion visit possible).
        if !self.expansion_possible()
            && let Some(sel) = self.next_rung()?
        {
            self.note_visit(&sel);
            return Ok(Some(sel));
        }
        // 4. EXPAND.
        if let Some(sel) = self.take_expand() {
            return Ok(Some(sel));
        }
        // Nothing expandable: step 4's last chance — ladder if something is
        // ladderable, else stop ("exhausted").
        if let Some(sel) = self.next_rung()? {
            self.note_visit(&sel);
            return Ok(Some(sel));
        }
        Ok(None)
    }

    /// An expansion visit through the layer-cap filter; counts the visit
    /// into `V` and its ply layer's cap.
    fn take_expand(&mut self) -> Option<Selection> {
        let cap = self.pacing.cfg.layer_visit_cap;
        let sel = self.pop_capped(cap)?;
        self.note_visit(&sel);
        Some(sel)
    }

    /// Count a selected visit: `V` always; the ply layer's cap for
    /// expansion visits only (rungs spend the reserve instead).
    fn note_visit(&mut self, sel: &Selection) {
        self.pacing.visits += 1;
        if sel.kind == VisitKind::Expand {
            *self.pacing.layer_visits.entry(sel.ply).or_insert(0) += 1;
        }
    }

    /// Charge a completed rung visit's actual child-evals (the driver calls
    /// this after the job; a decisive rung spends less than its budget).
    pub fn charge_reserve(&mut self, evals: u64) {
        self.pacing.charge_reserve(evals);
    }

    /// Is an expansion visit possible at all (any queued, undecided node in
    /// a ply layer with visits left)? Same criteria as [`Pns::pop_capped`]
    /// minus the pop (and minus its defensive number re-check, which only
    /// fires on a state defect).
    fn expansion_possible(&self) -> bool {
        let cap = self.pacing.layer_cap();
        self.queue.iter().any(|(_num, ply, key)| {
            !self.decided.contains(key)
                && (cap == usize::MAX || *self.pacing.layer_visits.get(ply).unwrap_or(&0) < cap)
        })
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_pacing;
