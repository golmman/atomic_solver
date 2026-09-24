//! Persistent campaign worker (plan5 D2 prototype).
//!
//! One process = one in-process `Search` instance with a private TT retained
//! across jobs for the whole session (architecture doc §5: retention is a
//! performance contract; the GHI journal contract binds within the worker
//! exactly as in the sequential solver, unchanged semantics).
//!
//! Per job: replay the job path from the campaign root (context contract),
//! run `search_depth_with_prefix` under the job's deterministic child-eval
//! budget, and — for decisive outcomes — export a validator-clean proof
//! subtree via the product's own offline pipeline (TT snapshot →
//! `reconstruct` → `validate_proof_tree`): with a retained TT a raw DF-PN
//! event stream is not always self-contained (holes at TT-resolved nodes
//! proven under earlier jobs), so only a reconstruction-verified subtree
//! crosses the boundary — architecture doc §2/§3.
//!
//! Usage:
//!     campaign_worker --session <dir> --worker 0 [--tt-mb 128]
//!                     [--retention on|off] [--poll-ms 5] [--max-runtime 3600]

mod campaign;

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Outcome;
use atomic_solver::proof_tree::validate_proof_tree;
use atomic_solver::reconstruct::{ReconstructConfig, reconstruct};
use atomic_solver::search::dfpn::Search;
use atomic_solver::tt_snapshot::{read_tt_snapshot, write_tt_snapshot};
use campaign::{EventRec, JOBS_DIR, Job, JobResult, RESULTS_DIR, SESSION_FILE, STOP_FILE};
use std::io::Cursor;
use std::path::Path;
use std::time::{Duration, Instant};

struct Args {
    session: String,
    worker: usize,
    tt_mb: usize,
    retention: bool,
    poll_ms: u64,
    max_runtime: u64,
    pt_mb: usize,
}

fn parse_args() -> Args {
    let mut a = Args {
        session: ".".into(),
        worker: 0,
        tt_mb: 128,
        retention: true,
        poll_ms: 5,
        max_runtime: 24 * 3600,
        pt_mb: 256,
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let need = |i: &mut usize, name: &str| -> String {
            let v = argv
                .get(*i + 1)
                .unwrap_or_else(|| panic!("{name} needs an argument"))
                .clone();
            *i += 2;
            v
        };
        match argv[i].as_str() {
            "--session" => a.session = need(&mut i, "--session"),
            "--worker" => a.worker = need(&mut i, "--worker").parse().expect("--worker number"),
            "--tt-mb" => a.tt_mb = need(&mut i, "--tt-mb").parse().expect("--tt-mb number"),
            "--retention" => a.retention = need(&mut i, "--retention") != "off",
            "--poll-ms" => a.poll_ms = need(&mut i, "--poll-ms").parse().expect("poll-ms number"),
            "--max-runtime" => a.max_runtime = need(&mut i, "--max-runtime").parse().expect("secs"),
            "--pt-mb" => a.pt_mb = need(&mut i, "--pt-mb").parse().expect("pt-mb number"),
            other => panic!("unknown option '{other}'"),
        }
    }
    a
}

/// Claim the next pending job for this worker via an atomic rename, so the
/// master (or a restarted worker) can safely see the claim.
fn claim_job(session: &str, worker: usize) -> Option<Job> {
    let jobs_dir = format!("{session}/{JOBS_DIR}");
    let prefix = format!("w{worker}_");
    let entries = std::fs::read_dir(&jobs_dir).ok()?;
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&prefix) && n.ends_with(".json"))
        .collect();
    names.sort();
    for name in names {
        let from = format!("{jobs_dir}/{name}");
        let claim = format!("{jobs_dir}/{}.claim", name.trim_end_matches(".json"));
        if std::fs::rename(Path::new(&from), Path::new(&claim)).is_ok() {
            match campaign::read_json::<Job, _>(&claim) {
                Ok(job) => return Some(job),
                Err(e) => eprintln!("worker: unparseable job file {name}: {e}"),
            }
        }
    }
    None
}

