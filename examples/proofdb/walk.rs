//! The `frontier-walk` policy (plan16; the only harvest policy since the
//! plan18 policy consolidation): the self-advancing shallowest-first
//! ledger walk — a *live* selection policy with a driver-shaped selection
//! loop: the frontier mutates between iterations, so the selection loop
//! and the batch driver are one module here.
//!
//! **D1 — scope.** `--policy frontier-walk` (the only accepted value); the
//! one selection-layer flag is `--moves <uci path>` (plan19): the session
//! is **re-anchored** at the node reached by replaying the given moves from
//! the startpos — the scope node *is* the walk's root (not a filter): the
//! virtual-root bootstrap, the drop rules, the ordering, and the census all
//! evaluate scope-relative values, so no budget is spent above the scope
//! and no in-scope record is silently excluded behind settled territory.
//! Out-of-scope ledger records are never candidates, never visited, and
//! never written — but stay in the shared ledger untouched (censors only
//! bump visited nodes), counted in the census as `out_of_scope`. Proven DB
//! rows *above* the scope root are ignored (the operator chose the scope
//! deliberately); a proven-stored scope root ends the session `exhausted`
//! with an explanatory stderr note (decision-9 philosophy: no work beneath
//! proven nodes). With the flag absent (scope = `""`) every scope-relative
//! rule degenerates to the global one — the session is behaviorally
//! identical to the pre-plan19 walk. The flat budget `B` is `--budget-evals`
//! (0 → the standing base [`BASE_EVALS`]); stop conditions and path flags
//! keep their tool-wide semantics. `--pns-config` and other legacy flags
//! were removed with the plan18 policy consolidation.
//! Census class tags reuse the existing classes — no `JobClass` change (the
//! classes carry the true territory meaning; the `policy` field already
//! identifies the session): `L` for a search visit on a ledger record, `C3`
//! for an expansion visit on a fresh child, `C1` for the virtual-root
//! search visit (an open DB row). The walk never built a precomputed
//! sequence over the frontier classes (the plan18 consolidation removed
//! the machinery that did).
//!
//! **D2 — candidate set and selection.** Candidates = ledger records,
//! minus: (a) records whose own path is a proven DB row (lineage drop,
//! decision 6); (b) records with a proven DB row on a proper prefix — the
//! empty prefix included, so a proven root row drops everything
//! (sibling-skip, decision 9); (b') records at-or-under an in-scope
//! **or-closed** white-to-move parent (`Drop::OrClosed`, plan21 D3): a
//! proven `Loss` DB-row child already proves the parent — one winning move
//! per OR node — so the parent and everything under it leaves candidacy
//! (candidacy-only: censor rows are never deleted). The drop rules are
//! evaluated scope-relative; or-closedness above the scope root is ignored
//! (the operator chose the scope deliberately). Selection: the minimum of
//! `(ply, space-joined path)` — shallowest first,
//! path-lexicographic tie-break (the codebase convention; both computed on
//! scope-relative values — the scope root is ply 0, the same special
//! position the game start has globally; among in-scope records the
//! relative order equals the absolute order, since all share the scope
//! prefix). **Root rule:** when the candidate set is empty, the scope root
//! becomes a virtual candidate (ply 0, work 0) — *unless* the scope root is
//! proven-stored (a proven DB row at the scope key, or a manifest/shard
//! path there, [`super::session::Session::has_path`]), or **or-closed**
//! (plan21 D6: a white-to-move scope root with a proven `Loss` child row —
//! statically, per D2(b') — or or-closed in-session), in which case the
//! session stops `exhausted` (a scoped session prints an explanatory
//! stderr note in either case; ruling plan19-b: no work beneath proven
//! nodes). A stored open root row is a legitimate first target
//! (the fresh-layer bootstrap); its censor creates the root's ledger record
//! via the bump hook — the same "a censored open DB row gets its record on
//! first censor" semantics. Accepted consequence: on a
//! grown-but-not-merged layer an empty ledger always spends one root search
//! at `B` unless the root is proven — the spec's bootstrap behavior.
//!
//! **D3 — the two visit kinds; the complete rule.** A *search visit*
//! (picked candidate with `work_done < B`) runs the job at the flat `B`
//! (retained TT, so a revisit is never a byte-identical repeat); decisive →
//! the standard shard pipeline and in-session decided — the ledger record
//! is left in place (and-close precedent: the next session's lineage gate
//! drops it once the merger made the path a proven row); censored → bump
//! its own record and save atomically. An *expansion visit* (picked
//! candidate with `work_done ≥ B`) replays the path, enumerates the legal
//! children in lex UCI order, and searches the **first unhandled child** at
//! `B`. *Handled* = a stored DB row (any outcome) ∪ a manifest ∪ in-session
//! shard path ([`Session::has_path`]) ∪ a ledger record with
//! `work_done ≥ B`; an underworked ledger child is therefore *not* handled
//! — searching it is exactly D2's rule (b), so the two visit kinds never
//! contradict. A candidate with `work_done ≥ B` and *no* unhandled child is
//! marked **complete** — a pick-only iteration (no job, no evals), removed
//! from in-session candidacy, ledger record retained; each candidate
//! completes at most once, so the loop cannot stall. A censor that spends
//! **zero child-evals** (a non-budget Draw — e.g. an on-path repetition)
//! cannot advance the flat budget; the visit target is complete-marked with
//! its (bumped) record retained, the same no-ladder territory rule applied
//! to a degenerate censor. Intra-session propagation is limited to the
//! plan21 OR-node sibling skip: a decisive `Loss` at a child **or-closes**
//! its in-scope white-to-move parent (one proving child suffices at an OR
//! node; the child is at odd absolute ply exactly when its parent is
//! white-to-move) — complete-marking the parent, sweeping its subtree from
//! candidacy — and a decided node's subtree leaves candidacy (the
//! in-session form of the cross-session lineage gate: everything under an
//! in-session decided node is inside its stored proof subtree or an
//! off-proof branch unnecessary under the skip policy). Otherwise there is
//! no AND/OR propagation: a decisive child does not decide its parent
//! in-session — the merger's closure owns that; the walk is a producer of
//! facts, not an inference engine.
//!
//! **D4 — budget and ledger semantics.** Flat budget `B`, **no ladder**: a
//! node censored at `B` is never re-searched deeper by this policy —
//! expansion replaces deepening. Bump-only censor hook
//! ([`super::ledger::on_censored`]), no child exposure (the
//! decision-11 deviation verbatim — children are derived by movegen at
//! expansion time). Every censor commits one `censor`-table transaction
//! (crash-safe; plan20).
//! Measurable signature: the session's ledger growth equals its fresh-node
//! censor count. Accepted deviations from the earlier PNS conventions,
//! recorded as
//! limitations: no structural-number exclusions (decision 10 is not
//! applied), and the search visit's re-search can re-spend known work (the
//! retained TT softens this). The walk is deliberately number-free.
//!
//! **D5 — census, summary.** Session-start census line (the `complete`
//! counter is live and starts at 0 — the complete/decided sets are
//! in-session state; the session's final count is [`WalkSummary::
//! completes`]; `out_of_scope` counts the ledger records outside the
//! session scope and `scope` names the session's world; `or_closed` counts
//! the D2(b') drops — the shallowest decisive prefix wins the attribution).
//! Job records: the
//! twelve standard fields with `policy: "frontier-walk"`, `tier:
//! "frontier-walk"`, `kind: "search" | "expand"`, `pass = passes_failed +
//! 1`, `work_before` = the record's `work_done` before the visit (a fresh
//! child's first visit is `pass 1`, `work_before 0`). Summary line: the
//! standard harvest shape plus `or_closes N` (the dynamic or-closes of the
//! skip; complete-marks via or-close do not count toward `completes`).
//!
//! **D6 — termination, determinism.** Stop conditions are checked between
//! iterations only (`max_total_evals` on cumulative child-evals, `max_jobs`
//! counting searches, `max_runtime`, `stop_file`; 0 = unlimited); a fired
//! cap is a normal stop. Natural `exhausted` = no candidate can yield a
//! visit. The walk **does not self-retire**: on any undecided layer it
//! always has work, so session caps are the binder — an uncapped session on
//! an undecided layer does not terminate (the documented operating model).
//! Determinism: lex ordering everywhere, the sorted `BTreeMap` ledger,
//! deterministic replay.
//!
//! Tests run in `tests/proofdb.rs` (which includes this module verbatim;
//! cargo does not run unit tests inside example targets) plus the inline
//! selection unit tests below.
//!
//! File-size justification: ~20 KB (the plan21 OR-node sibling skip grew
//! the selection state; the census/`RootState` types were peeled off into
//! [`census`] and the scope-relative key algebra plus the drop/handled
//! classification into [`keys`] — the splits the pre-plan21 header deferred
//! until a feature actually crossed the ~20 KB line) — the normative D1–D6
//! contract lives in this module doc and shares one indexing scheme with
//! the selection state ([`WalkFrontier`]); the batch driver is split out
//! ([`driver`], the driver-split file-size pattern) and the unit tests into
//! `walk/tests.rs`.

