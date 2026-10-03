//! SPDFPN-style parallel DF-PN over the shared sharded TT (opt-in).
//!
//! Enabled only by `--threads N` with N > 1 ([`Search::set_threads`]); at
//! N = 1 none of this module's machinery is constructed or touched and the
//! solver is exactly the sequential code path (byte-identical surface).
//! Reference design: Pawlewicz & Hayward 2014 (SPDFPN), mined in
//! `docs/plans/parallel/research_spdfpn.md`; pre-registered decisions in
//! `docs/plans/parallel/plan5.md`.
//!
//! # Soundness contract (normative)
//!
//! **Never a false decisive outcome.** The nondeterminism envelope accepted
//! by the product consumer (owner premise 2026-10-02) covers *which* valid
//! proof wins, work counts, and event ordering — nothing else. The argument,
//! mapped per `research_spdfpn.md` §3:
//!
//! - *Shared state*: the sharded [`TranspositionTable`] only. Its payload is
//!   path-independent by construction (repetition-dependent results are never
//!   cached; first-player-loss GHI shortcut), so cross-thread reuse of an
//!   entry is the same reuse the sequential solver performs at that key. The
//!   plan10 cross-worker ban concerns a global store across separate solves
//!   and does not cover in-run thread sharing. Write safety is the per-shard
//!   critical section (solved-never-downgraded, work monotone).
//! - *Thread-local state* (one `Search` per worker, never shared, never
//!   merged): the per-run repetition-draw cache (path-dependent by design —
//!   sharing across paths is the one sharp false-result edge), history,
//!   killers, and the pooled movegen/eval slots. Worker path stacks are
//!   seeded with the true root→job repetition keys, so per-job repetition
//!   semantics match the sequential path.
//! - *Virtual steering never produces results.* Virtual entries exist only in
//!   a per-worker snapshot ([`OverlaySnapshot`]) consulted at the frame entry
//!   and the child-evaluation site; a virtual hit yields either an immediate
//!   no-store "no progress" return (frame entry) or unsolved synthetic bounds
//!   (child evaluation). A virtual entry can therefore never flow into a
//!   solved TT store or a `NodeProven` event — a wrong virtual win/loss costs
//!   work only, exactly the paper's failure mode. Correctness is carried by
//!   the underlying dfpn rules (`dfpn::core`), unmodified.
//! - *Exhaustion / termination*: a root job that finishes under its work cap
//!   while no other worker is busy is the exact analog of the sequential
//!   `work_done < call_max_work` exhaustion test (all re-expansions left the
//!   bounds unchanged); see `jobs::Shared::complete_locked`. Root solvedness
//!   is always decided by a plain TT probe — virtual entries never count.
//!
//! # Coordination protocol
//!
//! One *job lock* (`jobs::Shared::job_lock`, a `Mutex`) held only during
//! candidate-finding ([`Shared::dispatch`], the SPDFPN `TRYRUNJOB` descent)
//! and end-of-job bookkeeping; released while the assigned worker solves its
//! job (a dfpn call capped at [`MAX_WORK_PER_JOB`] child evals, thresholds
//! `(INF, INF)`). While a job runs, its node carries a virtual win/loss entry
//! that steers other workers' selection away from the subtree. Work
//! assignment candidates are nodes on the root→MPN path whose TT `work` is
//! below `W`, closest to the root; the periodic root re-scan (every
//! `ROOT_SCAN_INTERVAL` dispatch attempts) refreshes the root's stale bounds
//! — the analog of the paper's post-job re-propagation.
//!
//! # Deviations from the paper (declared, plan5 decisions)
//!
//! - Work unit is *child evals* (our deterministic cumulative TT `work`),
//!   not DFPN calls (decision 3).
//! - Jobs run with `(INF, INF)` thresholds; the paper derives path-consistent
//!   thresholds from the descent. Thresholds guide effort, not soundness;
//!   the W cap bounds each job regardless.
//! - Event emissions are serialized by forwarding every worker's events
//!   through a single forwarder thread into the parent's sink; ordering is
//!   still nondeterministic under N > 1 (decision 7).
//! - `child_eval_budget` exhaustion stays enforced call-relatively but is
//!   **advisory** under N > 1: the run ends `Draw` +
//!   [`ExitReason::BudgetExhausted`] nondeterministically (constraint 3
//!   restated in the CLI help, not silently broken).
//!
//! # Resource and contract restatements
//!
//! RAM = one shared TT + O(N) small per-worker state (caches, killers,
//! pools) — the "RAM = TT only" contract restated, not broken (decision 9).
//! The TT's sequential-owner contract (`clear`/`new_generation` are never
//! called while workers run; the parallel phase shares one generation) is
//! owned here and holds by construction: the coordinator never clears the
//! table during a parallel bounded search. The wall-clock timeout remains
//! wall-clock (decision 8); worker `time_exceeded` uses copies of the
//! run-wide deadline and stop flag.

