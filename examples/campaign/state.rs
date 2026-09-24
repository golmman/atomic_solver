//! Campaign master proof-state v0: AND/OR obligation table, selection,
//! checkpoint dumps, and session finalization. Split out of the
//! `campaign_master` bin to keep both files under the size guideline; the
//! dispatch loop lives in the bin, the state machine lives here.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_event::{NodeProven, ProofEvent};
use atomic_solver::proof_tree::{ProofTree, ProofTreeWorkerHandle, validate_proof_tree};

use super::{EventRec, JOBS_DIR, RESULTS_DIR, SESSION_FILE, STOP_FILE};

// ---------------------------------------------------------------- state ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeafStatus {
    Open,
    Won,
    Lost,
}

pub struct Leaf {
    mv: Move,
    status: LeafStatus,
    /// Proven win depth of the leaf position once Won.
    depth: u32,
    /// Advisory proof numbers (worker-fed when available; (1,1) prior).
    pn: u64,
    dn: u64,
    /// Total worker child-evals spent on this leaf.
    work: u64,
    /// Bounded jobs dispatched (drives the doubling budget).
    slices: u32,
    last_worker: Option<usize>,
    /// In-flight job: (job_id, worker, dispatched-at).
    job: Option<(String, usize, Instant)>,
    static_rank: usize,
}

pub struct Child {
    mv: Move,
    /// Rule-derived terminal classification at the child position (opponent
    /// to move): `Some(Loss)` resolves the child; `Some(Win|Draw)` refutes.
    terminal: Option<Outcome>,
    /// Zobrist hash of the child position (master's own replay).
    hash: u64,
    replies: Vec<Leaf>,
    resolved: bool,
    refuted: bool,
    /// Depth of the synthesized Loss event once resolved.
    depth: u32,
    synthesized: bool,
    static_rank: usize,
}

impl Child {
    pub fn refresh(&mut self) {
        if self.resolved || self.refuted {
            return;
        }
        if self.terminal == Some(Outcome::Loss) {
            self.resolved = true;
            self.depth = 0;
            return;
        }
        if self.terminal.is_some() {
            self.refuted = true;
            return;
        }
        if self.replies.iter().any(|l| l.status == LeafStatus::Lost) {
            self.refuted = true;
            return;
        }
        if !self.replies.is_empty() && self.replies.iter().all(|l| l.status == LeafStatus::Won) {
            self.resolved = true;
            self.depth = self.replies.iter().map(|l| l.depth).max().unwrap_or(0) + 1;
        }
    }

    /// OR-level pn over open leaves (PNS: sum over AND children).
    pub fn child_pn(&self) -> u64 {
        if self.resolved {
            0
        } else if self.refuted {
            u64::MAX
        } else {
            self.replies
                .iter()
                .filter(|l| l.status == LeafStatus::Open)
                .map(|l| l.pn)
                .sum()
        }
    }
}

/// PNS-style leaf selection: min OR-level pn over unresolved children, then
/// min dn over open unlocked leaves, static rank as the deterministic prior,
/// same-worker affinity preferred for TT reuse.
pub fn select_leaf(children: &[Child], worker: usize) -> Option<(usize, usize)> {
    let mut best: Option<(u64, u64, u64, u64, usize, usize)> = None;
    for (ci, child) in children.iter().enumerate() {
        if child.resolved || child.refuted {
            continue;
        }
        let cpn = child.child_pn();
        if cpn == u64::MAX {
            continue;
        }
        for (li, leaf) in child.replies.iter().enumerate() {
            if leaf.status != LeafStatus::Open || leaf.job.is_some() {
                continue;
            }
            let affinity = u64::from(leaf.last_worker != Some(worker));
            let key = (cpn, affinity, leaf.dn, leaf.static_rank as u64, ci, li);
            if best.is_none() || key < best.unwrap() {
                best = Some(key);
            }
        }
    }
    best.map(|(_, _, _, _, ci, li)| (ci, li))
}

pub fn find_job_loc(children: &mut [Child], job_id: &str) -> Option<(usize, usize)> {
    for (ci, child) in children.iter().enumerate() {
        for (li, leaf) in child.replies.iter().enumerate() {
            if leaf.job.as_ref().is_some_and(|(id, _, _)| id == job_id) {
                return Some((ci, li));
            }
        }
    }
    None
}

pub fn pending_jobs(children: &[Child]) -> usize {
    children
        .iter()
        .flat_map(|c| c.replies.iter())
        .filter(|l| l.job.is_some())
        .count()
}

// ----------------------------------------------------------------- dumps ---