use std::collections::{BTreeSet, HashSet};

use atomic_solver::position::Outcome;

use super::db::DbContent;
use super::ledger::Ledger;

pub mod census;
pub mod driver;
pub mod keys;

pub use census::{WalkCensus, root_state};
use keys::{
    Drop, drop_reason, in_scope, is_handled, join_scope, path_of, ply_of, rel_of, under_closed,
};

/// The standing base per-visit child-eval budget (used when
/// `--budget-evals` is 0).
pub const BASE_EVALS: u64 = 4_000_000;

/// The live selection state: the viable candidates in `(relative ply,
/// relative path)` order plus the in-session complete/decided exclusions
/// (D2/D3). The frontier mutates as the walk runs — censors admit fresh
/// records, complete/decided marks retire candidates. Keys are stored
/// scope-relative (a global session's scope is `""`, so its relative keys
/// are the absolute ones — the pre-plan19 representation).
pub struct WalkFrontier {
    /// The session scope (empty = global).
    scope: String,
    /// Viable candidates, ordered by `(relative ply, relative path)`.
    order: BTreeSet<(usize, String)>,
    /// Proven DB-row paths *relative to the scope* (the D2 drop rules'
    /// input; static per session; rows above the scope root excluded).
    proven: HashSet<String>,
    /// In-scope or-closed white-to-move parents *relative to the scope*
    /// (plan21 D3, static: a proven `Loss` child row in the DB); rows above
    /// the scope root excluded, like `proven`.
    or_closed: HashSet<String>,
    /// Or-closed in-session (plan21 D4, dynamic: a decisive `Loss` child).
    or_closed_dyn: HashSet<String>,
    complete: HashSet<String>,
    decided: HashSet<String>,
    /// Complete-marked candidates this session (summary/report metric).
    pub completes: usize,
}

