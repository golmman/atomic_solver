//! `proofdb_harvest` — the harvest loop of the startpos proof-line database
//! (proofdb initiative, backlog item 2, plan2).
//!
//! Per job (one open node of the current DB, deepest-first): replay the full
//! startpos→node path (context contract — repetition verdicts are
//! path-dependent), run `Search::search_depth_with_prefix` under the
//! deterministic `--budget-evals` child-eval budget, and for a decisive
//! outcome export a validator-clean proof subtree via the product's offline
//! pipeline (TT snapshot → `reconstruct` → `validate_proof_tree`), write the
//! shard into the standing shard directory and record its manifest entry.
//! `Draw` from any cause is **censored** (no fact, no write). The merger
//! runs after the batch as a separate tool — this CLI never merges and never
//! writes a DB; `--out-db` / `--dump` are the caller's `proofdb_merge`
//! output targets, recorded in the summary only.
//!
//! Session shape and the heavy tier are documented in
//! `examples/proofdb/session.rs`; the DB-facing engine (frontier extraction,
//! job order, replay-prefix contract) in `examples/proofdb/harvest.rs`.
//!
//! Heavy tier (`--heavy-sample N`, `--heavy-budget-evals`): after the screen
//! pass, the first N censored jobs *in job order* are re-run at the heavy
//! budget as a pre-registered censored-tail sample (measures the plateau's
//! cost curve; any decision there is a shard like any other). Jobs not
//! reached by the screen pass are not sampled.
//!
//! Stop conditions are checked **between jobs only** (`--max-jobs`,
//! `--max-runtime`, `--stop-file`; 0 = unlimited for jobs/runtime): a job is
//! never abandoned mid-search — budgets are small enough that losing one
//! interrupted job's work is cheaper than nondeterministic interruption.
//!
//! Output: one `job: {…}` JSON line per job (census input), then
//! `manifest:`/`harvest:` summary lines. On any defect (export/validation
//! failure, manifest collision) the session aborts non-zero before the
//! manifest rewrite — nothing inconsistent enters the durable layer (shard
//! files of jobs completed before the abort are unreferenced orphans and are
//! deterministically overwritten by the retry).

mod proofdb;

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use proofdb::harvest::extract_jobs;
use proofdb::session::{Session, fail};

struct Args {
    db: PathBuf,
    manifest: PathBuf,
    shard_dir: PathBuf,
    budget_evals: u64,
    heavy_budget_evals: u64,
    heavy_sample: usize,
    tt_mb: usize,
    max_jobs: usize,
    max_runtime: u64,
    stop_file: PathBuf,
    out_db: Option<PathBuf>,
    dump: Option<PathBuf>,
}

fn usage() -> ! {
    eprintln!(
        "usage: proofdb_harvest --db <proofdb.db> --manifest <manifest.json> \
         --shard-dir <dir> [--budget-evals <n>] [--heavy-budget-evals <n>] \
         [--heavy-sample <n>] [--tt-mb <mb>] [--max-jobs <n>] [--max-runtime <s>] \
         [--stop-file <path>] [--out-db <grown.db>] [--dump <nodes.txt>]"
    );
    std::process::exit(1);
}

fn parse_args() -> Args {
    let mut a = Args {
        db: PathBuf::new(),
        manifest: PathBuf::new(),
        shard_dir: PathBuf::new(),
        budget_evals: 4_000_000,
        heavy_budget_evals: 40_000_000,
        heavy_sample: 5,
        tt_mb: 128,
        max_jobs: 0,
        max_runtime: 0,
        stop_file: PathBuf::from("STOP"),
        out_db: None,
        dump: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut next = || it.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "--db" => a.db = PathBuf::from(next()),
            "--manifest" => a.manifest = PathBuf::from(next()),
            "--shard-dir" => a.shard_dir = PathBuf::from(next()),
            "--budget-evals" => a.budget_evals = next().parse().unwrap_or_else(|_| usage()),
            "--heavy-budget-evals" => {
                a.heavy_budget_evals = next().parse().unwrap_or_else(|_| usage())
            }
            "--heavy-sample" => a.heavy_sample = next().parse().unwrap_or_else(|_| usage()),
            "--tt-mb" => a.tt_mb = next().parse().unwrap_or_else(|_| usage()),
            "--max-jobs" => a.max_jobs = next().parse().unwrap_or_else(|_| usage()),
            "--max-runtime" => a.max_runtime = next().parse().unwrap_or_else(|_| usage()),
            "--stop-file" => a.stop_file = PathBuf::from(next()),
            "--out-db" => a.out_db = Some(PathBuf::from(next())),
            "--dump" => a.dump = Some(PathBuf::from(next())),
            _ => usage(),
        }
    }
    if a.db.as_os_str().is_empty()
        || a.manifest.as_os_str().is_empty()
        || a.shard_dir.as_os_str().is_empty()
    {
        usage();
    }
    a
}

