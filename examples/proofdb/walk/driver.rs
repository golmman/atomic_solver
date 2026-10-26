//! The `frontier-walk` batch driver (plan16 D3–D6): the live selection
//! loop over the [`WalkFrontier`](super::WalkFrontier) with the two visit
//! kinds, the complete rule, and the root rule. The CLI drives this from
//! `main`. Since plan20 the censor hook commits to the DB's `censor`
//! table (one transaction per censor) instead of saving a JSON sidecar.
//!
//! File-size justification: ~11 KB — the whole live selection loop (visit
//! bookkeeping, batch loop, session assembly) shares one state shape;
//! the selection state's normative D1–D6 contract lives in [`super`].
//!
//! Stop conditions are checked **between iterations only** (`max_jobs`
//! counts searches; pick-only complete iterations count toward neither cap;
//! 0 = unlimited) — a started job always runs to its granted budget, and a
//! fired cap is a normal stop. The censor hook is bump-only
//! ([`super::super::ledger::on_censored`]); every censor is committed to
//! the DB's `censor` table in its own transaction (crash-safe, plan20 D3).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Outcome;
use rusqlite::Connection;

use super::super::db::DbContent;
use super::super::harvest::{Job, JobClass, child_key, replay_job_path};
use super::super::ledger::{Ledger, on_censored, on_proven_draw};
use super::super::session::Session;
use super::{BASE_EVALS, WalkFrontier, is_handled, path_of, ply_of, root_state};

/// Is the scope root proven-stored (a proven DB row or a manifest/shard
/// path at the scope key)? The scope key is `""` for a global session.
#[must_use]
fn root_proven_stored(
    rows: &HashMap<String, Option<Outcome>>,
    session: &Session,
    scope: &str,
) -> bool {
    rows.get(scope).is_some_and(|o| o.is_some()) || session.has_path(scope)
}

/// Batch options (stop conditions, checked between iterations only; 0 =
/// unlimited; a fired cap is a normal stop).
pub struct WalkOptions {
    pub max_total_evals: u64,
    pub max_jobs: usize,
    pub max_runtime: u64,
    pub stop_file: PathBuf,
}

/// One `frontier-walk` batch's counters (for the session summary).
#[derive(Debug, Default)]
pub struct WalkSummary {
    /// Searches run (pick-only complete iterations count toward neither
    /// this nor the caps).
    pub jobs: usize,
    pub decisive: usize,
    pub censored: usize,
    /// Proven-draw jobs (`Complete` Draw): no shard, a `drawn` row only.
    pub drawn: usize,
    /// Censors on nodes without a prior ledger record — the ledger-growth
    /// signature (D4).
    pub fresh_censored: usize,
    pub evals: u64,
    /// Complete-marked candidates this session (or-close complete-marks do
    /// not count — D8).
    pub completes: usize,
    /// Dynamic or-closes this session (plan21 D4; the static D2(b') drops
    /// are the census's `or_closed`, not this).
    pub or_closes: usize,
    /// `""` = exhausted; otherwise `budget`/`max-jobs`/`max-runtime`/
    /// `stop-file`.
    pub stop_reason: String,
}

