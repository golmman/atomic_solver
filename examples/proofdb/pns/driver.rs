//! The breadth-first PNS batch driver (plan4 D2, plan6 D1): the visit loop
//! over the selector's decision function, the stop conditions, and the
//! session summary. Moved from `batch.rs` (plan6; the legacy screen-pass
//! driver stays there). Split for the 10 KB file-size convention; the CLI
//! drives this from `main`.
//!
//! Stop conditions are checked **between visits only** (`max_jobs`,
//! `max_runtime`, `stop_file`; 0 = unlimited for jobs/runtime): a job is
//! never abandoned mid-search. The session cap (`max_total_evals`, 0 =
//! unlimited) is likewise a between-visits check on the cumulative
//! child-evals; a fired cap is a normal stop (`budget`). Queue exhaustion
//! (the decision function finds neither an expansion visit nor a ladder
//! visit) is the normal `""` (exhausted) stop.
//!
//! The plan6 visit kinds (census field `kind`): an **expand** visit counts
//! against its ply layer's cap; a **rung** visit is charged to the reserve
//! *after* the job, at its actual child-evals — a decisive rung spends
//! less than its budget. Both kinds count into the pacing counter `V`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use super::super::harvest::{Job, JobClass};
use super::super::session::Session;
use super::Pns;
use super::selector::VisitKind;

/// One PNS batch's counters (for the session summary).
#[derive(Debug, Default)]
pub struct PnsSummary {
    pub jobs: usize,
    /// Rung visits among `jobs` (the plan6 mechanism's ladder spend).
    pub rungs: usize,
    pub decisive: usize,
    pub evals: u64,
    /// `""` = queue exhausted; otherwise `budget`/`max-jobs`/`max-runtime`/
    /// `stop-file`.
    pub stop_reason: String,
}

/// PNS batch options (CLI-derived). The session cap (`max_total_evals`) and
/// the stop conditions are **between-visits checks only**: a job is never
/// abandoned mid-search, and a popped-but-unvisited node keeps its ledger
/// state (the next session re-selects it).
pub struct PnsOptions {
    /// Cumulative child-eval cap over the batch (0 = unlimited; the
    /// pre-registered plan4 session cap is 300M).
    pub max_total_evals: u64,
    pub max_jobs: usize,
    pub max_runtime: u64,
    pub stop_file: PathBuf,
}

/// Run the breadth-PNS batch: per visit, the selector's decision function
/// ([`Pns::next`]) picks the target and kind (expand | rung), the job runs
/// at its budget, and the outcome feeds back (censor → bump + frontier
/// exposure; decisive → the session's shard pipeline). Emits one census
/// `job:` line per job (with the `kind`/`pass`/`number`/`work_before`
/// quadruple, plan4 D2 + plan6 D1).
///
/// # Errors
/// A selector/ledger defect (queue inconsistency, ledger save failure) —
/// aborts the batch; the ledger's last saved state is consistent.
pub fn run_pns_batch(
    session: &mut Session,
    sel: &mut Pns,
    opts: &PnsOptions,
) -> Result<PnsSummary, String> {
    let t_start = Instant::now();
    let mut summary = PnsSummary::default();
    loop {
        let mut stop = String::new();
        if opts.max_total_evals > 0 && summary.evals >= opts.max_total_evals {
            stop = "budget".to_string();
        } else if opts.max_jobs > 0 && summary.jobs >= opts.max_jobs {
            stop = "max-jobs".to_string();
        } else if opts.max_runtime > 0 && t_start.elapsed().as_secs() >= opts.max_runtime {
            stop = "max-runtime".to_string();
        } else if !opts.stop_file.as_os_str().is_empty() && Path::new(&opts.stop_file).exists() {
            stop = "stop-file".to_string();
        }
        if !stop.is_empty() {
            summary.stop_reason = stop;
            break;
        }
        let Some(item) = sel.next()? else {
            break; // queue exhausted (no expansion, no ladder)
        };
        let job = Job {
            path: item.path.clone(),
            ply: item.ply,
            class: if item.from_ledger {
                JobClass::Ledger
            } else {
                JobClass::Open
            },
            parent_bound: None,
            budget: 0,
        };
        let rec = session.run_pns_job(
            &job,
            item.budget,
            item.pass,
            item.number,
            item.work_before,
            item.kind.as_str(),
        );
        let censored = rec.outcome == "censored";
        summary.evals += rec.child_evals;
        summary.jobs += 1;
        if item.kind == VisitKind::Rung {
            summary.rungs += 1;
            sel.charge_reserve(rec.child_evals);
        }
        if !censored {
            summary.decisive += 1;
            sel.on_decided(&item.key);
        } else {
            sel.on_censored(&item.key, rec.child_evals)?;
        }
        rec.emit();
    }
    Ok(summary)
}