fn main() {
    let args = parse_args();
    let t_start = Instant::now();
    let manifest = proofdb::read_manifest(&args.manifest).unwrap_or_else(|e| fail(&e));
    let input = extract_jobs(&args.db, &manifest.sha256_hex).unwrap_or_else(|e| fail(&e));
    // H1 assert: job paths are disjoint from the manifest's shard paths and
    // from the DB's proven paths.
    let known_paths: HashSet<String> = manifest.entries.iter().map(|e| e.moves.join(" ")).collect();
    for job in &input.jobs {
        let p = job.path.join(" ");
        if known_paths.contains(&p) {
            fail(&format!(
                "open node {p:?} already has a shard (stale frontier?)"
            ));
        }
        if input.proven_paths.contains(&p) {
            fail(&format!("open node {p:?} is also a proven DB path"));
        }
    }
    eprintln!(
        "harvest: db {} ({} nodes, built_from {}) — {} open jobs, order deepest-first",
        args.db.display(),
        input.n_nodes,
        &manifest.sha256_hex[..16],
        input.jobs.len()
    );

    let mut session = Session::new(args.tt_mb, args.shard_dir.clone(), manifest.entries.clone());

    let stopped = |completed: usize, t: &Instant| -> Option<String> {
        if args.max_jobs > 0 && completed >= args.max_jobs {
            return Some("max-jobs".to_string());
        }
        if args.max_runtime > 0 && t.elapsed().as_secs() >= args.max_runtime {
            return Some("max-runtime".to_string());
        }
        if args.stop_file.as_os_str().is_empty() {
            return None;
        }
        if std::path::Path::new(&args.stop_file).exists() {
            return Some("stop-file".to_string());
        }
        None
    };

    // Screen pass over all open jobs, then stop-condition bookkeeping.
    let mut completed = 0usize;
    let mut screen_jobs = 0usize;
    let mut screen_decisive = 0usize;
    let mut stop_reason = String::new();
    for job in &input.jobs {
        if let Some(reason) = stopped(completed, &t_start) {
            stop_reason = reason;
            break;
        }
        let rec = session.run_job(job, "screen", args.budget_evals);
        if rec.outcome != "censored" {
            screen_decisive += 1;
        }
        rec.emit();
        completed += 1;
        screen_jobs += 1;
    }

    // Heavy tier: first N censored jobs (in job order) re-run at the heavy
    // budget (pre-registered censored-tail sample). Jobs not reached by the
    // screen pass are not sampled.
    let mut heavy_jobs = 0usize;
    let mut heavy_decisive = 0usize;
    if args.heavy_sample > 0 && args.heavy_budget_evals > 0 && stop_reason.is_empty() {
        for job in input.jobs.iter().take(screen_jobs) {
            if heavy_jobs >= args.heavy_sample {
                break;
            }
            // A censored screen job: no shard exists for its path.
            if session.has_path(&job.path.join(" ")) {
                continue; // decided during the screen pass
            }
            if let Some(reason) = stopped(completed, &t_start) {
                stop_reason = reason;
                break;
            }
            let rec = session.run_job(job, "heavy", args.heavy_budget_evals);
            if rec.outcome != "censored" {
                heavy_decisive += 1;
            }
            rec.emit();
            completed += 1;
            heavy_jobs += 1;
        }
    }

    session.rewrite_manifest(&args.manifest, &manifest.sha256_hex);
    println!(
        "harvest: stop={} screen_jobs {screen_jobs} screen_decisive \
         {screen_decisive} heavy_jobs {heavy_jobs} heavy_decisive \
         {heavy_decisive} new_shards {} wall {:.1}s",
        if stop_reason.is_empty() {
            "exhausted"
        } else {
            &stop_reason
        },
        session.new_shards,
        t_start.elapsed().as_secs_f64(),
    );
    if let Some(p) = &args.out_db {
        println!(
            "out-db: {} (caller's proofdb_merge --db target)",
            p.display()
        );
    }
    if let Some(p) = &args.dump {
        println!(
            "dump: {} (caller's proofdb_merge --dump target)",
            p.display()
        );
    }
}
