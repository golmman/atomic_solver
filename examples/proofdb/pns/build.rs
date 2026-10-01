//! Tree construction (plan4 D1): the extended tree over DB rows + ledger
//! records, the lineage gate (decision 6), and the job-set classification
//! (sibling-skip + decided-row exclusion, decisions 9/10). Types live in
//! the parent module; tests via `tests/proofdb.rs`.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use super::super::harvest::replay_job_path;
use super::{DbRow, Exclusion, INF, Kind, Pns, PnsCensus, PnsNode, Res};
use crate::proofdb::db::DbContent;
use crate::proofdb::ledger::Ledger;
use crate::proofdb::pns::Pacing;
use atomic_solver::position::Outcome;

impl Pns {
    /// Build the extended tree over `db` + `ledger`, run the lineage gate,
    /// derive all numbers, classify the job set, and initialize the queue.
    /// `cfg` + `session_cap` seed the plan6 pacing state (§2); the plan4
    /// selector is the degenerate config (`reserve_share = 0`,
    /// `layer_visit_cap = 0`).
    ///
    /// # Errors
    /// Anything `replay_job_path` rejects for a ledger record (the lineage
    /// gate's illegal-path abort, decision 6), a ledger record with an
    /// unresolvable parent (DB shrank — a defect), a malformed DB tree, or
    /// a draw outcome in the DB.
    pub fn build(
        db: &DbContent,
        ledger: Ledger,
        ledger_path: PathBuf,
        base_budget: u64,
        cfg: crate::proofdb::pns::PnsConfig,
        session_cap: u64,
    ) -> Res<Self> {
        let mut pns = Self {
            nodes: Vec::new(),
            by_key: HashMap::new(),
            queue: BTreeSet::new(),
            decided: std::collections::HashSet::new(),
            ledger,
            ledger_path,
            base_budget,
            census: PnsCensus::default(),
            pacing: Pacing::new(cfg, session_cap),
        };
        pns.census.rows_total = db.rows.len();
        // DB rows first (creation order = DB id order), walking the row
        // tree from the root to carry the proven-ancestor flags down.
        let row_children: HashMap<String, Vec<usize>> = {
            let mut m: HashMap<String, Vec<usize>> = HashMap::new();
            for (i, r) in db.rows.iter().enumerate() {
                if r.ply == 0 {
                    continue;
                }
                let parent = r.path[..r.path.len() - 1].join(" ");
                m.entry(parent).or_default().push(i);
            }
            m
        };
        let root = db
            .rows
            .iter()
            .position(|r| r.ply == 0)
            .ok_or("DB has no root row")?;
        {
            let r = &db.rows[root];
            let kind = Self::proven_kind(r.ply, r.outcome)?;
            // The root has no ancestors, so it is always "active"; a proven
            // root is settled regardless and never frontier.
            pns.push_row(r, kind, true);
        }
        // The stack carries (db.rows index, active-below-here); the rows are
        // pushed into `nodes` in walk order, so the two index spaces are
        // kept apart.
        let mut stack: Vec<(usize, bool)> = vec![(root, true)];
        while let Some((ridx, active)) = stack.pop() {
            let key = db.rows[ridx].path.join(" ");
            for &c in row_children.get(&key).into_iter().flatten() {
                let r = &db.rows[c];
                let child_kind = Self::proven_kind(r.ply, r.outcome)?;
                let child_active = active && child_kind.is_none();
                pns.push_row(r, child_kind, child_active);
                stack.push((c, child_active));
            }
        }
        if pns.nodes.len() != db.rows.len() {
            return Err(format!(
                "DB row tree walk reached {}/{} rows (broken tree)",
                pns.nodes.len(),
                db.rows.len()
            ));
        }
        // Ledger nodes: lineage gate + parent resolution, in path-length
        // order so a ledger parent resolves before its children.
        let mut ledger_keys: Vec<String> = pns.ledger.iter().map(|(k, _)| k.clone()).collect();
        ledger_keys.sort_by_key(|k| k.split(' ').count());
        for key in ledger_keys {
            if let Some(&idx) = pns.by_key.get(&key) {
                // The path is a DB row now: drop the record iff decided
                // (gate, decision 6); on an open row keep the record (its
                // censor history feeds the effective number).
                if !matches!(pns.nodes[idx].kind, Kind::OpenRow) {
                    pns.ledger.remove(&key);
                    pns.census.ledger_dropped += 1;
                }
                continue;
            }
            let path: Vec<String> = key.split(' ').map(str::to_string).collect();
            replay_job_path(&path).map_err(|e| {
                format!("ledger lineage gate: record {key:?} does not replay legally: {e}")
            })?;
            let parent_key = path[..path.len() - 1].join(" ");
            let pidx = *pns
                .by_key
                .get(&parent_key)
                .ok_or_else(|| format!("ledger record {key:?}: parent not in tree"))?;
            let (pactive, pply, pkind) = {
                let p = &pns.nodes[pidx];
                (p.active, p.ply, p.kind)
            };
            let active = pactive && matches!(pkind, Kind::OpenRow | Kind::Ledger);
            let passes = pns.ledger.get(&key).map_or(0, |e| e.passes_failed);
            let idx = pns.nodes.len();
            pns.nodes.push(PnsNode {
                path,
                key: key.clone(),
                ply: pply + 1,
                kind: Kind::Ledger,
                active,
                passes,
                children: Vec::new(),
                nums: None,
            });
            pns.by_key.insert(key, idx);
        }
        pns.census.ledger_records = pns.nodes.len() - db.rows.len();
        pns.classify_jobs();
        Ok(pns)
    }
    fn push_row(&mut self, row: &DbRow, kind: Option<Kind>, active: bool) {
        if kind.is_none() {
            self.census.open_rows += 1;
        }
        let passes = if kind.is_none() {
            self.ledger
                .get(&row.path.join(" "))
                .map_or(0, |e| e.passes_failed)
        } else {
            0
        };
        let idx = self.nodes.len();
        self.nodes.push(PnsNode {
            key: row.path.join(" "),
            path: row.path.clone(),
            ply: row.ply,
            kind: kind.unwrap_or(Kind::OpenRow),
            active,
            passes,
            children: Vec::new(),
            nums: None,
        });
        self.by_key.insert(self.nodes[idx].key.clone(), idx);
    }

