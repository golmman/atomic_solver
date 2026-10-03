//! SPDFPN job machinery: virtual TT, job lock, and `TRYRUNJOB` dispatch.
//!
//! Reference design: `docs/plans/parallel/research_spdfpn.md` §1 (items 1–3)
//! mapped onto this solver per plan5's pre-registered decisions 3–5. The
//! soundness contract lives in the [`super`] module header; this file documents
//! the mechanics.

use atomic_movegen::types::{Move, MoveList};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::position::{Outcome, Position};
use crate::search::dfpn::Search;
use crate::search::tt::TranspositionTable;
use crate::zobrist::INF;

use super::MAX_WORK_PER_JOB;

/// One virtual-TT entry: a node currently assigned to a worker, marked with a
/// virtual win/loss (by `p <= d`, the paper's rule) from the assigned node's
/// side-to-move perspective. At most one entry per worker exists at any time,
/// so the table holds ≤ N entries (decision 5: per-ply ≤ N — degenerately
/// satisfied by a flat ≤ N-entry list, which the ≤ N-element linear scans make
/// cheaper than any per-ply indexing).
#[derive(Clone, Copy, Debug)]
pub(super) struct VirtualEntry {
    pub(super) key: u64,
    pub(super) outcome: Outcome,
    pub(super) thread: usize,
}

/// Job-lock-protected state: the virtual entries. This `Mutex` is *the* job
/// lock; it is held only during candidate-finding and end-of-job bookkeeping,
/// never while a worker solves.
pub(super) struct JobLockState {
    pub(super) entries: Vec<VirtualEntry>,
    /// Job nodes whose dfpn call finished without spending a single child
    /// eval (repetition-cache hit, already-resolved entry, deadline hit).
    /// Their value is already reflected in the run's caches/TT bounds, so the
    /// dispatch stops routing into them — candidacy via `work < W` (bumped on
    /// retirement) *and* the blocker rule (this set). Real dfpn calls from a
    /// parent job still expand them per-path with the correct repetition
    /// semantics, so retirement never changes any node's value.
    pub(super) retired: std::collections::HashSet<u64>,
}

/// State shared by all workers of one bounded-search call.
///
/// Created fresh per call (each work chunk / refinement round), so `stop` and
/// `exhausted` are call-local. Budget and round-cap checks use *relative*
/// totals: `total_evals` counts the evals spent by this call's workers only.
pub(super) struct Shared<'a> {
    pub(super) tt: Arc<TranspositionTable>,
    /// Call-local stop: set when the root is solved, the budget/round cap is
    /// spent, or exhaustion was claimed. Workers also honor the run-wide
    /// stop flag and deadline through their own `Search` copies.
    pub(super) stop: AtomicBool,
    /// Workers currently holding a job. Incremented under the job lock inside
    /// `dispatch`, decremented under it in `complete`, so a `busy == 0`
    /// observation under the lock is race-free.
    pub(super) busy: AtomicUsize,
    /// Cumulative child evals / dfpn nodes / completed jobs across all
    /// helpers (each worker reports per-job deltas under the job lock).
    /// Coordinator work is *not* counted here — it accrues directly on the
    /// coordinator's own `Search` counters.
    pub(super) total_evals: AtomicU64,
    pub(super) total_nodes: AtomicU64,
    pub(super) jobs_done: AtomicU64,
    pub(super) job_lock: Mutex<JobLockState>,
    pub(super) root_key: u64,
    pub(super) max_depth: u32,
    /// Round work cap, relative to this call's start (`u64::MAX` = uncapped).
    pub(super) round_cap: u64,
    /// Global child-eval budget, relative to this call's start.
    pub(super) budget_end: u64,
    /// Repetition keys prefixing the root (from `search_depth_with_prefix`);
    /// every job's path stack is seeded with them so repetition semantics at
    /// the root match the sequential path.
    pub(super) prefix_rep_keys: Vec<u64>,
    /// Root position template; each dispatch/job clones it (read-only).
    pub(super) root: &'a Position,
}

/// Wait between unproductive dispatch attempts while another worker holds the
/// only available work. The wall deadline bounds any pathology.
const IDLE_SLEEP: Duration = Duration::from_micros(250);

/// Hard cap on the `TRYRUNJOB` descent depth below the root (the paper's
/// "shallow enough for load balance" criterion, made explicit). Without it
/// the MPN descent can follow stored best-move chains into dozens of plies,
/// making every dispatch pay O(depth x branching) probes for a job whose
/// subtree the W cap barely scratches. A node at the cap is taken as the job
/// regardless of its past work (resumption): its W-capped dfpn call does the
/// real work at that depth.
const MAX_DESCENT_PLIES: usize = 12;