mod jobs;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;

use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::proof_event::ProofEvent;
use crate::zobrist::INF;

use super::RoundTermination;
use super::Search;
use jobs::{Job, Shared};

/// Work threshold `W` (SPDFPN `MaxWorkPerJob`), in child evaluations
/// (decision 3): a job's dfpn call halts once it has spent this many evals,
/// storing unsolved bounds exactly like a sequential work-chunk cutoff, so
/// jobs are resumable by construction. Deviation from the pre-registered
/// initial value 1000, recorded for `report5a`: on the hard class, 1 000-eval
/// dives demonstrably cannot change any stored bounds (instrumented runs —
/// nodes cycled with identical (pn, dn) across tens of thousands of jobs),
/// so jobs degenerated into pure re-verification. 20 000 evals ≈ 4 % of a
/// sequential chunk keeps jobs resumable and granular while making each job's
/// dive productive; the sweep that locks the final value remains plan5b's.
pub(super) const MAX_WORK_PER_JOB: u64 = 20_000;

/// Lock-free per-worker snapshot of the other workers' virtual entries,
/// refreshed once per job dispatch (staleness within a job costs work only:
/// a worker may descend into a subtree someone claimed *after* its snapshot
/// was taken, where the frame-entry steering stops it immediately).
pub(super) struct OverlaySnapshot {
    entries: Arc<[VirtualEntryLite]>,
}

#[derive(Clone, Copy)]
struct VirtualEntryLite {
    key: u64,
    outcome: Outcome,
}

impl OverlaySnapshot {
    fn new(entries: Vec<jobs::VirtualEntry>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|e| VirtualEntryLite {
                    key: e.key,
                    outcome: e.outcome,
                })
                .collect::<Vec<_>>()
                .into(),
        }
    }

    /// The virtual outcome assigned to `key` by another worker, if any.
    /// Used at the frame entry (core.rs) and the child-evaluation site
    /// (children.rs); must never be consulted for solved-result reuse.
    pub(super) fn outcome_for(&self, key: u64) -> Option<Outcome> {
        self.entries
            .iter()
            .find(|e| e.key == key)
            .map(|e| e.outcome)
    }
}

/// Steering bounds for a virtually assigned child (children.rs): unsolved
/// synthetic bounds that make the subtree unattractive in the parent's
/// selection dimension without ever looking solved. A virtual win for the
/// child's mover (`Outcome::Win`) excludes the child from OR-side proof
/// selection (`pn = INF`); a virtual loss excludes it from AND-side
/// selection (`dn = INF`). The nonzero partner value keeps aggregate bounds
/// non-degenerate (the TT store's `(INF, INF)` → `(1, 1)` fallback must not
/// trigger on virtual-influenced aggregates).
pub(super) fn steering_bounds(outcome: Outcome) -> (u64, u64) {
    match outcome {
        Outcome::Win | Outcome::Draw => (INF, 1),
        Outcome::Loss => (1, INF),
    }
}

