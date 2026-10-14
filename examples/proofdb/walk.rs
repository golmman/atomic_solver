//! The `frontier-walk` policy (plan16; the only harvest policy since the
//! plan18 policy consolidation): the self-advancing shallowest-first
//! ledger walk — a *live* selection policy with a driver-shaped selection
//! loop: the frontier mutates between iterations, so the selection loop
//! and the batch driver are one module here.
//!
//! **D1 — scope.** `--policy frontier-walk` (the only accepted value), no
//! new flags: the flat budget `B` is `--budget-evals` (0 → the standing
//! base [`BASE_EVALS`]); stop conditions and path flags keep their
//! tool-wide semantics. `--pns-config` and other legacy flags were removed
//! with the plan18 policy consolidation.
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
//! (sibling-skip, decision 9); (c) in-session complete/decided nodes.
//! Selection: the minimum of `(ply, space-joined path)` — shallowest first,
//! path-lexicographic tie-break (the codebase convention). **Root rule:**
//! when the candidate set is empty, the root becomes a virtual candidate
//! (ply 0, work 0) — *unless* the root is proven-stored (a proven DB row or
//! a manifest/shard path, [`Session::has_path`]), in which case the session
//! stops `exhausted`. A stored open root row is a legitimate first target
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
//! to a degenerate censor. No intra-session AND/OR propagation: a decisive
//! child does not decide its parent in-session — the merger's closure owns
//! that; the walk is a producer of facts, not an inference engine.
//!
//! **D4 — budget and ledger semantics.** Flat budget `B`, **no ladder**: a
//! node censored at `B` is never re-searched deeper by this policy —
//! expansion replaces deepening. Bump-only censor hook
//! ([`super::ledger::on_censored`]), no child exposure (the
//! decision-11 deviation verbatim — children are derived by movegen at
//! expansion time). The ledger is rewritten between jobs (crash-safe).
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
//! completes`]). Job records: the twelve standard fields with
//! `policy: "frontier-walk"`, `tier: "frontier-walk"`, `kind: "search" |
//! "expand"`, `pass = passes_failed + 1`, `work_before` = the record's
//! `work_done` before the visit (a fresh child's first visit is `pass 1`,
//! `work_before 0`). Summary line: the standard harvest shape.
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
//! File-size justification: ~14.7 KB — the normative D1–D7 contract lives
//! in this module doc and shares one indexing scheme with the selection
//! state ([`WalkFrontier`]); the batch driver is split out
//! ([`driver`], the driver-split file-size pattern).

use std::collections::{BTreeSet, HashMap, HashSet};

use atomic_solver::position::Outcome;

use super::db::DbContent;
use super::ledger::Ledger;
use super::session::Session;

pub mod driver;

/// The standing base per-visit child-eval budget (used when
/// `--budget-evals` is 0).
pub const BASE_EVALS: u64 = 4_000_000;

/// Why a ledger record is not a walk candidate (D2's lineage/ancestor
/// drops; a record can carry both — the shallowest proven prefix wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drop {
    /// The record's own path is a proven DB row (decision 6).
    Lineage,
    /// A proven DB row on a proper prefix (decision 9; the empty prefix
    /// included — a proven root row drops everything).
    Ancestor,
}

/// The session-start root state (the census's `root` field, D5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RootState {
    /// No root row in the DB (pathological — a DB always has the root).
    #[default]
    Fresh,
    /// The root is a stored open row (the fresh-layer bootstrap target).
    Open,
    /// The root is proven-stored (a proven DB row or a manifest/shard path).
    Proven,
}

impl RootState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Open => "open",
            Self::Proven => "proven",
        }
    }
}

/// The session-start census (D5; the exact wording is pinned by report16).
#[derive(Debug, Default)]
pub struct WalkCensus {
    /// Ledger records at session start.
    pub records: usize,
    /// Records whose own path is a proven DB row (lineage drop).
    pub lineage: usize,
    /// Records behind a proven DB row proper prefix (shallowest reason
    /// wins; lineage wins only if no shallower prefix is proven).
    pub ancestor: usize,
    /// Complete-marked candidates at emission time (0 at session start —
    /// the complete/decided sets are in-session state).
    pub complete: usize,
    pub root: RootState,
}

impl WalkCensus {
    /// The one-line session-start census (D5).
    #[must_use]
    pub fn describe(&self, base: u64) -> String {
        format!(
            "frontier-walk: records {} (lineage {}, ancestor {}, complete {}) \
             — root {}; base budget {}",
            self.records,
            self.lineage,
            self.ancestor,
            self.complete,
            self.root.as_str(),
            base,
        )
    }
}

/// The live selection state: the viable candidates in `(ply, path)` order
/// plus the in-session complete/decided exclusions (D2/D3). The frontier
/// mutates as the walk runs — censors admit fresh records, complete/decided
/// marks retire candidates.
pub struct WalkFrontier {
    /// Viable candidates, ordered by `(ply, space-joined path)`.
    order: BTreeSet<(usize, String)>,
    /// Proven DB-row paths (the D2 drop rules' input; static per session).
    proven: HashSet<String>,
    complete: HashSet<String>,
    decided: HashSet<String>,
    /// Complete-marked candidates this session (summary/report metric).
    pub completes: usize,
}

