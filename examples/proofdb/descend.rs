//! The `descend` policy (plan15 D1): the self-bootstrapping harvest. A
//! fresh layer cannot bootstrap itself by harvesting its DB frontier (the
//! cold-start finding: root-only layers run to `stop=exhausted decisive 0`
//! — the job set is exactly the root's 20 first-move replies, the deepest
//! positions in the tree). `descend` generates **off-tree bootstrap
//! candidates** instead: all legal startpos-rooted paths of length 1..=K,
//! bounded-solved under the standard budget ladder, decisive ones exported
//! as shards — the solve initiative's plan4 ply-1/ply-2 enumeration,
//! productized as a harvest policy.
//!
//! **Candidate set and skip rule** (plan15 D2). Candidates are all legal
//! paths of length 1..=K from the startpos in lexicographic UCI order.
//! A candidate is skipped (covered) iff the path itself is a stored DB row
//! (any outcome, including open) or a manifest shard path, **or** any
//! proper prefix of length ≥ 1 is one. The root row (empty path) is
//! deliberately **not** a skip trigger — the root being stored must not
//! suppress ply-1 candidates. Rationale: a stored self-or-ancestor means
//! the candidate is decided (proven ancestor), frontier territory (open
//! ancestor), or a C2/C3 member — all exploitation territory the existing
//! policies own. On a grown layer the kept set shrinks toward empty:
//! `descend` is a bootstrap policy that self-retires. Kept candidates are
//! asserted disjoint from all stored territory (the decision-3 convention;
//! a collision is a defect abort).
//!
//! **Budgets, ladder, ledger** (plan15 D3). Identical to `and-close`: the
//! per-candidate budget is `max(2^(k−1)·base, 2·work_done)` over the
//! candidate's own ledger record ([`super::and_close::ladder_budget`],
//! base = `--budget-evals`), with the bump-only censor hook
//! ([`super::and_close::driver::on_censored`]). Off-tree censor records
//! are first-class ledger records exactly like `and-close`'s.
//!
//! **Interaction** (plan15 D5). No `--pns-config` or heavy-option effect;
//! no and-close gradient/exclusion consultation. `descend` and `and-close`
//! compose by construction: a decisive ply-2 shard grafted at merge time
//! inserts open-ancestor rows (its ply-1 parent), which the next
//! `and-close` batch picks up as C3/frontier work. Batch 1 = descend
//! (bootstrap), batch 2+ = and-close (grow).
//!
//! Tests run in `tests/proofdb.rs` (which includes this module verbatim;
//! cargo does not run unit tests inside example targets).
//!
//! File-size justification: ~11.7 KB — the plan15 D1–D5 contract in the
//! module docs, the enumeration, the skip rule, the census, and the batch
//! driver share one candidate indexing scheme; splitting would separate
//! the D2/D3 semantics from their only consumers (same pattern as
//! `and_close.rs`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::Position;

use super::and_close::driver::on_censored;
use super::and_close::ladder_budget;
use super::db::DbContent;
use super::harvest::{Job, JobClass};
use super::ledger::Ledger;
use super::pns::BUDGET_PNS_BASE_EVALS;
use super::session::Session;

/// Default `--descend-plies` (plan15 D1).
pub const DEFAULT_PLIES: usize = 2;
/// Maximum accepted `--descend-plies` (plan15 D1: ply 3 enumerates ~10k
/// paths at the base budget each — documented user-controlled cost; ply 4
/// and beyond are rejected at the CLI).
pub const MAX_PLIES: usize = 3;

/// Is `n` an accepted `--descend-plies` value (range 1..=3)?
#[must_use]
pub fn plies_in_range(n: usize) -> bool {
    (1..=MAX_PLIES).contains(&n)
}

/// Enumerate all legal startpos-rooted paths of length 1..=plies, in
/// lexicographic UCI order (plan15 D2; deterministic). Depth-limited DFS
/// with undo over the shared `Position` — no FEN string handling.
#[must_use]
pub fn enumerate_paths(plies: usize) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut pos = Position::from_fen(Position::STARTPOS_FEN).expect("startpos FEN always parses");
    let mut path: Vec<String> = Vec::new();
    walk(&mut pos, &mut path, 1, plies, &mut out);
    out.sort_by_key(|p| p.join(" "));
    out
}

