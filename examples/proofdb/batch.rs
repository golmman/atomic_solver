//! The batch driver (plan3 D2): the screen pass over a policy's job
//! sequence, the screen-budget cap, and the heavy-tier placement. Split
//! from the `proofdb_harvest` CLI for the 10 KB file-size convention; the
//! CLI drives this from `main`.
//!
//! Stop conditions are checked **between screen jobs only** (`max_jobs`,
//! `max_runtime`, `stop_file`; 0 = unlimited for jobs/runtime): a job
//! is never abandoned mid-search — budgets are small enough that losing one
//! interrupted job's work is cheaper than nondeterministic interruption.
//! The screen-budget cap (`max_total_evals`, 0 = unlimited; plan3 decision
//! 6's A/B fairness knob) is likewise a between-jobs check on the cumulative
//! screen child-evals; a fired cap is a normal stop (`budget`), not a
//! defect.
//!
//! Heavy tier (`heavy_sample`, `heavy_budget_evals`; `heavy_sample = 0`
//! disables): the first `heavy_sample` censored jobs among the screened
//! prefix, re-run at the heavy budget as the pre-registered censored-tail
//! sample. For `sharp-heavy-tail` the sample is C2-only and runs **where
//! the C2 screen ends** (the first non-C2 job, the frontier's end, or a
//! budget stop) — *before* C3/C1 start (plan3 §2). The heavy tier is not
//! part of the screen budget: it still runs after a budget stop;
//! user-facing stops skip it (plan2 finding 6 — a stopped screen does not
//! yield the pre-registered sample sequence).

use std::path::{Path, PathBuf};
use std::time::Instant;

use super::harvest::{Job, JobClass};
use super::session::Session;

/// One batch's counters (for the session summary).
#[derive(Debug, Default)]
pub struct BatchSummary {
    pub screen_jobs: usize,
    pub screen_decisive: usize,
    pub screen_evals: u64,
    pub heavy_jobs: usize,
    pub heavy_decisive: usize,
    /// `""` = exhausted; otherwise `budget`/`max-jobs`/`max-runtime`/`stop-file`.
    pub stop_reason: String,
}

/// Batch options (CLI-derived; see the module docs).
pub struct BatchOptions {
    /// `sharp-heavy-tail` sequences the heavy tier at the C2 screen's end
    /// and restricts its sample to C2 jobs.
    pub heavy_tail_is_c2: bool,
    pub max_total_evals: u64,
    pub heavy_budget_evals: u64,
    pub heavy_sample: usize,
    pub max_jobs: usize,
    pub max_runtime: u64,
    pub stop_file: PathBuf,
}

/// The user-facing stop condition (`None` = keep going).
fn user_stop(opts: &BatchOptions, completed: usize, t_start: &Instant) -> Option<String> {
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

/// Heavy tier: the first `heavy_sample` censored jobs among the screened
/// prefix `jobs[..upto]` (C2-only for `sharp-heavy-tail`), re-run at the
/// heavy budget. Emits one census line per heavy job.
fn heavy_tail(
    session: &mut Session,
    jobs: &[Job],
    upto: usize,
    opts: &BatchOptions,
    t_start: &Instant,
    completed: &mut usize,
    summary: &mut BatchSummary,
) {
    let mut n = 0usize;
    for job in jobs.iter().take(upto) {
        if n >= opts.heavy_sample {
            break;
        }
        if opts.heavy_tail_is_c2 && job.class != JobClass::SharpSibling {
            continue;
        }
        // A censored screen job: no shard exists for its path.
        if session.has_path(&job.path.join(" ")) {
            continue; // decided during the screen pass
        }
        if let Some(reason) = user_stop(opts, *completed, t_start) {
            eprintln!("harvest: heavy tier stopped early: {reason}");
            break;
        }
        let rec = session.run_job_with_budget(job, "heavy", opts.heavy_budget_evals);
        if rec.outcome != "censored" {
            summary.heavy_decisive += 1;
        }
        rec.emit();
        *completed += 1;
        n += 1;
    }
    summary.heavy_jobs += n;
}

/// Run the batch: the screen pass over `jobs`, with the heavy tier inserted
/// at the C2/C3 boundary for `sharp-heavy-tail`. Emits one census `job:`
/// line per screen job.
pub fn run_batch(session: &mut Session, jobs: &[Job], opts: &BatchOptions) -> BatchSummary {
    let t_start = Instant::now();
    // Where the C2 screen ends for `sharp-heavy-tail`: the index of the
    // first non-C2 job (the heavy tail runs there, before C3/C1 start).
    let c2_end = if opts.heavy_tail_is_c2 {
        jobs.iter().position(|j| j.class != JobClass::SharpSibling)
    } else {
        None
    };
    let mut summary = BatchSummary::default();
    let mut completed = 0usize;
    let mut heavy_done = false;
    let mut idx = 0usize;
    loop {
        if !heavy_done && c2_end == Some(idx) {
            heavy_done = true;
            heavy_tail(
                session,
                jobs,
                idx,
                opts,
                &t_start,
                &mut completed,
                &mut summary,
            );
        }
        if opts.max_total_evals > 0 && summary.screen_evals >= opts.max_total_evals {
            summary.stop_reason = "budget".to_string();
        } else if let Some(reason) = user_stop(opts, completed, &t_start) {
            summary.stop_reason = reason;
        }
        if !summary.stop_reason.is_empty() {
            // The heavy tier is not part of the screen budget: it still runs
            // after a budget stop; user-facing stops skip it (plan2 finding 6).
            if !heavy_done && c2_end.is_some() && summary.stop_reason == "budget" {
                heavy_tail(
                    session,
                    jobs,
                    idx,
                    opts,
                    &t_start,
                    &mut completed,
                    &mut summary,
                );
            }
            break;
        }
        let rec = session.run_job(&jobs[idx]);
        if rec.outcome != "censored" {
            summary.screen_decisive += 1;
        }
        summary.screen_evals += rec.child_evals;
        rec.emit();
        completed += 1;
        summary.screen_jobs += 1;
        idx += 1;
        if idx == jobs.len() && !heavy_done && summary.stop_reason.is_empty() {
            heavy_tail(
                session,
                jobs,
                idx,
                opts,
                &t_start,
                &mut completed,
                &mut summary,
            );
            break;
        }
    }
    summary
}