/// Export the worker-decisive subtree via the product's offline pipeline:
/// TT snapshot → `reconstruct` (deterministic hole-filling over the solved
/// map) → replay validator → DFS event list. Returns the events plus the
/// reconstruction's own work counters (fill child-evals, walk nodes), which
/// count toward the job's reported work.
fn export_via_reconstruction(
    search: &Search,
    sub_fen: &str,
    tt_mb: usize,
    pt_mb: usize,
) -> Result<(Vec<EventRec>, u64, u64), String> {
    let mut snap = Vec::new();
    write_tt_snapshot(
        search.tt(),
        sub_fen,
        tt_mb.min(u32::MAX as usize) as u32,
        &mut snap,
    )
    .map_err(|e| format!("snapshot write: {e}"))?;
    let (_header, solved, _unsolved) =
        read_tt_snapshot(&mut Cursor::new(&snap)).map_err(|e| format!("snapshot read: {e}"))?;
    let config = ReconstructConfig {
        tt_mb,
        pt_size_mb: pt_mb,
        fill_base: 8,
        fill_depth_cap: 32,
        fill_attempt_budget: 2_000_000,
        fill_total_budget: 100_000_000,
    };
    let out = reconstruct(sub_fen, &solved, &config);
    if out.root_outcome.is_none() {
        return Err(out
            .error
            .unwrap_or_else(|| "reconstruction did not resolve the root".to_string()));
    }
    let tree = out.tree.ok_or_else(|| {
        out.error
            .unwrap_or_else(|| "reconstruction produced no tree".to_string())
    })?;
    if let Err(defects) = validate_proof_tree(&tree) {
        return Err(format!(
            "replayed validation found {} defects",
            defects.len()
        ));
    }
    let mut out_events = Vec::new();
    let mut stack: Vec<(usize, Vec<String>)> = vec![(0, Vec::new())];
    while let Some((id, path)) = stack.pop() {
        let node = &tree.nodes[id];
        if let Some(outcome) = node.outcome {
            out_events.push(EventRec {
                path: path.clone(),
                outcome: outcome.as_str().to_string(),
                depth: node.depth,
            });
            for child in tree.children(id) {
                let mut child_path = path.clone();
                child_path.push(move_to_uci(tree.nodes[child].mv));
                stack.push((child, child_path));
            }
        }
    }
    Ok((out_events, out.fill_evals, out.stats.total()))
}

fn run_job(
    job: &Job,
    root_fen: &str,
    tt_mb: usize,
    retention: bool,
    retained: &mut Option<Search>,
    pt_mb: usize,
) -> JobResult {
    let t0 = Instant::now();
    let fail = |err: String, wall_s: f64| JobResult {
        job_id: job.job_id.clone(),
        path_uci: job.path_uci.clone(),
        outcome: "error".into(),
        exit_reason: "Error".into(),
        nodes: 0,
        child_evals: 0,
        wall_s,
        root_pn: 0,
        root_dn: 0,
        events: Vec::new(),
        error: Some(err),
    };

    let (mut pos, prefix) = match campaign::replay_path(root_fen, &job.path_uci) {
        Ok(v) => v,
        Err(e) => return fail(e, t0.elapsed().as_secs_f64()),
    };
    let sub_fen = pos.fen();

    // Retention contract (architecture doc §5): keep the Search object (and
    // its TT) across jobs; `--retention off` is the C2-nr ablation arm.
    if !retention || retained.is_none() {
        *retained = Some(Search::new(tt_mb));
    }
    let search = retained.as_mut().unwrap();
    search.set_timeout(3600); // safety net; the child-eval budget is binding
    search.set_child_eval_budget(job.budget_evals);

    let start = Instant::now();
    let (outcome, _win_depth, _nodes) =
        search.search_depth_with_prefix(&mut pos, u32::MAX, &prefix);
    let wall_s = start.elapsed().as_secs_f64();

    // Advisory job-root bounds (post-run TT entry; never trusted for
    // composition — architecture doc §2).
    let (root_pn, root_dn) = search
        .tt()
        .probe(pos.hash())
        .map(|e| e.advisory_pn_dn())
        .unwrap_or((0, 0));

    let exit_reason = search.exit_reason().to_string();
    let nodes = search.nodes();
    let child_evals = search.child_evaluations();

    // Export the worker-decisive result as a proof-event stream. With a
    // retained TT a decisive search can resolve nodes via entries proven
    // under earlier jobs (whose event streams were discarded), so a raw
    // event stream is not always self-contained. The export therefore runs
    // the product's own offline pipeline — TT snapshot → `reconstruct`
    // (deterministic hole-filling over the solved map) → replay validator —
    // and only a validator-clean subtree crosses the boundary (architecture
    // doc §2/§3). On failure the result downgrades to unresolved and the
    // worker resets its TT so the master's re-queue re-proves the leaf
    // self-contained. Reconstruction work counts toward the job's work.
    let mut recon_fills = 0u64;
    let mut recon_nodes = 0u64;
    let mut exported: Vec<EventRec> = Vec::new();
    let mut downgrade: Option<String> = None;
    if outcome != Outcome::Draw {
        match export_via_reconstruction(search, &sub_fen, tt_mb, pt_mb) {
            Ok((events, fills, rnodes)) => {
                exported = events;
                recon_fills = fills;
                recon_nodes = rnodes;
            }
            Err(reason) => {
                eprintln!(
                    "job {}: export failed ({}); downgrading to unresolved + TT reset",
                    job.job_id, reason
                );
                downgrade = Some(reason);
            }
        }
    }
    if let Some(reason) = downgrade {
        *retained = None; // fresh TT so the master's re-queue re-proves self-contained
        return JobResult {
            job_id: job.job_id.clone(),
            path_uci: job.path_uci.clone(),
            outcome: "draw".into(),
            exit_reason: "SubtreeDefect".into(),
            nodes,
            child_evals,
            wall_s,
            root_pn: 0,
            root_dn: 0,
            events: Vec::new(),
            error: Some(reason),
        };
    }
    let child_evals = child_evals + recon_fills;
    let nodes = nodes + recon_nodes;

    eprintln!(
        "job {} outcome={} exit={} evals={} nodes={} wall={wall_s:.3} events={}",
        job.job_id,
        outcome.as_str(),
        exit_reason,
        child_evals,
        nodes,
        exported.len(),
    );

    JobResult {
        job_id: job.job_id.clone(),
        path_uci: job.path_uci.clone(),
        outcome: outcome.as_str().to_string(),
        exit_reason,
        nodes,
        child_evals,
        wall_s,
        root_pn,
        root_dn,
        events: exported,
        error: None,
    }
}