/// A dispatched job: search the node at the end of `path_moves` from the
/// root, with the given OR/AND role and remaining depth, capped at W child
/// evals. `th_pn`/`th_dn` are the path-derived thresholds (decision:
/// `TRYRUNJOB` accumulates them top-down exactly like the dfpn child-call
/// formulas) — they make a W-capped job *resumable*: a node whose local
/// (d)pn already answers the root search's current question returns at once
/// instead of burning its whole work cap re-diving a saturated region.
pub(super) struct Job {
    pub(super) key: u64,
    pub(super) path_moves: Vec<Move>,
    /// Repetition keys of the positions *strictly above* the job node
    /// (prefix keys + one per played move), seeding the worker's path stack.
    pub(super) path_rep_keys: Vec<u64>,
    pub(super) is_or_node: bool,
    pub(super) max_depth: u32,
    pub(super) th_pn: u64,
    pub(super) th_dn: u64,
}

impl<'a> Shared<'a> {
    /// Find and claim a job for `worker` (SPDFPN `TRYRUNJOB`, Alg. 4).
    ///
    /// Called with the worker's `Search` so the per-job virtual snapshot can
    /// be installed (the worker's hot path reads it lock-free; see the
    /// [`super`] module header for the staleness argument). On success the
    /// virtual entry is inserted and `busy` incremented — both under the job
    /// lock — before the lock is released for the solve itself.
    ///
    /// Candidate rule (decision 5, with resumption): descend the root→MPN
    /// path and take the first node whose past work is below `W` — closest to
    /// the root — following the best unassigned, unsolved child otherwise. A
    /// node whose children are all solved or assigned is taken as a
    /// *blocker* job regardless of its work: its dfpn call concludes the
    /// solved children (resumption; the same-child resume clause keeps its
    /// previous best child) or marks them explored. This is what lets
    /// saturated nodes finish bottom-up; combined with the post-job
    /// re-propagation (`super::repropagate`) it replaces the paper's
    /// threshold-derived descent. When the root itself is assigned elsewhere
    /// and no child is dispatchable, no job exists — the caller waits.
    pub(super) fn dispatch(&self, worker: &mut Search, id: usize) -> Option<Job> {
        if self.stop.load(Ordering::Acquire) {
            return None;
        }
        let total = self.total_evals.load(Ordering::Relaxed);
        if total >= self.round_cap || total >= self.budget_end {
            // Round cap / global budget spent (advisory under N > 1 — the run
            // ends as Draw + ResourceCut; `ExitReason` classification happens
            // upstream on the coordinator's `Search`).
            self.stop.store(true, Ordering::Release);
            return None;
        }

        let mut state = self.job_lock.lock().expect("job lock poisoned");

        // Root solved? (Plain TT probe — virtual entries never count.)
        if let Some(entry) = self.tt.probe(self.root_key)
            && Search::resolved_from_entry(&entry, self.max_depth).is_some()
        {
            self.stop.store(true, Ordering::Release);
            return None;
        }
        // The coordinator's chunk loop owns the root search; helpers only
        // ever work strictly below it, so the root is never a job candidate.
        let root_assigned = true;

        let mut pos = self.root.clone();
        let mut path_moves: Vec<Move> = Vec::new();
        let mut path_rep_keys: Vec<u64> = self.prefix_rep_keys.clone();
        let mut is_or_node = true;
        let mut remaining = self.max_depth;
        let mut th_pn = INF;
        let mut th_dn = INF;
        let mut job_key: Option<u64> = None;
        loop {
            let key = pos.hash();
            // The root assigned to another worker is never taken as a job;
            // deeper nodes cannot be assigned (assigned children are skipped
            // below), so this guard only binds at the descent start.
            let may_take = !(root_assigned && path_moves.is_empty());
            let entry = self.tt.probe(key);
            let work = entry.as_ref().map_or(0, |e| e.work);
            if may_take
                && (entry.is_none()
                    || work < MAX_WORK_PER_JOB
                    || remaining <= 1
                    || path_moves.len() >= MAX_DESCENT_PLIES)
            {
                job_key = Some(key);
                break;
            }
            // Node aggregate for the threshold update: the entry's stored
            // bounds (refreshed by the post-job re-propagation), the same
            // values a dfpn frame at this node would aggregate from.
            let (node_pn, node_dn) = match entry {
                Some(e) if e.outcome.is_none() => (e.pn, e.dn),
                _ => (1, 1),
            };
            let mut moves = MoveList::new();
            pos.legal_moves(&mut moves);
            if moves.is_empty() {
                // Defensive: an unclassified terminal with past work; the
                // worker's dfpn entry classifies it.
                if may_take {
                    job_key = Some(key);
                }
                break;
            }
            let mut best: Option<(Move, u64, u64, u64)> = None; // (mv, cmp, pn, dn)
            let mut second_cmp: Option<u64> = None;
            for i in 0..moves.len() {
                let m = moves[i];
                pos.do_move(m);
                let child_key = pos.hash();
                let child_rep = pos.repetition_key();
                let rule50_expired = pos.board().rule50() >= 100;
                let child_entry = self.tt.probe(child_key);
                pos.undo_move(m);
                // A child repeating a position on the descent path is a
                // repetition draw in this context (first-player-loss
                // shortcut): the parent's real dfpn call classifies it with
                // zero work, so it is never worth a job.
                if !rule50_expired && path_rep_keys.contains(&child_rep) {
                    continue;
                }
                if state.entries.iter().any(|e| e.key == child_key) {
                    continue; // assigned to another worker: steer away
                }
                if state.retired.contains(&child_key) {
                    continue; // zero-work retired: value already known
                }
                if child_entry
                    .as_ref()
                    .and_then(|e| Search::resolved_from_entry(e, remaining - 1))
                    .is_some()
                {
                    continue; // solved: progress past it
                }
                let (pn, dn) = match child_entry {
                    Some(e) if e.outcome.is_none() => (e.pn, e.dn),
                    _ => (1, 1),
                };
                let cmp = if is_or_node { pn } else { dn };
                if best.is_none_or(|(_, b, _, _)| cmp < b) {
                    if let Some((_, b, _, _)) = best {
                        second_cmp = Some(second_cmp.map_or(b, |s: u64| s.min(b)));
                    }
                    best = Some((m, cmp, pn, dn));
                } else if second_cmp.is_none_or(|s| cmp < s) {
                    second_cmp = Some(cmp);
                }
            }
            match best {
                Some((m, _cmp, child_pn, child_dn)) => {
                    // Threshold update for the child call, mirroring the dfpn
                    // child-call formulas exactly (core.rs): OR —
                    // np = min(th_pn, εceil(p2)), nd = th_dn − dn + dn_c;
                    // AND — the perspective-mirrored counterpart.
                    let p2 = second_cmp.unwrap_or(INF);
                    let (nth_pn, nth_dn) = if is_or_node {
                        (
                            th_pn.min(worker.epsilon_ceil(p2)),
                            if th_dn == INF {
                                INF
                            } else {
                                th_dn.saturating_sub(node_dn).saturating_add(child_dn)
                            },
                        )
                    } else {
                        (
                            if th_pn == INF {
                                INF
                            } else {
                                th_pn.saturating_sub(node_pn).saturating_add(child_pn)
                            },
                            th_dn.min(worker.epsilon_ceil(p2)),
                        )
                    };
                    th_pn = nth_pn;
                    th_dn = nth_dn;
                    path_rep_keys.push(pos.repetition_key());
                    path_moves.push(m);
                    pos.do_move(m);
                    is_or_node = !is_or_node;
                    remaining -= 1;
                }
                None => {
                    // Every child is solved or assigned: re-search this node
                    // (resumption). Its dfpn call concludes the solved
                    // children or works around the assigned ones. At an
                    // assigned root with no dispatchable child, wait.
                    if may_take {
                        job_key = Some(key);
                    }
                    break;
                }
            }
        }
        let job_key = job_key?;

        // Claim: virtual win/loss by p <= d (paper §1 item 3), then hand out
        // the job. `busy` is incremented under the same lock so a concurrent
        // dispatch's `busy` observation is race-free.
        let (vpn, vdn) = match self.tt.probe(job_key) {
            Some(e) => (e.pn, e.dn),
            None => (INF, INF),
        };
        let virtual_outcome = if vpn <= vdn {
            Outcome::Win
        } else {
            Outcome::Loss
        };
        state.entries.push(VirtualEntry {
            key: job_key,
            outcome: virtual_outcome,
            thread: id,
        });
        self.busy.fetch_add(1, Ordering::AcqRel);

        // Per-job virtual snapshot for the worker's lock-free hot path:
        // every entry except the worker's own (its job node must be searchable
        // by itself). Stale within the job by design — see the module header.
        let snapshot: Vec<VirtualEntry> = state
            .entries
            .iter()
            .copied()
            .filter(|e| e.thread != id)
            .collect();
        worker.parallel_overlay = Some(super::OverlaySnapshot::new(snapshot));

        Some(Job {
            key: job_key,
            path_moves,
            path_rep_keys,
            is_or_node,
            max_depth: remaining,
            th_pn,
            th_dn,
        })
    }