/// Plies of a space-joined path key (0 for the root).
fn ply_of(key: &str) -> usize {
    if key.is_empty() {
        0
    } else {
        key.split(' ').count()
    }
}

/// The path vector of a key (empty for the root).
fn path_of(key: &str) -> Vec<String> {
    if key.is_empty() {
        Vec::new()
    } else {
        key.split(' ').map(str::to_string).collect()
    }
}

impl WalkFrontier {
    /// Build over the ledger + DB rows: admit every record not dropped by
    /// the lineage/ancestor rules (D2 a/b) and count the census. Read-only
    /// over both inputs.
    #[must_use]
    pub fn new(ledger: &Ledger, db: &DbContent) -> (Self, WalkCensus) {
        let proven: HashSet<String> = db
            .rows
            .iter()
            .filter(|r| r.outcome.is_some())
            .map(|r| r.path.join(" "))
            .collect();
        let mut order = BTreeSet::new();
        let mut census = WalkCensus {
            records: ledger.len(),
            ..WalkCensus::default()
        };
        for (key, _) in ledger.iter() {
            match Self::drop_reason(key, &proven) {
                Some(Drop::Lineage) => census.lineage += 1,
                Some(Drop::Ancestor) => census.ancestor += 1,
                None => {
                    order.insert((ply_of(key), key.clone()));
                }
            }
        }
        (
            Self {
                order,
                proven,
                complete: HashSet::new(),
                decided: HashSet::new(),
                completes: 0,
            },
            census,
        )
    }

    /// The D2 drop rule for one key: `Some(Lineage)` iff the key's own path
    /// is a proven row, `Some(Ancestor)` iff a proper prefix (the empty
    /// prefix included) is; the shallowest proven prefix decides.
    #[must_use]
    fn drop_reason(key: &str, proven: &HashSet<String>) -> Option<Drop> {
        let parts: Vec<&str> = if key.is_empty() {
            Vec::new()
        } else {
            key.split(' ').collect()
        };
        for i in 0..=parts.len() {
            if proven.contains(&parts[..i].join(" ")) {
                return Some(if i == parts.len() {
                    Drop::Lineage
                } else {
                    Drop::Ancestor
                });
            }
        }
        None
    }

    /// The next candidate: the shallowest viable record with its live
    /// ledger work; `None` = no candidate (the caller applies the root
    /// rule).
    #[must_use]
    pub fn pick(&self, ledger: &Ledger) -> Option<(String, u64)> {
        let (_, key) = self.order.iter().next()?;
        let work = ledger.get(key).map_or(0, |e| e.work_done);
        Some((key.clone(), work))
    }

    /// Is the root blocked as a virtual candidate (complete-marked; a
    /// decided root is covered by the proven-stored check instead)?
    #[must_use]
    pub fn root_blocked(&self) -> bool {
        self.complete.contains("")
    }

    /// Mark a candidate complete (D3: pick-only, no unhandled child — or a
    /// zero-eval censor): removed from in-session candidacy, record
    /// retained.
    pub fn mark_complete(&mut self, key: &str) {
        if self.complete.insert(key.to_string()) {
            self.completes += 1;
        }
        self.order.remove(&(ply_of(key), key.to_string()));
    }

    /// Mark a decided node (a decisive visit's target): removed from
    /// in-session candidacy, ledger record left in place.
    pub fn mark_decided(&mut self, key: &str) {
        self.decided.insert(key.to_string());
        self.order.remove(&(ply_of(key), key.to_string()));
    }

    /// Admit a record as a candidate (after its creating censor): no-op
    /// unless it survives the D2 drop rules and is not already admitted.
    pub fn admit(&mut self, key: &str) {
        if WalkFrontier::drop_reason(key, &self.proven).is_none() {
            self.order.insert((ply_of(key), key.to_string()));
        }
    }
}

/// The handled rule (D3): a stored DB row (any outcome), a manifest ∪
/// in-session shard path, or a ledger record with `work_done ≥ B`.
#[must_use]
pub fn is_handled(
    key: &str,
    rows: &HashMap<String, Option<Outcome>>,
    ledger: &Ledger,
    base: u64,
    has_shard: impl Fn(&str) -> bool,
) -> bool {
    rows.contains_key(key) || has_shard(key) || ledger.get(key).is_some_and(|e| e.work_done >= base)
}

/// The session-start root state for the census (D5): proven-stored, else a
/// stored open row, else fresh (no root row — pathological).
#[must_use]
pub fn root_state(db: &DbContent, session: &Session) -> RootState {
    let root_row = db.rows.iter().find(|r| r.ply == 0);
    if root_row.is_some_and(|r| r.outcome.is_some()) || session.has_path("") {
        RootState::Proven
    } else if root_row.is_some() {
        RootState::Open
    } else {
        RootState::Fresh
    }
}

impl WalkFrontier {
    /// All DB-row paths keyed by outcome — the expansion handled-rule's
    /// row map, built once per batch (and reused by the census root state).
    #[must_use]
    pub fn rows_map(db: &DbContent) -> HashMap<String, Option<Outcome>> {
        db.rows
            .iter()
            .map(|r| (r.path.join(" "), r.outcome))
            .collect()
    }
}

#[cfg(test)]
mod tests;