/// Run one bounded-search phase with `--threads N`, N > 1.
///
/// Stage-2a shape (deviation from the pre-registered "workers replace the
/// chunk loop" design, declared in `report5a`): the **coordinator thread runs
/// the unmodified sequential chunk loop** ([`Search::chunk_loop`]) — the exact
/// N = 1 code path — while N-1 SPDFPN helper workers pre-warm the shared TT
/// with W-capped, threshold-guided, virtually-steered jobs ([`worker_loop`]).
/// Rationale: a pure worker-pool search could not converge on the hard class
/// within this session (fine-grained jobs re-verify saturated regions without
/// advancing the global proof-number state; diagnosed with instrumented runs,
/// see `report5a`). The helper pool is strictly additive: helpers store only
/// path-independent TT payload (the same entries a sequential chunk leaves
/// behind), so the coordinator's trajectory stays sound and its convergence
/// is at worst the sequential one — outcome agreement with the sequential
/// solver is structural, not measured-only.
///
/// The coordinator's outcome/termination classification is exactly the
/// sequential one (including sound `Exhausted` detection inside the chunk
/// loop); the child-eval budget and the wall deadline bind the coordinator
/// directly. Helpers stop on the deadline, the run-wide stop flag, the
/// round-cap/budget helper-relative totals, or a decisive root outcome.
pub(super) fn bounded_search_with_helpers(
    parent: &mut Search,
    pos: &mut Position,
    max_depth: u32,
    round_work_cap: u64,
) -> (Outcome, Vec<Move>, RoundTermination) {
    if parent.time_exceeded() || parent.child_eval_budget_exceeded() {
        return (Outcome::Draw, Vec::new(), RoundTermination::ResourceCut);
    }

    let threads = parent.threads;
    let prefix_rep_keys = parent.prefix_path.clone().unwrap_or_default();

    // Serialized event forwarding (decision 7): helper emissions are routed
    // through a single forwarder thread into the parent's sink; the
    // coordinator emits directly. The forwarder is joined before returning.
    let parent_tx = parent.proof_event_sender.clone();
    let (event_tx, event_rx) = mpsc::channel::<ProofEvent>();

    // Stable root template for the helpers: cloned before the coordinator's
    // chunk loop starts mutating `pos`; the main thread never touches it.
    let root_owned = pos.clone();

    let shared = &Shared {
        tt: parent.tt.clone(),
        stop: AtomicBool::new(false),
        busy: AtomicUsize::new(0),
        total_evals: AtomicU64::new(0),
        total_nodes: AtomicU64::new(0),
        jobs_done: AtomicU64::new(0),
        job_lock: std::sync::Mutex::new(jobs::JobLockState {
            entries: Vec::new(),
            retired: std::collections::HashSet::new(),
        }),
        root_key: root_owned.hash(),
        max_depth,
        round_cap: round_work_cap,
        budget_end: parent.child_eval_budget.saturating_sub(parent.child_evals),
        prefix_rep_keys,
        root: &root_owned,
    };

    let scope_result = std::thread::scope(|scope| {
        if parent_tx.is_some() {
            scope.spawn(move || {
                for event in event_rx {
                    let _ = parent_tx.as_ref().expect("checked").send(event);
                }
            });
        } else {
            drop(event_rx);
        }

        // Helpers 1..N: fresh `Search`es sharing the TT (Arc) with per-worker
        // thread-local state; the repetition cache is per-worker by
        // construction (decision 6).
        for id in 1..threads {
            let mut worker = worker_like(parent, Some(event_tx.clone()));
            let root = root_owned.clone();
            scope.spawn(move || worker_loop(shared, &mut worker, id, &root));
        }

        // The coordinator runs the unmodified sequential chunk loop; when it
        // returns (decisive, exhausted, or resource-cut), the round is over
        // for the helpers too — release them immediately instead of letting
        // them churn until their own deadline.
        let result = parent.chunk_loop(pos, max_depth, round_work_cap);
        shared.stop.store(true, Ordering::Release);
        result
    });
    // All `event_tx` clones are dropped at scope end, so the forwarder's
    // receive loop has ended and was joined with the scope.

    // Helper-pool diagnostic (stderr only, never on the N = 1 path): total
    // helper work vs the coordinator's own counters — the per-thread work
    // split the smoke/campaign record.
    eprintln!(
        "[parallel] helpers={} jobs={} helper_evals={} helper_nodes={} coordinator_evals={}",
        threads - 1,
        shared.jobs_done.load(Ordering::Relaxed),
        shared.total_evals.load(Ordering::Relaxed),
        shared.total_nodes.load(Ordering::Relaxed),
        parent.child_evals,
    );
    scope_result
}