impl WalkFrontier {
    /// Build over the ledger + DB rows: admit every in-scope record not
    /// dropped by the D2 rules (evaluated on the scope-relative path against
    /// proven rows and or-closed parents *relative to the scope*; rows above
    /// the scope root are ignored) and count the census. Read-only over both
    /// inputs.
    #[must_use]
    pub fn new(ledger: &Ledger, db: &DbContent, scope: &str) -> (Self, WalkCensus) {
        let proven_abs: HashSet<String> = db
            .rows
            .iter()
            .filter(|r| r.outcome.is_some())
            .map(|r| r.path.join(" "))
            .collect();
        let proven: HashSet<String> = if scope.is_empty() {
            proven_abs
        } else {
            proven_abs
                .iter()
                .filter(|p| in_scope(scope, p))
                .map(|p| rel_of(scope, p).to_string())
                .collect()
        };
        // The or-closed parents (plan21 D3): in-scope nodes at even absolute
        // ply (white to move) with a proven `Loss` child row — one winning
        // move already proves the OR node. The scope root's own
        // or-closedness comes from a loss child *inside* the scope; a loss
        // row at the scope root itself has its parent above the scope and is
        // ignored.
        let mut or_closed = HashSet::new();
        for r in &db.rows {
            if r.outcome != Some(Outcome::Loss) || r.path.is_empty() {
                continue;
            }
            let parent_abs = r.path[..r.path.len() - 1].join(" ");
            if !in_scope(scope, &parent_abs) || !ply_of(&parent_abs).is_multiple_of(2) {
                continue;
            }
            or_closed.insert(rel_of(scope, &parent_abs).to_string());
        }
        let mut order = BTreeSet::new();
        let mut census = WalkCensus {
            records: ledger.len(),
            scope: scope.to_string(),
            ..WalkCensus::default()
        };
        for (key, _) in ledger.iter() {
            if !in_scope(scope, key) {
                census.out_of_scope += 1;
                continue;
            }
            let rel = rel_of(scope, key);
            match drop_reason(rel, &proven, &or_closed) {
                Some(Drop::Lineage) => census.lineage += 1,
                Some(Drop::Ancestor) => census.ancestor += 1,
                Some(Drop::OrClosed) => census.or_closed += 1,
                None => {
                    order.insert((ply_of(rel), rel.to_string()));
                }
            }
        }
        (
            Self {
                scope: scope.to_string(),
                order,
                proven,
                or_closed,
                or_closed_dyn: HashSet::new(),
                complete: HashSet::new(),
                decided: HashSet::new(),
                completes: 0,
            },
            census,
        )
    }

    /// The next candidate: the shallowest viable record with its live
    /// ledger work (the returned key is absolute — the driver and the job
    /// records speak absolute paths); `None` = no candidate (the caller
    /// applies the root rule).
    #[must_use]
    pub fn pick(&self, ledger: &Ledger) -> Option<(String, u64)> {
        let (_, rel) = self.order.iter().next()?;
        let key = join_scope(&self.scope, rel);
        let work = ledger.get(&key).map_or(0, |e| e.work_done);
        Some((key, work))
    }