fn user_stop(opts: &WalkOptions, completed: usize, t_start: &Instant) -> Option<String> {
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

/// Run one visit's job at `budget` and book its outcome (D3/D5): shard on
/// decisive, bump + censor-row commit on censor (with the zero-eval
/// no-progress guard and the candidate admission), census fields
/// `pass`/`work_before`/`kind`.
///
/// # Errors
/// A censor commit failure on a censor — aborts the batch; the DB's last
/// committed state is consistent (plan20 D4).
#[allow(clippy::too_many_arguments)] // the walk's per-visit state, one arg each
fn visit(
    session: &mut Session,
    frontier: &mut WalkFrontier,
    ledger: &mut Ledger,
    censor: &Connection,
    key: &str,
    budget: u64,
    class: JobClass,
    kind: &'static str,
    summary: &mut WalkSummary,
) -> Result<(), String> {
    let (passes, work_before) = ledger
        .get(key)
        .map_or((0, 0), |e| (e.passes_failed, e.work_done));
    let job = Job {
        path: path_of(key),
        ply: ply_of(key),
        class,
        budget,
    };
    let mut rec = session.run_job_with_budget(&job, "frontier-walk", budget);
    match rec.outcome.as_str() {
        "censored" => {
            summary.censored += 1;
            if !ledger.contains(key) {
                summary.fresh_censored += 1;
            }
            on_censored(ledger, censor, key, rec.child_evals)?;
            if rec.child_evals == 0 {
                // A censor that spends nothing cannot advance the flat budget
                // (a non-budget Draw — e.g. an on-path repetition); complete-
                // mark so the walk moves on (record retained, D3).
                frontier.mark_complete(key);
            } else {
                frontier.admit(key);
            }
        }
        // A `Complete` Draw is a proof, not a censor: record it in the
        // path-keyed scheduling state and prune the subtree immediately.
        // No shard is exported (schema v2 has no draw outcome).
        "draw" => {
            summary.drawn += 1;
            on_proven_draw(ledger, censor, key)?;
            frontier.mark_drawn(key);
        }
        _ => {
            summary.decisive += 1;
            frontier.mark_decided(key);
            // Plan21 D4: a decisive `Loss` at an odd-absolute-ply child
            // (defender to move loses ⇒ the parent's white move wins) or-closes
            // the in-scope white-to-move parent — one proving child suffices at
            // an OR node. A decisive `Win` child refutes the parent's move and
            // must not trigger; black-to-move parents never trigger (D2).
            if rec.outcome == "loss" && frontier.or_close_parent(key) {
                summary.or_closes += 1;
            }
        }
    }
    rec.pass = Some(passes + 1);
    rec.work_before = Some(work_before);
    rec.kind = Some(kind);
    rec.emit();
    summary.jobs += 1;
    summary.evals += rec.child_evals;
    Ok(())
}

/// Run the `frontier-walk` batch: one search per iteration; the shallowest
/// candidate first; expansion instead of deepening (D2/D3/D6). `scope` is
/// the session's re-anchor key (`""` = global): the virtual-root rule and
/// the proven-stored check speak the scope root.
///
/// # Errors
/// A censor commit failure on a censor, or an illegal replay of a censor
/// record at expansion time (the lineage-gate contract — a defect, abort).
#[allow(clippy::too_many_arguments)] // the walk's per-batch state, one arg each
pub fn run_walk_batch(
    session: &mut Session,
    db: &DbContent,
    frontier: &mut WalkFrontier,
    ledger: &mut Ledger,
    censor: &Connection,
    base: u64,
    opts: &WalkOptions,
    scope: &str,
) -> Result<WalkSummary, String> {
    let t_start = Instant::now();
    let mut summary = WalkSummary::default();
    let rows: HashMap<String, Option<Outcome>> = db
        .rows
        .iter()
        .map(|r| (r.path.join(" "), r.outcome))
        .collect();
    loop {
        if opts.max_total_evals > 0 && summary.evals >= opts.max_total_evals {
            summary.stop_reason = "budget".to_string();
            break;
        }
        if let Some(reason) = user_stop(opts, summary.jobs, &t_start) {
            summary.stop_reason = reason;
            break;
        }
        let picked = frontier.pick(ledger);
        let Some((key, work)) = picked else {
            // Root rule (D2/D6): the scope root as a virtual candidate,
            // unless the scope root is proven-stored, or-closed (plan21 D6:
            // statically via a proven `Loss` child row, or dynamically via
            // an in-session or-close — both get the scoped stderr note), or
            // complete-blocked — then the walk is done. A scoped session's
            // proven-stored/or-closed scope root explains itself on stderr
            // (plan19 ruling b: no work beneath proven nodes; the global
            // session's shape is unchanged, so the notes fire only under a
            // scope).
            if root_proven_stored(&rows, session, scope) {
                if !scope.is_empty() {
                    eprintln!(
                        "frontier-walk: scope root {scope:?} is proven-stored — \
                         nothing to harvest"
                    );
                }
                break; // exhausted (empty stop reason)
            }
            if frontier.root_or_closed() {
                if !scope.is_empty() {
                    eprintln!(
                        "frontier-walk: scope root {scope:?} is or-closed by proven \
                         winning child — nothing to harvest"
                    );
                }
                break; // exhausted (empty stop reason)
            }
            if frontier.root_drawn() {
                if !scope.is_empty() {
                    eprintln!(
                        "frontier-walk: scope root {scope:?} is a proven draw — \
                         nothing to harvest"
                    );
                }
                break; // exhausted (empty stop reason)
            }
            if frontier.root_blocked() {
                break; // exhausted (empty stop reason)
            }
            visit(
                session,
                frontier,
                ledger,
                censor,
                scope,
                base,
                JobClass::Open,
                "search",
                &mut summary,
            )?;
            continue;
        };
        if work < base {
            visit(
                session,
                frontier,
                ledger,
                censor,
                &key,
                base,
                JobClass::Ledger,
                "search",
                &mut summary,
            )?;
            continue;
        }
        // Expansion visit (D3): replay, lex children, first unhandled.
        let (pos, _) = replay_job_path(&path_of(&key))
            .map_err(|e| format!("frontier-walk expand replay {key:?}: {e}"))?;
        let mut ucis: Vec<String> = pos
            .legal_moves_vec()
            .iter()
            .map(|&m| move_to_uci(m))
            .collect();
        ucis.sort();
        let target = ucis
            .iter()
            .map(|u| child_key(&key, u))
            .find(|ck| !is_handled(ck, &rows, ledger, base, |k| session.has_path(k)));
        let Some(child_key) = target else {
            frontier.mark_complete(&key);
            continue; // pick-only iteration: no job, no evals
        };
        let class = if ledger.contains(&child_key) {
            JobClass::Ledger
        } else {
            JobClass::OpenChild
        };
        visit(
            session,
            frontier,
            ledger,
            censor,
            &child_key,
            base,
            class,
            "expand",
            &mut summary,
        )?;
    }
    summary.completes = frontier.completes;
    Ok(summary)
}

/// The CLI-side session assembly (the `proofdb_harvest` frontier-walk
/// branch): load the censor state from the DB, build the frontier +
/// census, echo the census line, run the batch. `budget_evals = 0` selects
/// the standing base ([`BASE_EVALS`]). `scope` is the `--moves` re-anchor
/// key (`""` = global; plan19). `censor` is the RW connection to the
/// session DB (plan20 D3's carve-out: `censor` rows only).
///
/// # Errors
/// Anything the censor-state load or a censor commit on censor rejects (a
/// defect — abort, never patched).
pub fn run_walk_session(
    session: &mut Session,
    db: &DbContent,
    censor: &Connection,
    budget_evals: u64,
    opts: &WalkOptions,
    scope: &str,
) -> Result<WalkSummary, String> {
    let mut ledger = Ledger::load_db(censor)?;
    let base = if budget_evals > 0 {
        budget_evals
    } else {
        BASE_EVALS
    };
    let (mut frontier, mut census) = WalkFrontier::new(&ledger, db, scope);
    census.root = root_state(db, session, scope);
    eprintln!("{}", census.describe(base));
    run_walk_batch(
        session,
        db,
        &mut frontier,
        &mut ledger,
        censor,
        base,
        opts,
        scope,
    )
}