    /// Side-to-move outcome → root-player proven kind (`None` = undecided).
    fn proven_kind(ply: usize, outcome: Option<Outcome>) -> Res<Option<Kind>> {
        Ok(match outcome {
            None => None,
            Some(Outcome::Win) => Some(if ply.is_multiple_of(2) {
                Kind::ProvenWin
            } else {
                Kind::ProvenLoss
            }),
            Some(Outcome::Loss) => Some(if ply.is_multiple_of(2) {
                Kind::ProvenLoss
            } else {
                Kind::ProvenWin
            }),
            Some(Outcome::Draw) => return Err("draw outcome is not a DB fact".to_string()),
        })
    }
    /// Classify the active undecided frontier (open rows + ledger records):
    /// excluded per reason (decisions 9/10) or queued at the effective
    /// number. Inactive undecided nodes are proven-ancestor exclusions.
    fn classify_jobs(&mut self) {
        for idx in 0..self.nodes.len() {
            let (kind, active) = (self.nodes[idx].kind, self.nodes[idx].active);
            if !matches!(kind, Kind::OpenRow | Kind::Ledger) {
                continue; // proven rows are settled, not frontier
            }
            let ledger_node = kind == Kind::Ledger;
            if !active {
                self.census.excl_add(Exclusion::ProvenAncestor, ledger_node);
                continue;
            }
            let (pn, _) = self.numbers(idx);
            let excl = if pn == 0 {
                Some(Exclusion::ImpliedWin)
            } else if pn == INF {
                Some(Exclusion::ImpliedLoss)
            } else {
                None
            };
            if let Some(e) = excl {
                self.census.excl_add(e, ledger_node);
                continue;
            }
            let (key, ply) = (self.nodes[idx].key.clone(), self.nodes[idx].ply);
            let eff = self.effective(idx);
            if !self.queue.insert((eff, ply, key)) {
                panic!("PNS queue duplicate insert (state defect)");
            }
            if ledger_node {
                self.census.jobs_ledger += 1;
            } else {
                self.census.jobs_rows += 1;
            }
        }
    }
}