    /// Is the scope root blocked as a virtual candidate (complete-marked;
    /// a decided scope root is covered by the proven-stored check instead)?
    /// The scope root's relative key is always `""`.
    #[must_use]
    pub fn root_blocked(&self) -> bool {
        self.complete.contains("")
    }

    /// Is the scope root or-closed (plan21 D6)? Statically (an in-scope
    /// proven `Loss` child row — D2(b')) or dynamically (or-closed
    /// in-session via [`Self::or_close`]); a plain complete-mark does not
    /// count — the caller keeps that case silent.
    #[must_use]
    pub fn root_or_closed(&self) -> bool {
        self.or_closed.contains("") || self.or_closed_dyn.contains("")
    }

    /// Mark a candidate complete (D3: pick-only, no unhandled child — or a
    /// zero-eval censor): removed from in-session candidacy, record
    /// retained.
    pub fn mark_complete(&mut self, key: &str) {
        let rel = rel_of(&self.scope, key).to_string();
        if self.complete.insert(rel.clone()) {
            self.completes += 1;
        }
        self.order.remove(&(ply_of(&rel), rel));
    }

    /// Mark a decided node (a decisive visit's target): removed from
    /// in-session candidacy, ledger record left in place — and (plan21 D5)
    /// its whole subtree leaves candidacy: every node under an in-session
    /// decided node is either inside its stored proof subtree (proven
    /// territory, the next session's lineage gate drops it) or an off-proof
    /// branch unnecessary under the skip policy. This is what makes an
    /// or-close of the winner's parent sound without a winner-subtree
    /// exception.
    pub fn mark_decided(&mut self, key: &str) {
        let rel = rel_of(&self.scope, key).to_string();
        self.decided.insert(rel.clone());
        self.sweep_subtree(&rel);
    }

    /// Remove every candidate at-or-under `rel` from the order set (the
    /// D4/D5 sweeps; candidacy-only — ledger records and censor rows stay).
    fn sweep_subtree(&mut self, rel: &str) {
        let prefix = if rel.is_empty() {
            String::new()
        } else {
            format!("{rel} ")
        };
        self.order
            .retain(|(_, r)| r != rel && !r.starts_with(&prefix));
    }

    /// Or-close `key` (plan21 D4): its value is established by one proven
    /// winning child — complete-mark it (without counting toward
    /// `completes`; D8 — an or-close is a pick-only iteration, no job) and
    /// sweep its subtree from candidacy. Idempotent: an already complete
    /// node is neither re-swept nor re-counted. Returns whether a new
    /// or-close happened (the summary's `or_closes`).
    pub fn or_close(&mut self, key: &str) -> bool {
        let rel = rel_of(&self.scope, key).to_string();
        if !self.complete.insert(rel.clone()) {
            return false;
        }
        self.or_closed_dyn.insert(rel.clone());
        self.sweep_subtree(&rel);
        true
    }

    /// The plan21 D4 in-session trigger: after a decisive `Loss` visit at
    /// `key`, or-close `key`'s parent if it is an in-scope white-to-move
    /// node (the child at odd absolute ply exactly when its parent is
    /// white-to-move; the startpos is white, strict alternation — D2's
    /// absolute-color rule). A decisive `Win` child refutes the parent's
    /// move and must not trigger (the caller only calls this on a `Loss`).
    /// Returns whether a new or-close happened.
    pub fn or_close_parent(&mut self, key: &str) -> bool {
        if ply_of(key) % 2 != 1 {
            return false;
        }
        // The parent of a single-token key is the root (the empty path);
        // only the root itself has none.
        let parent = match key.rsplit_once(' ') {
            Some((p, _)) => p.to_string(),
            None if !key.is_empty() => String::new(),
            None => return false,
        };
        if !in_scope(&self.scope, &parent) {
            return false;
        }
        self.or_close(&parent)
    }

    /// Admit a record as a candidate (after its creating censor): a no-op
    /// for out-of-scope keys (a censor only ever visits in-scope nodes, so
    /// this is defensive) and for keys dropped by the D2 rules (evaluated
    /// scope-relative; the plan21 D3 or-closed check included) or already
    /// admitted. Defensively also a no-op under an in-session or-closed
    /// ancestor — unreachable today, since no visit happens under an
    /// or-closed parent (plan21 D3).
    pub fn admit(&mut self, key: &str) {
        if !in_scope(&self.scope, key) {
            return;
        }
        let rel = rel_of(&self.scope, key);
        if drop_reason(rel, &self.proven, &self.or_closed).is_some()
            || under_closed(rel, &self.or_closed_dyn)
        {
            return;
        }
        self.order.insert((ply_of(rel), rel.to_string()));
    }
}

#[cfg(test)]
mod tests;