fn walk(
    pos: &mut Position,
    path: &mut Vec<String>,
    depth: usize,
    plies: usize,
    out: &mut Vec<Vec<String>>,
) {
    if depth > plies {
        return;
    }
    let mut ucis: Vec<String> = pos
        .legal_moves_vec()
        .iter()
        .map(|&m| move_to_uci(m))
        .collect();
    ucis.sort();
    for uci in ucis {
        let mv = uci_to_move(&uci, pos).expect("movegen/notation roundtrip");
        pos.do_move(mv);
        path.push(uci);
        out.push(path.clone());
        walk(pos, path, depth + 1, plies, out);
        path.pop();
        pos.undo_move(mv);
    }
}

/// Is `path` covered by stored territory (plan15 D2): the path itself or
/// any proper prefix of length ≥ 1 in `stored`? The empty root is never a
/// trigger, and no candidate is empty, so the root row cannot suppress.
fn covered(path: &[String], stored: &HashSet<String>) -> bool {
    path.iter()
        .enumerate()
        .any(|(i, _)| stored.contains(&path[..=i].join(" ")))
}

/// The session-start census (plan15 D4; the executor pins the exact line).
#[derive(Debug, Default)]
pub struct DescendCensus {
    /// All enumerated candidate paths (length 1..=plies).
    pub candidates: usize,
    /// Kept candidates (off-tree, not covered by stored territory).
    pub kept: usize,
    /// Skipped candidates (self or a proper prefix stored).
    pub covered: usize,
    /// Kept candidates per ply (index 0 = ply 1).
    pub kept_per_ply: [usize; MAX_PLIES],
}

impl DescendCensus {
    /// The one-line session-start census (plan15 D4 shape).
    #[must_use]
    pub fn describe(&self, plies: usize, base: u64) -> String {
        let per_ply: Vec<String> = (0..plies)
            .map(|i| format!("ply {}: {} kept", i + 1, self.kept_per_ply[i]))
            .collect();
        format!(
            "descend: candidates {} (kept {}, covered {}) — {}; base budget {}",
            self.candidates,
            self.kept,
            self.covered,
            per_ply.join(", "),
            base,
        )
    }
}

/// The extraction output: the ordered job sequence and the census.
#[derive(Debug)]
pub struct DescendBuild {
    pub jobs: Vec<Job>,
    pub census: DescendCensus,
}

/// Build the `descend` job sequence (plan15 D2/D3): enumerate, apply the
/// skip rule against stored territory (all DB row paths + manifest shard
/// paths), assert kept-candidate disjointness (decision-3 convention), and
/// resolve each job's ladder budget from its own ledger record. The ledger
/// is read, never written. Read-only over the DB content.
///
/// # Errors
/// A kept candidate that is nevertheless stored territory (a skip-rule
/// defect — abort, never patched).
pub fn build_sequence(
    db: &DbContent,
    manifest_paths: &HashSet<String>,
    ledger: &Ledger,
    plies: usize,
    base: u64,
) -> Result<DescendBuild, String> {
    let mut stored: HashSet<String> = manifest_paths.clone();
    for row in &db.rows {
        stored.insert(row.path.join(" "));
    }
    let mut census = DescendCensus::default();
    let mut jobs = Vec::new();
    for path in enumerate_paths(plies) {
        census.candidates += 1;
        if covered(&path, &stored) {
            census.covered += 1;
            continue;
        }
        let key = path.join(" ");
        if stored.contains(&key) {
            return Err(format!(
                "descend: kept candidate {key:?} is stored territory (skip-rule defect)"
            ));
        }
        let (passes, work) = ledger
            .get(&key)
            .map_or((0, 0), |e| (e.passes_failed, e.work_done));
        let ply = path.len();
        census.kept += 1;
        census.kept_per_ply[ply - 1] += 1;
        jobs.push(Job {
            budget: ladder_budget(passes, work, base),
            path,
            ply,
            class: JobClass::Descend,
            parent_bound: None,
        });
    }
    Ok(DescendBuild { jobs, census })
}