    /// End-of-job bookkeeping. Must be called with the job lock held
    /// (callers lock; this keeps the busy/entry/counter updates atomic).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn complete_locked(
        &self,
        state: &mut JobLockState,
        id: usize,
        job_key: u64,
        job_evals: u64,
        job_nodes: u64,
        pre_bounds: Option<(u64, u64)>,
        solved: bool,
    ) {
        // Progress = the job stored a decisive outcome or changed the node's
        // stored (pn, dn) bounds. A job that changed neither learned nothing
        // (its exploration was pure re-verification of saturated subtrees, or
        // it hit a repetition-cache/resolution shortcut with no store) —
        // without retirement the `work < W` candidacy and the blocker rule
        // would re-pick it forever. Retirement is call-local dispatch
        // bookkeeping only: real dfpn calls from a parent job still expand
        // the node per-path with the correct repetition semantics, so no
        // node's value changes.
        let post_bounds = self.tt.probe(job_key).map(|e| (e.pn, e.dn));
        let progress = pre_bounds != post_bounds;
        state.entries.retain(|e| e.thread != id);
        self.total_evals.fetch_add(job_evals, Ordering::Relaxed);
        self.total_nodes.fetch_add(job_nodes, Ordering::Relaxed);
        self.jobs_done.fetch_add(1, Ordering::Relaxed);
        // A job that spent almost no child evals and stored no outcome was
        // cut by its thresholds (the node's local numbers already answer the
        // root search's current question) or hit a cache/resolution shortcut:
        // the dispatch must stop routing into it this call, or it would be
        // re-picked forever. Real dfpn calls from a parent job still expand
        // it per-path with the correct repetition semantics, so retirement
        // never changes any node's value.
        // A job that spent almost no child evals and stored no outcome was
        // cut by its thresholds (the node's local numbers already answer the
        // root search's current question) or hit a cache/resolution shortcut
        // (a repetition-cache hit even returns with zero evals and no store,
        // by the first-player-loss GHI shortcut). Without retirement the
        // `work < W` candidacy and the blocker rule would re-pick the node
        // forever. Retirement is call-local dispatch bookkeeping only: real
        // dfpn calls from a parent job still expand the node per-path with
        // the correct repetition semantics, so no node's value changes.
        if !solved && !progress {
            // Raise `work` so candidacy fails; if no entry exists, insert the
            // standard unsolved `(1, 1)` marker (the same store shape the
            // GHI draw suppression uses).
            if !self.tt.bump_work(job_key, MAX_WORK_PER_JOB) {
                self.tt.store(
                    job_key,
                    Move::NONE,
                    u8::MAX,
                    MAX_WORK_PER_JOB,
                    None,
                    1,
                    1,
                    0,
                    0,
                );
            }
            state.retired.insert(job_key);
        }
        let busy_before = self.busy.fetch_sub(1, Ordering::AcqRel);

        if let Some(entry) = self.tt.probe(self.root_key)
            && Search::resolved_from_entry(&entry, self.max_depth).is_some()
        {
            self.stop.store(true, Ordering::Release);
            return;
        }
        // NOTE (stage 2a): no exhaustion claim. A root job finishing under its
        // work cap is NOT a sound exhaustion signal for W-capped resumable
        // jobs: the all-explored break it would rely on can fire while deep
        // regions remain unexplored (threshold-cut re-verification), and a
        // premature Exhausted would manufacture a false proven-Draw surface.
        // Without a decisive root outcome the call reports ResourceCut /
        // CapCut — the same unfinished-search surface a sequential timeout
        // produces. Sound exhaustion detection is deferred to plan5b.
        let _ = (busy_before, job_key, job_evals);
    }

    /// Park briefly after an unproductive dispatch (another worker holds the
    /// only available work).
    pub(super) fn idle_sleep(&self) {
        std::thread::sleep(IDLE_SLEEP);
    }
}