/// A per-worker `Search`: config copied from the coordinator, TT shared by
/// `Arc`, everything else (history, killers, repetition cache, pools, path)
/// thread-local and fresh. `threads = 1` so a worker can never re-enter the
/// parallel path.
fn worker_like(parent: &Search, sender: Option<mpsc::Sender<ProofEvent>>) -> Search {
    let mut worker = Search::new(1);
    worker.tt = parent.tt.clone();
    worker.epsilon_num = parent.epsilon_num;
    worker.epsilon_den = parent.epsilon_den;
    worker.scorer = parent.scorer.clone();
    worker.timeout = parent.timeout;
    worker.deadline = parent.deadline;
    worker.start = parent.start;
    worker.max_ply = parent.max_ply;
    worker.stop_flag = parent.stop_flag.clone();
    worker.memory_limited = parent.memory_limited.clone();
    worker.proof_event_sender = sender;
    worker.preflight_enabled = false;
    worker.threads = 1;
    worker
}

/// The SPDFPN helper worker loop: dispatch (job lock) → solve (lock-free,
/// W-capped dfpn) → bookkeeping (job lock), until a stop condition fires.
/// Helpers are strictly additive to the coordinator's sequential chunk loop:
/// they only store path-independent TT payload and never touch coordinator
/// state.
fn worker_loop(shared: &Shared, worker: &mut Search, id: usize, root: &Position) {
    loop {
        if shared.stop.load(Ordering::Acquire) || worker.time_exceeded() {
            shared.stop.store(true, Ordering::Release);
            return;
        }
        match shared.dispatch(worker, id) {
            Some(job) => {
                let (job_evals, job_nodes, pre_bounds) = run_job(worker, root, &job);
                // Did the job store a decisive outcome for its node?
                let solved = worker.tt.probe(job.key).and_then(|e| e.outcome).is_some();
                let mut state = shared.job_lock.lock().expect("job lock poisoned");
                shared.complete_locked(
                    &mut state, id, job.key, job_evals, job_nodes, pre_bounds, solved,
                );
                drop(state);
                worker.parallel_overlay = None;
                // SPDFPN re-propagation (paper §1 item 3): refresh ancestor
                // bounds from current child entries so the next dispatch's
                // MPN descent routes through current information and just-
                // solved subtrees propagate upward. Costs probes only.
                repropagate(shared, root, &job.path_moves);
            }
            None => {
                if shared.stop.load(Ordering::Acquire) {
                    return;
                }
                // Another worker holds the only available work (assigned
                // subtrees block the descent). Wait for its job to finish;
                // the wall deadline bounds any pathology.
                shared.idle_sleep();
            }
        }
    }
}

/// Execute one job: replay the root→job path on a private `Position`, seed
/// the worker's path/move stacks (root-relative repetition keys and proof
/// event paths), and run a W-capped dfpn call with `(INF, INF)` thresholds.
/// Returns `(job child evals, job dfpn nodes)`.
fn run_job(worker: &mut Search, root: &Position, job: &Job) -> (u64, u64, Option<(u64, u64)>) {
    let pre_bounds = worker.tt.probe(job.key).map(|e| (e.pn, e.dn));
    let mut pos = root.clone();
    for &m in &job.path_moves {
        pos.do_move(m);
    }
    worker.path_stack = job.path_rep_keys.clone();
    worker.move_stack = job.path_moves.clone();
    // The same-child resume clause (decision 4) applies to this job's top
    // frame only: `frame_depth` of the job call is `path_stack.len() + 1`.
    worker.parallel_resume_top = Some(worker.path_stack.len() + 1);

    let evals_before = worker.child_evals;
    let nodes_before = worker.nodes;
    let _ = worker.dfpn(
        &mut pos,
        job.th_pn,
        job.th_dn,
        job.max_depth,
        MAX_WORK_PER_JOB,
        job.is_or_node,
    );
    worker.parallel_resume_top = None;
    (
        worker.child_evals - evals_before,
        worker.nodes - nodes_before,
        pre_bounds,
    )
}