/// Batch options (stop conditions, checked between jobs only; 0 =
/// unlimited; a fired cap is a normal stop).
pub struct DescendOptions {
    pub max_total_evals: u64,
    pub max_jobs: usize,
    pub max_runtime: u64,
    pub stop_file: PathBuf,
}

/// One `descend` batch's counters (for the session summary).
#[derive(Debug, Default)]
pub struct DescendSummary {
    pub jobs: usize,
    pub decisive: usize,
    pub censored: usize,
    pub evals: u64,
    /// `""` = exhausted; otherwise `budget`/`max-jobs`/`max-runtime`/
    /// `stop-file`.
    pub stop_reason: String,
}

fn user_stop(opts: &DescendOptions, completed: usize, t_start: &Instant) -> Option<String> {
    if opts.max_jobs > 0 && completed >= opts.max_jobs {
        return Some("max-jobs".to_string());
    }
    if opts.max_runtime > 0 && t_start.elapsed().as_secs() >= opts.max_runtime {
        return Some("max-runtime".to_string());
    }
    if opts.stop_file.as_os_str().is_empty() {
        return None;
    }
    if Path::new(&opts.stop_file).exists() {
        return Some("stop-file".to_string());
    }
    None
}

/// Run the `descend` batch over the (already budgeted) job sequence: one
/// census `job:` line per visit (`"class":"D"`, `"tier":"descend"`,
/// ledger-integrated `pass`/`work_before`), the bump-only censor hook on a
/// censored candidate (plan15 D3), the standard shard pipeline for a
/// decisive one (plan15 D4).
///
/// # Errors
/// A ledger save failure on a censored candidate — aborts the batch; the
/// ledger's last saved state is consistent.
pub fn run_descend_batch(
    session: &mut Session,
    jobs: &[Job],
    ledger: &mut Ledger,
    ledger_path: &Path,
    opts: &DescendOptions,
) -> Result<DescendSummary, String> {
    let t_start = Instant::now();
    let mut summary = DescendSummary::default();
    for job in jobs {
        if opts.max_total_evals > 0 && summary.evals >= opts.max_total_evals {
            summary.stop_reason = "budget".to_string();
            break;
        }
        if let Some(reason) = user_stop(opts, summary.jobs, &t_start) {
            summary.stop_reason = reason;
            break;
        }
        let key = job.path.join(" ");
        let (passes, work_before) = ledger
            .get(&key)
            .map_or((0, 0), |e| (e.passes_failed, e.work_done));
        let mut rec = session.run_job_with_budget(job, "descend", job.budget);
        if rec.outcome == "censored" {
            summary.censored += 1;
            on_censored(ledger, ledger_path, &key, rec.child_evals)?;
        } else {
            summary.decisive += 1;
        }
        rec.pass = Some(passes + 1);
        rec.work_before = Some(work_before);
        rec.emit();
        summary.jobs += 1;
        summary.evals += rec.child_evals;
    }
    Ok(summary)
}

/// The CLI-side session assembly (the `proofdb_harvest` descend branch):
/// load the ledger, build the sequence, echo the census line, run the
/// batch. `budget_evals = 0` selects the standing base
/// ([`BUDGET_PNS_BASE_EVALS`]).
///
/// # Errors
/// Anything the ledger load, the sequence extraction, or a ledger save on
/// censor rejects (a defect — abort, never patched).
pub fn run_descend_session(
    session: &mut Session,
    db: &DbContent,
    manifest_paths: &HashSet<String>,
    ledger_path: &Path,
    plies: usize,
    budget_evals: u64,
    opts: &DescendOptions,
) -> Result<DescendSummary, String> {
    let mut ledger = Ledger::load(ledger_path)?;
    let base = if budget_evals > 0 {
        budget_evals
    } else {
        BUDGET_PNS_BASE_EVALS
    };
    let built = build_sequence(db, manifest_paths, &ledger, plies, base)?;
    eprintln!("{}", built.census.describe(plies, base));
    run_descend_batch(session, &built.jobs, &mut ledger, ledger_path, opts)
}
