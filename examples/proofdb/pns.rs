//! Breadth-first PNS selection (plan4 D1): structural proof/disproof
//! numbers over the DB tree *extended by the ledger's known-open records*,
//! the effective-number rules, the job set (sibling-skip + decided-row
//! exclusion, decisions 9/10), and the live priority queue: tree
//! construction and the lineage gate in [`build`], number derivation in
//! [`numbers`], the pop/censor state machine in [`selector`], and the
//! batch driver in [`super::batch`].
//!
//! **Numbers (pre-registered, plan4 §2).** For every node of the extended
//! tree, computed bottom-up with saturating arithmetic (`INF = u64::MAX`;
//! `INF` only ever arises from a proven root-loss child — finite sums that
//! overflow saturate to `INF - 1`, which is *not* decisive):
//!
//! - unvisited child (legal move of an undecided node with no DB row and
//!   no ledger record): pn = dn = 1;
//! - proven node: root-player-win → (pn 0, dn ∞), root-player-loss →
//!   (pn ∞, dn 0); outcomes are side-to-move, so an even-ply `win` or
//!   odd-ply `loss` is a root-player win;
//! - undecided node (open row or ledger record), parity formula over all
//!   legal children — even ply (OR): pn = min, dn = sum; odd ply (AND):
//!   pn = sum, dn = min;
//! - known-open ledger child: contributes the flat `1 + passes_failed`
//!   (§2 "known-open child" rule) to its parent's formula; its own numbers
//!   are computed recursively when the node itself is classified.
//!
//! **Job set.** Active undecided nodes (open rows + ledger records with no
//! proven ancestor) whose structural pn is 0 (implied win) or ∞ (implied
//! loss) are excluded (decision 10) and counted per reason; so is every
//! inactive node (behind a proven ancestor — sibling-skip, decision 9).
//! Every remaining job's relevant structural number is exactly 1 (§2
//! induction), so its effective number is `max(structural, 1 +
//! passes_failed)` = `1 + passes_failed` until it is censored.
//!
//! **Selection (live priority queue).** `(effective number, ply, path)`
//! ascending — OR-role nodes by pn, AND-role nodes by dn; freshly exposed
//! children join at their structural number 1, before any censored node's
//! doubled revisit (the layer discipline). Numbers are computed once at
//! session start; a node's number moves only through its own censor bumps
//! (no in-session re-propagation, §2 simplification); full PNS dynamics
//! happen at the session boundary, when the merged DB + ledger are
//! re-loaded and everything is recomputed.
//!
//! The AND-completeness assert over proven loss nodes is *not* duplicated
//! here — the CLI runs [`super::frontier::extract_frontier`] first (a
//! standing extraction invariant, plan3 finding 5); [`Pns::build`] may
//! assume it.
//!
//! Tests run in `tests/proofdb.rs` (which includes this module verbatim;
//! cargo does not run unit tests inside example targets).

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use atomic_solver::notation::move_to_uci;

use super::db::DbRow;
use super::harvest::replay_job_path;
use super::ledger::Ledger;

pub mod selector;

/// Proof-theoretic infinity (a proven root-loss child's pn / root-win
/// child's dn). Finite sums never produce it: they saturate one below.
pub const INF: u64 = u64::MAX;
/// Base budget per visit (plan4 decision 5); the k-th visit of a node runs
/// at `base << passes_failed` (the geometric ladder).
pub const BUDGET_PNS_BASE_EVALS: u64 = 4_000_000;

type Res<T> = Result<T, String>;

/// Why a frontier node is not a job (decision 9/10 census).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exclusion {
    /// Behind a proven ancestor (sibling-skip: alternative-move territory).
    ProvenAncestor,
    /// Structural pn = 0: the node is a proven root win via its children.
    ImpliedWin,
    /// Structural pn = ∞: every line below is refuted.
    ImpliedLoss,
}

impl Exclusion {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProvenAncestor => "proven-ancestor",
            Self::ImpliedWin => "implied-win",
            Self::ImpliedLoss => "implied-loss",
        }
    }
}

/// What kind of tree node this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Proven root-player win → (0, ∞).
    ProvenWin,
    /// Proven root-player loss → (∞, 0).
    ProvenLoss,
    /// Open DB row.
    OpenRow,
    /// Known-open ledger record (no DB row).
    Ledger,
}

/// One classified legal child of an expanded undecided node.
#[derive(Debug)]
pub enum Child {
    /// A DB row (proven or open).
    Row(usize),
    /// A ledger record node.
    LedgerNode(usize),
    /// No row, no record: contributes (1, 1); becomes a ledger record on
    /// censor-time exposure.
    Unvisited,
}

/// A node of the extended tree (DB rows ∪ ledger records).
pub struct PnsNode {
    pub path: Vec<String>,
    /// Space-joined path (map/queue key; empty for the root).
    pub key: String,
    pub ply: usize,
    pub kind: Kind,
    /// No proven ancestor (only active nodes are classified or queued).
    pub active: bool,
    /// Ledger `passes_failed` merged from the record (0 if none/proven).
    pub passes: u32,
    /// Legal children (filled lazily by [`Pns::expand`]).
    pub children: Vec<(String, Child)>,
    /// Structural (pn, dn), memoized by [`Pns::numbers`].
    pub nums: Option<(u64, u64)>,
}

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
    fn excl_add(&mut self, e: Exclusion, ledger: bool) {
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

/// The PNS selection state: the extended tree, the derived numbers, the
/// live queue, and the ledger it picks up from. The pop/censor state
/// machine is in [`selector`].
pub struct Pns {
    pub nodes: Vec<PnsNode>,
    by_key: HashMap<String, usize>,
    queue: BTreeSet<(u64, usize, String)>,
    decided: std::collections::HashSet<String>,
    pub ledger: Ledger,
    pub ledger_path: PathBuf,
    pub base_budget: u64,
    pub census: PnsCensus,
}

impl Pns {
    /// Expand an undecided node: replay its position, movegen, and classify
    /// every legal child (row / ledger record / unvisited).
    ///
    /// # Errors
    /// An illegal DB move on replay (the DB replays were validated upstream,
    /// so this is a defect) or a tree key collision.
    pub fn expand(&mut self, idx: usize) -> Res<()> {
        if !self.nodes[idx].children.is_empty() {
            return Ok(());
        }
        let path = self.nodes[idx].path.clone();
        let (pos, _prefix) =
            replay_job_path(&path).map_err(|e| format!("PNS expand replay: {e}"))?;
        let mut children = Vec::new();
        for mv in pos.legal_moves_vec() {
            let uci = move_to_uci(mv);
            let mut child_path = path.clone();
            child_path.push(uci.clone());
            let key = child_path.join(" ");
            let child = match self.by_key.get(&key) {
                Some(&i) => {
                    if self.nodes[i].path != child_path {
                        return Err(format!("tree key collision at {key:?}"));
                    }
                    if matches!(self.nodes[i].kind, Kind::Ledger) {
                        Child::LedgerNode(i)
                    } else {
                        Child::Row(i)
                    }
                }
                None => Child::Unvisited,
            };
            children.push((uci, child));
        }
        self.nodes[idx].children = children;
        Ok(())
    }
}

pub mod build;
pub mod numbers;

#[cfg(test)]
mod tests;
