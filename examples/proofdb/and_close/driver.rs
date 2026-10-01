//! The `and-close` batch driver (plan8 D1): the job loop over the
//! completion-gradient sequence with the bump-only censor hook. Split from
//! [`super::and_close`] for the 10 KB file-size convention; the CLI drives
//! this from `main`.
//!
//! Stop conditions are checked **between jobs only** (`max_jobs`,
//! `max_runtime`, `stop_file`; 0 = unlimited; the cumulative
//! `max_total_evals`, 0 = unlimited) — a job is never abandoned
//! mid-search, and a fired cap is a normal stop (`budget`).
//!
//! The censor hook (plan8 §2, superseding plan4 decision 11 **for this
//! policy only**): a censored reply bumps its own ledger record
//! (`passes_failed += 1`, work added) and the ledger is saved atomically
//! (crash-safe). It does **not** expose the reply's children — exposure's
//! only consumers (the pns queue, pick-up state) derive fresh nodes by
//! movegen anyway, so a pass-0 record is number-equivalent to an unvisited
//! child and exposure is bookkeeping with no consumer. Measurable
//! signature (gate H1): the session's ledger growth equals its censor
//! count.
//!
//! Decisive replies run the standard shard pipeline ([`super::session`],
//! class `OpenChild`); a reply's ledger record, if any, is left in place —
//! the next session's lineage gate drops it once the merger made the path
//! a proven row.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::proofdb::and_close::{AndCloseOrder, build_sequence, describe_gradient};
use crate::proofdb::db::load_db_rows;
use crate::proofdb::harvest::Job;
use crate::proofdb::ledger::Ledger;
use crate::proofdb::pns::{BUDGET_PNS_BASE_EVALS, Pns, PnsConfig};
use crate::proofdb::session::Session;

/// One `and-close` batch's counters (for the session summary).
#[derive(Debug, Default)]
pub struct AndCloseSummary {
    pub jobs: usize,
    pub decisive: usize,
    pub evals: u64,
    /// Censored replies = the expected ledger growth (the H1 signature).
    pub censored: usize,
    /// `""` = exhausted; otherwise `budget`/`max-jobs`/`max-runtime`/
    /// `stop-file`.
    pub stop_reason: String,
}

/// Batch options (CLI-derived; see the module docs).
pub struct AndCloseOptions {
    pub max_total_evals: u64,
    pub max_jobs: usize,
    pub max_runtime: u64,
    pub stop_file: PathBuf,
}

/// The user-facing stop condition (`None` = keep going).
fn user_stop(opts: &AndCloseOptions, completed: usize, t_start: &Instant) -> Option<String> {
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

/// The censor hook: bump the reply's own ledger record and save atomically.
/// No exposure (the decision-11 deviation, normative for this policy —
/// see the module docs).
///
/// # Errors
/// A ledger save failure (the bump is in memory only; the caller aborts).
pub fn on_censored(
    ledger: &mut Ledger,
    ledger_path: &Path,
    key: &str,
    child_evals: u64,
) -> Result<(), String> {
    ledger.bump(key, child_evals);
    ledger.save(ledger_path)
}

/// Run the `and-close` batch over the (already budgeted) job sequence./// Emits one census `job:` line per job; census records carry `pass` and
/// `work_before` (ledger-integrated); `number`/`kind` stay null.
///
/// # Errors
/// A ledger save failure on a censored reply — aborts the batch; the
/// ledger's last saved state is consistent.
pub fn run_and_close_batch(
    session: &mut Session,
    jobs: &[Job],
    ledger: &mut Ledger,
    ledger_path: &Path,
    opts: &AndCloseOptions,
) -> Result<AndCloseSummary, String> {
    let t_start = Instant::now();
    let mut summary = AndCloseSummary::default();
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
        let mut rec = session.run_job_with_budget(job, "and-close", job.budget);
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

/// The CLI-side session assembly (the `proofdb_harvest` and-close branch,
/// moved here for the 10 KB convention on the example target root): load
/// the ledger + DB, build the classified PNS tree (the plan4 decisions 9/10
/// exclusions verbatim; the pns selection/queue itself is not used), build
/// the sequence, echo the census lines, and run the batch. `budget_evals =
/// 0` selects the plan4 base ([`BUDGET_PNS_BASE_EVALS`]).
///
/// # Errors
/// Anything the ledger/DB load, the tree build, the sequence extraction,
/// or a ledger save on censor rejects (a defect — abort, never patched).
#[allow(clippy::too_many_arguments)]
pub fn run_and_close_session(
    session: &mut Session,
    db_path: &Path,
    manifest_sha256: &str,
    ledger_path: &Path,
    order: AndCloseOrder,
    budget_evals: u64,
    opts: &AndCloseOptions,
) -> Result<AndCloseSummary, String> {
    let ledger = Ledger::load(ledger_path)?;
    let db = load_db_rows(db_path, manifest_sha256)?;
    let base = if budget_evals > 0 {
        budget_evals
    } else {
        BUDGET_PNS_BASE_EVALS
    };
    let mut sel = Pns::build(
        &db,
        ledger,
        ledger_path.to_path_buf(),
        base,
        PnsConfig::default(),
        0,
    )?;
    let built = build_sequence(&mut sel, order, base)?;
    eprintln!("{}", built.census.describe(order, base));
    eprintln!("{}", describe_gradient(&built.gradient));
    eprintln!("{}", built.census.describe_exclusions());
    let mut ledger = std::mem::take(&mut sel.ledger);
    run_and_close_batch(session, &built.jobs, &mut ledger, ledger_path, opts)
}