/// SPDFPN re-propagation (paper §1 item 3): after a job at the end of
/// `path_moves` finishes, refresh the stored unsolved bounds of every
/// ancestor on the root→job path from its children's current TT entries.
/// This is what lets the next dispatch's MPN descent route through current
/// information and lets saturated ancestors be concluded by blocker jobs —
/// without it, stale bounds strand the search at shallow depths.
///
/// Reads and stores only path-independent TT payload (unsolved bounds);
/// solved entries are never touched. Costs one movegen + one probe per
/// child per ancestor level, per job — coordination overhead outside the
/// job lock.
fn repropagate(shared: &Shared, root: &Position, path_moves: &[Move]) {
    let mut pos = root.clone();
    // Ancestors of the job node are the positions *before* each move
    // (root included); the root player's side is the OR role at even depths.
    for (i, &m) in path_moves.iter().enumerate() {
        refresh_node_bounds(shared, &pos, i % 2 == 0);
        pos.do_move(m);
    }
}

/// Recompute one node's unsolved aggregate bounds from its children's
/// current TT entries and refresh the entry (`TranspositionTable::
/// refresh_bounds`). Mirrors the dfpn selection aggregation: OR — min over
/// child pn, sum over child dn; AND — the reverse. Solved children
/// contribute their outcome numbers; missing/ineligible children the neutral
/// `(1, 1)`, exactly like `evaluate_child`'s reuse fallbacks.
fn refresh_node_bounds(shared: &Shared, pos: &Position, is_or_node: bool) {
    let mut pos = pos.clone();
    let key = pos.hash();
    if let Some(entry) = shared.tt.probe(key)
        && entry.outcome.is_some()
    {
        return; // solved: final, never touched
    }
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    if moves.is_empty() {
        return; // unclassified terminal: a blocker job's business
    }
    let mut pn_agg = if is_or_node { INF } else { 0 };
    let mut dn_agg = if is_or_node { 0 } else { INF };
    let mut best: Option<(usize, u64)> = None; // (child index, cmp value)
    for i in 0..moves.len() {
        let m = moves[i];
        pos.do_move(m);
        let child_key = pos.hash();
        let child_entry = shared.tt.probe(child_key);
        pos.undo_move(m);
        let (cpn, cdn) = match child_entry {
            Some(e) => match e.outcome {
                Some(o) => o.pn_dn_for(!is_or_node),
                None if e.pn > 0 && e.dn > 0 => (e.pn, e.dn),
                None => (1, 1),
            },
            None => (1, 1),
        };
        if is_or_node {
            pn_agg = pn_agg.min(cpn);
            dn_agg = dn_agg.saturating_add(cdn).min(INF);
        } else {
            pn_agg = pn_agg.saturating_add(cpn).min(INF);
            dn_agg = dn_agg.min(cdn);
        }
        if child_entry.as_ref().is_none_or(|e| e.outcome.is_none()) {
            let cmp = if is_or_node { cpn } else { cdn };
            if best.is_none_or(|(_, b)| cmp < b) {
                best = Some((i, cmp));
            }
        }
    }
    let (best_move, best_child) = best.map_or((Move::NONE, u8::MAX), |(i, _)| (moves[i], i as u8));
    shared
        .tt
        .refresh_bounds(key, best_move, best_child, pn_agg, dn_agg);
}