fn main() {
    let args = parse_args();
    // The master writes session.json before its dispatch loop, but the
    // workers may be spawned concurrently: retry briefly on a missing file.
    let config: campaign::SessionConfig = {
        let path = format!("{}/{}", args.session, SESSION_FILE);
        let mut config = None;
        for _ in 0..100 {
            match campaign::read_json::<campaign::SessionConfig, _>(&path) {
                Ok(c) => {
                    config = Some(c);
                    break;
                }
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
        config.unwrap_or_else(|| panic!("cannot read session config at {path}"))
    };

    let mut retained: Option<Search> = None;
    let poll = Duration::from_millis(args.poll_ms);
    let started = Instant::now();

    eprintln!(
        "worker{}: tt_mb={} retention={} root={}",
        args.worker, args.tt_mb, args.retention, config.root_fen
    );
    loop {
        if std::path::Path::new(&format!("{}/{}", args.session, STOP_FILE)).exists() {
            break;
        }
        if started.elapsed() > Duration::from_secs(args.max_runtime) {
            eprintln!("worker{}: max runtime reached", args.worker);
            break;
        }
        let job = claim_job(&args.session, args.worker);
        match job {
            Some(job) => {
                // V3 cancellation (plan6 §2): a job the master abandoned at a
                // merge boundary is dropped un-run; the TT is kept (retention
                // semantics unchanged) and the worker picks up the next
                // dispatch. If the marker appears after the claim, the job
                // runs to completion and the master drops the result.
                let cancel = format!("{}/{}/{}.cancel", args.session, JOBS_DIR, job.job_id);
                if std::path::Path::new(&cancel).exists() {
                    eprintln!(
                        "worker{}: job {} cancelled before start; dropped",
                        args.worker, job.job_id
                    );
                    let claim = format!("{}/{}/{}.claim", args.session, JOBS_DIR, job.job_id);
                    let _ = std::fs::remove_file(&claim);
                    let _ = std::fs::remove_file(&cancel);
                    continue;
                }
                let result = run_job(
                    &job,
                    &config.root_fen,
                    args.tt_mb,
                    args.retention,
                    &mut retained,
                    args.pt_mb,
                );
                let path = format!("{}/{}/{}.json", args.session, RESULTS_DIR, result.job_id);
                if let Err(e) = campaign::write_json(&path, &result) {
                    eprintln!("worker{}: cannot write result: {e}", args.worker);
                }
                let claim = format!("{}/{}/{}.claim", args.session, JOBS_DIR, result.job_id);
                let _ = std::fs::remove_file(&claim);
            }
            None => std::thread::sleep(poll),
        }
    }
    eprintln!("worker{}: exiting", args.worker);
}