pub fn dump_tree_json(tree: &ProofTree, path: &str) -> Result<(), String> {
    let mut out: Vec<EventRec> = Vec::new();
    let mut stack: Vec<(usize, Vec<String>)> = vec![(0, Vec::new())];
    while let Some((id, node_path)) = stack.pop() {
        let node = &tree.nodes[id];
        if let Some(outcome) = node.outcome {
            out.push(EventRec {
                path: node_path.clone(),
                outcome: outcome.as_str().to_string(),
                depth: node.depth,
            });
            for child in tree.children(id) {
                let mut cp = node_path.clone();
                cp.push(move_to_uci(tree.nodes[child].mv));
                stack.push((child, cp));
            }
        }
    }
    super::write_json(path, &out)
}

pub fn dump_state(args: &Args, children: &[Child], jobs: u64, per_worker: &HashMap<usize, u64>) {
    let state = serde_json::json!({
        "jobs_dispatched": jobs,
        "children": children.iter().map(|c| serde_json::json!({
            "mv": move_to_uci(c.mv),
            "static_rank": c.static_rank,
            "terminal": c.terminal.map(|o| o.as_str()),
            "resolved": c.resolved,
            "refuted": c.refuted,
            "replies": c.replies.iter().map(|l| serde_json::json!({
                "mv": move_to_uci(l.mv),
                "status": format!("{:?}", l.status),
                "pn": l.pn, "dn": l.dn, "work": l.work, "slices": l.slices,
                "locked": l.job.is_some(),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "per_worker_child_evals": per_worker,
    });
    let _ = super::write_json(format!("{}/master_state.json", args.session), &state);
}

/// Build the master's initial AND/OR proof state: root children (OR nodes)
/// and their replies (AND leaves), each with rule-derived terminal
/// classification from the master's own global replay.
pub fn build_children(fen: &str) -> (u64, Vec<Child>) {
    let mut pos = Position::from_fen(fen).expect("valid FEN");
    let root_hash = pos.hash();
    let mut children: Vec<Child> = Vec::new();
    for (rank, mv) in pos.legal_moves_vec().into_iter().enumerate() {
        let mut state = StateInfo::new();
        let mut moves = MoveList::new();
        pos.do_move(mv);
        pos.legal_moves_with_state(&mut moves, &mut state);
        let child_hash = pos.hash();
        let child_terminal = pos.outcome_from_state(&state, &moves);
        let mut replies: Vec<Leaf> = Vec::new();
        if child_terminal.is_none() {
            for (lrank, rmv) in pos.legal_moves_vec().into_iter().enumerate() {
                let mut lstate = StateInfo::new();
                let mut lmoves = MoveList::new();
                pos.do_move(rmv);
                pos.legal_moves_with_state(&mut lmoves, &mut lstate);
                let leaf_terminal = pos.outcome_from_state(&lstate, &lmoves);
                // Us (root mover) to move at the leaf: a static Win is a
                // rule-derived resolved fact; Loss/Draw refutes the child.
                let (status, depth) = match leaf_terminal {
                    Some(Outcome::Win) => (LeafStatus::Won, 0),
                    Some(_) => (LeafStatus::Lost, 0),
                    None => (LeafStatus::Open, 0),
                };
                pos.undo_move(rmv);
                replies.push(Leaf {
                    mv: rmv,
                    status,
                    depth,
                    pn: 1,
                    dn: 1,
                    work: 0,
                    slices: 0,
                    last_worker: None,
                    job: None,
                    static_rank: lrank,
                });
            }
        }
        pos.undo_move(mv);
        let mut child = Child {
            mv,
            terminal: child_terminal,
            hash: child_hash,
            replies,
            resolved: false,
            refuted: false,
            depth: 0,
            synthesized: false,
            static_rank: rank,
        };
        child.refresh();
        children.push(child);
    }
    (root_hash, children)
}

#[allow(clippy::too_many_arguments)]
pub fn finish(
    args: &Args,
    children: Vec<Child>,
    pt_tx: std::sync::mpsc::Sender<ProofEvent>,
    pt: ProofTreeWorkerHandle,
    pt_join: std::thread::JoinHandle<()>,
    per_worker: HashMap<usize, u64>,
    jobs_dispatched: u64,
    jobs_completed: u64,
    job_errors: u64,
    verify_failures: u64,
    worker_nodes: u64,
    worker_child_evals: u64,
    root_outcome: Option<Outcome>,
    exit_reason: &str,
    t0: Instant,
    children_total: usize,
) -> i32 {
    let mut summary = Summary {
        mode: "campaign".into(),
        fen: args.fen.clone(),
        workers: args.workers,
        feedback: !args.nf,
        exit: exit_reason.to_string(),
        outcome: root_outcome.map(|o| o.as_str().to_string()),
        wall_s: t0.elapsed().as_secs_f64(),
        jobs_dispatched,
        jobs_completed,
        job_errors,
        verify_failures,
        worker_nodes,
        worker_child_evals,
        children_total,
        children_resolved: children.iter().filter(|c| c.resolved).count(),
        children_refuted: children.iter().filter(|c| c.refuted).count(),
        leaves_open: children
            .iter()
            .flat_map(|c| c.replies.iter())
            .filter(|l| l.status == LeafStatus::Open)
            .count(),
        leaves_won: children
            .iter()
            .flat_map(|c| c.replies.iter())
            .filter(|l| l.status == LeafStatus::Won)
            .count(),
        leaves_lost: children
            .iter()
            .flat_map(|c| c.replies.iter())
            .filter(|l| l.status == LeafStatus::Lost)
            .count(),
        tree_nodes: None,
        tree_root_depth: None,
        validate: None,
        per_worker_child_evals: per_worker.clone(),
    };

    let mut code = 1;
    if exit_reason == "verify_failed" {
        code = 3;
    }
    if root_outcome == Some(Outcome::Win) {
        // Root synthesis: rule-derived OR resolution over the verified child.
        let best = children
            .iter()
            .filter(|c| c.resolved)
            .min_by_key(|c| c.depth)
            .expect("resolved child exists");
        let _ = pt
            .event_sender()
            .send(ProofEvent::NodeProven(NodeProven::new(
                Vec::new(),
                root_hash_of(args),
                Outcome::Win,
                best.depth + 1,
            )));
        pt.finalize();
        let stats = pt.stats();
        let tree = pt.tree();
        let validate = match validate_proof_tree(&tree) {
            Ok(()) => "ok".to_string(),
            Err(defects) => {
                for defect in defects.iter().take(10) {
                    eprintln!("pt_validate: FAILED {defect}");
                }
                format!("FAILED {}", defects.len())
            }
        };
        if let Err(e) = std::fs::File::create(&args.out).and_then(|mut f| tree.to_bin(&mut f)) {
            eprintln!("master: cannot write tree dump: {e}");
        }
        if !args.tree_json.is_empty()
            && let Err(e) = dump_tree_json(&tree, &args.tree_json)
        {
            eprintln!("master: cannot write tree json: {e}");
        }
        summary.tree_nodes = Some(stats.nodes);
        summary.tree_root_depth = Some(tree.nodes[0].depth);
        summary.validate = Some(validate.clone());
        code = if validate == "ok" { 0 } else { 2 };
    }
    // Disconnect the event channel BEFORE joining: the proof-tree worker
    // thread exits only when every sender is dropped.
    drop(pt_tx);
    drop(pt);
    let _ = pt_join.join();
    dump_state(args, &children, jobs_dispatched, &per_worker);
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    let _ = super::write_json(format!("{}/summary.json", args.session), &summary);
    code
}

pub fn root_hash_of(args: &Args) -> u64 {
    Position::from_fen(&args.fen).map(|p| p.hash()).unwrap_or(0)
}

/// Session configuration + run-tunable CLI state for one master process.
pub struct Args {
    pub session: String,
    pub fen: String,
    pub workers: usize,
    pub tt_mb: usize,
    pub pt_mb: usize,
    pub slice: u64,
    pub max_slice: u64,
    pub nf: bool,
    pub max_wall: u64,
    pub mode: String,
    pub out: String,
    pub tree_json: String,
    pub state_every: u64,
    pub seq_timeout: u64,
}

/// Per-session summary printed as JSON at exit (and written to
/// `summary.json` in the session directory).
#[derive(serde::Serialize)]
pub struct Summary {
    pub mode: String,
    pub fen: String,
    pub workers: usize,
    pub feedback: bool,
    pub exit: String,
    pub outcome: Option<String>,
    pub wall_s: f64,
    pub jobs_dispatched: u64,
    pub jobs_completed: u64,
    pub job_errors: u64,
    pub verify_failures: u64,
    pub worker_nodes: u64,
    pub worker_child_evals: u64,
    pub children_total: usize,
    pub children_resolved: usize,
    pub children_refuted: usize,
    pub leaves_open: usize,
    pub leaves_won: usize,
    pub leaves_lost: usize,
    pub tree_nodes: Option<usize>,
    pub tree_root_depth: Option<u32>,
    pub validate: Option<String>,
    pub per_worker_child_evals: HashMap<usize, u64>,
}

// ------------------------------------------------------- dispatch loop ---

/// The master's main loop: drain durable results, verify + merge decisive
/// facts, dispatch idle workers (pseudo-MPN selection + locked leaves),
/// checkpoint, and stop on proof / refutation / wall cap.
#[allow(clippy::too_many_lines)]
pub fn run_campaign(args: &Args) -> i32 {
    let t0 = Instant::now();
    for dir in [JOBS_DIR, RESULTS_DIR] {
        std::fs::create_dir_all(format!("{}/{}", args.session, dir)).expect("session dirs");
    }
    let config = super::SessionConfig {
        root_fen: args.fen.clone(),
        tt_mb: args.tt_mb,
        slice_budget: args.slice,
        max_slice: args.max_slice,
        feedback: !args.nf,
    };
    super::write_json(format!("{}/{}", args.session, SESSION_FILE), &config)
        .expect("session config");

    let (root_hash, mut children) = build_children(&args.fen);
    let _ = root_hash;
    let children_total = children.len();

    // Master proof tree (the global artifact).
    let memory_limited = Arc::new(AtomicBool::new(false));
    let (pt, pt_join) =
        ProofTreeWorkerHandle::spawn(args.fen.clone(), args.pt_mb, Arc::clone(&memory_limited));
    let pt_tx = pt.event_sender();

    let mut jobs_dispatched: u64 = 0;
    let mut processed: Vec<String> = Vec::new();
    let mut per_worker: HashMap<usize, u64> = HashMap::new();
    let mut jobs_completed = 0u64;
    let mut job_errors = 0u64;
    let mut verify_failures = 0u64;
    let mut worker_nodes = 0u64;
    let mut worker_child_evals = 0u64;
    let mut root_outcome: Option<Outcome> = None;
    let mut exit_reason = "timeout".to_string();
    let mut last_state_dump = Instant::now();

    // Rule-derived terminal-child facts are emitted immediately.
    for child in &mut children {
        if child.terminal == Some(Outcome::Loss) {
            child.synthesized = true;
            let _ = pt_tx.send(ProofEvent::NodeProven(NodeProven::new(
                vec![child.mv],
                child.hash,
                Outcome::Loss,
                0,
            )));
        }
    }

    loop {
        // 1. Drain durable results (at-least-once; each processed once).
        let results_dir = format!("{}/{}", args.session, RESULTS_DIR);
        let mut names: Vec<String> = std::fs::read_dir(&results_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".json"))
            .collect();
        names.sort();
        let mut progressed = false;
        for name in names {
            if processed.contains(&name) {
                continue;
            }
            processed.push(name.clone());
            let full = format!("{results_dir}/{name}");
            let result: super::JobResult = match super::read_json(&full) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("master: bad result file {name}: {e}");
                    job_errors += 1;
                    continue;
                }
            };
            progressed = true;
            jobs_completed += 1;
            worker_nodes += result.nodes;
            worker_child_evals += result.child_evals;
            let worker = result
                .job_id
                .split('_')
                .next()
                .and_then(|w| w.strip_prefix('w'))
                .and_then(|w| w.parse::<usize>().ok())
                .unwrap_or(usize::MAX);
            *per_worker.entry(worker).or_insert(0) += result.child_evals;
            let Some((ci, li)) = find_job_loc(&mut children, &result.job_id) else {
                continue;
            };
            let job_path_uci: Vec<String> = vec![
                move_to_uci(children[ci].mv),
                move_to_uci(children[ci].replies[li].mv),
            ];
            let leaf = &mut children[ci].replies[li];
            leaf.job = None;
            if let Some(err) = &result.error {
                job_errors += 1;
                eprintln!("master: job {} errored: {err}", result.job_id);
                continue;
            }
            leaf.work += result.child_evals;
            match result.outcome.as_str() {
                "win" | "loss" => {
                    let mut emit = |global: &[Move], hash: u64, outcome: Outcome, depth: u32| {
                        let _ = pt_tx.send(ProofEvent::NodeProven(NodeProven::new(
                            global.to_vec(),
                            hash,
                            outcome,
                            depth,
                        )));
                    };
                    let verify = super::verify::verify_and_merge(
                        &args.fen,
                        &job_path_uci,
                        &result.events,
                        &mut emit,
                    );
                    match verify {
                        Ok((outcome, depth)) => {
                            let leaf = &mut children[ci].replies[li];
                            if outcome == Outcome::Win {
                                leaf.status = LeafStatus::Won;
                                leaf.depth = depth;
                            } else {
                                leaf.status = LeafStatus::Lost;
                            }
                        }
                        Err(e) => {
                            verify_failures += 1;
                            eprintln!("master: VERIFY FAILED for job {}: {e}", result.job_id);
                            // Reject the whole result and abort (architecture
                            // doc §3: rejected, never patched).
                            exit_reason = "verify_failed".to_string();
                            return finish(
                                args,
                                children,
                                pt_tx,
                                pt,
                                pt_join,
                                per_worker,
                                jobs_dispatched,
                                jobs_completed,
                                job_errors,
                                verify_failures,
                                worker_nodes,
                                worker_child_evals,
                                None,
                                &exit_reason,
                                t0,
                                children_total,
                            );
                        }
                    }
                }
                "draw" => {
                    // Advisory update only (never trusted for composition).
                    let leaf = &mut children[ci].replies[li];
                    if result.root_pn > 0 {
                        leaf.pn = result.root_pn;
                    }
                    if result.root_dn > 0 {
                        leaf.dn = result.root_dn;
                    }
                }
                other => {
                    job_errors += 1;
                    eprintln!("master: job {} unknown outcome {other}", result.job_id);
                }
            }
            // AND-resolution synthesis over verified leaves.
            let child = &mut children[ci];
            child.refresh();
            if child.resolved && !child.synthesized {
                child.synthesized = true;
                let _ = pt_tx.send(ProofEvent::NodeProven(NodeProven::new(
                    vec![child.mv],
                    child.hash,
                    Outcome::Loss,
                    child.depth,
                )));
            }
            if child.resolved && root_outcome.is_none() {
                root_outcome = Some(Outcome::Win);
                exit_reason = "proven".to_string();
            }
        }

        if root_outcome.is_some() {
            break;
        }
        if children.iter().all(|c| c.refuted) {
            exit_reason = "refuted_all".to_string();
            break;
        }

        // 2. Dispatch idle workers. Release stale locks first (worker death).
        for child in &mut children {
            for leaf in &mut child.replies {
                if let Some((_, _, since)) = &leaf.job
                    && since.elapsed() > Duration::from_secs(args.max_wall.max(900))
                {
                    eprintln!(
                        "master: releasing stale job lock on {}",
                        move_to_uci(leaf.mv)
                    );
                    leaf.job = None;
                }
            }
        }
        let mut dispatched = false;
        for w in 0..args.workers {
            let busy = children
                .iter()
                .flat_map(|c| c.replies.iter())
                .filter_map(|l| l.job.as_ref())
                .any(|(_, jw, _)| *jw == w);
            if busy {
                continue;
            }
            if let Some((ci, li)) = select_leaf(&children, w) {
                let (job_path, budget) = {
                    let child = &children[ci];
                    let leaf = &child.replies[li];
                    let path = vec![move_to_uci(child.mv), move_to_uci(leaf.mv)];
                    let budget = if args.nf {
                        args.max_slice
                    } else {
                        args.slice
                            .saturating_mul(1u64 << leaf.slices.min(20))
                            .min(args.max_slice)
                    };
                    (path, budget)
                };
                let job_id = format!("w{w}_{jobs_dispatched}");
                jobs_dispatched += 1;
                dispatched = true;
                let job = super::Job {
                    job_id: job_id.clone(),
                    worker: w,
                    path_uci: job_path,
                    direction: "evaluate".into(),
                    budget_evals: budget,
                };
                let path = format!("{}/{}/{job_id}.json", args.session, JOBS_DIR);
                if let Err(e) = super::write_json(&path, &job) {
                    eprintln!("master: cannot write job: {e}");
                    continue;
                }
                let leaf = &mut children[ci].replies[li];
                leaf.job = Some((job_id, w, Instant::now()));
                leaf.slices += 1;
                leaf.last_worker = Some(w);
            }
        }

        // 3. Checkpoint + wall clock.
        if last_state_dump.elapsed() > Duration::from_secs(args.state_every) {
            dump_state(args, &children, jobs_dispatched, &per_worker);
            last_state_dump = Instant::now();
        }
        if t0.elapsed() > Duration::from_secs(args.max_wall) {
            exit_reason = "timeout".to_string();
            break;
        }
        if !dispatched && !progressed {
            std::thread::sleep(Duration::from_millis(3));
        }
    }

    let _ = std::fs::write(format!("{}/{}", args.session, STOP_FILE), b"stop");
    if root_outcome.is_some() {
        for _ in 0..100 {
            if pending_jobs(&children) == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    finish(
        args,
        children,
        pt_tx,
        pt,
        pt_join,
        per_worker,
        jobs_dispatched,
        jobs_completed,
        job_errors,
        verify_failures,
        worker_nodes,
        worker_child_evals,
        root_outcome,
        &exit_reason,
        t0,
        children_total,
    )
}
