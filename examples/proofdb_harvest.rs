//! `proofdb_harvest` — the harvest loop of the startpos proof-line database
//! (proofdb initiative, backlog items 2 and 4).
//!
//! Per job (one frontier position of the current DB, selected by the
//! `--policy` coverage policy over the full frontier): replay the full
//! startpos→node path (context contract — repetition verdicts are
//! path-dependent), run `Search::search_depth_with_prefix` under the job's
//! policy-resolved child-eval budget, and for a decisive outcome export a
//! validator-clean proof subtree via the product's offline pipeline (TT
//! snapshot → `reconstruct` → `validate_proof_tree`), writing the shard
//! into the standing shard directory and recording its manifest entry.
//! `Draw` from any cause is **censored** (no fact, no write). The merger
//! runs after the batch as a separate tool — this CLI never merges and
//! never writes a DB; `--out-db` / `--dump` are the caller's
//! `proofdb_merge` output targets, recorded in the summary only.
//!
//! Module map: session shape and the job pipeline in
//! `examples/proofdb/session.rs`; the batch driver (screen pass, budget
//! cap, heavy-tier placement, stop conditions) in `examples/proofdb/batch.rs`;
//! the DB-facing engine (frontier-class extraction, AND-completeness assert,
//! decision-3 disjointness) in `examples/proofdb/frontier.rs` + `db.rs`;
//! the policies and per-class budgets in `examples/proofdb/policy.rs`.
//!
//! Coverage policies (`--policy`; default `sharp-siblings`, the plan3 A/B
//! winner per pre-registered decision 7 — pass `open-deepest` to reproduce
//! plan2's behavior): `open-deepest` (C1 only, deepest-first),
//! `sharp-siblings` (C2 at 1M evals each, then C3, then C1 at 4M),
//! `sharp-heavy-tail` (same, plus a C2-only censored-tail sample at the
//! heavy budget where the C2 screen ends — see `batch.rs`).
//!
//! Output: one `job: {…}` JSON line per job (census input; fields include
//! `policy`, `class`, `parent_bound`), then `manifest:`/`harvest:` summary
//! lines. On any defect (extraction violation, export/validation failure,
//! manifest collision) the session aborts non-zero before the manifest
//! rewrite — nothing inconsistent enters the durable layer (shard files of
//! jobs completed before the abort are unreferenced orphans and are
//! deterministically overwritten by the retry).

mod proofdb;

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use proofdb::batch::{BatchOptions, run_batch};
use proofdb::frontier::extract_frontier;
use proofdb::policy::{Policy, jobs_for_policy};
use proofdb::session::{Session, fail};

struct Args {
    db: PathBuf,
    manifest: PathBuf,
    shard_dir: PathBuf,
    policy: Policy,
    budget_evals: u64,
    max_total_evals: u64,
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
         --shard-dir <dir> [--policy <open-deepest|sharp-siblings|sharp-heavy-tail>] \
         [--budget-evals <n>] [--max-total-evals <n>] [--heavy-budget-evals <n>] \
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
        policy: Policy::SharpSiblings,
        budget_evals: 0,
        max_total_evals: 0,
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
            "--policy" => {
                a.policy = Policy::parse(&next()).unwrap_or_else(|e| {
                    eprintln!("proofdb_harvest: {e}");
                    usage()
                })
            }
            "--budget-evals" => a.budget_evals = next().parse().unwrap_or_else(|_| usage()),
            "--max-total-evals" => a.max_total_evals = next().parse().unwrap_or_else(|_| usage()),
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
    // Decision 3: frontier paths must be neither DB rows nor manifest paths;
    // the extractor asserts both while building every class.
    let manifest_paths: HashSet<String> =
        manifest.entries.iter().map(|e| e.moves.join(" ")).collect();
    let frontier = extract_frontier(&args.db, &manifest.sha256_hex, &manifest_paths)
        .unwrap_or_else(|e| fail(&e));
    let mut jobs = jobs_for_policy(args.policy, &frontier);
    // Optional screen-budget override (probe knob; 0 = policy-resolved).
    if args.budget_evals > 0 {
        for j in &mut jobs {
            j.budget = args.budget_evals;
        }
    }
    eprintln!(
        "harvest: db {} ({} nodes, built_from {}) — policy {} over frontier \
         C1 {} / C2 {} / C3 {} (AND-checks {}), {} jobs",
        args.db.display(),
        frontier.n_nodes,
        &manifest.sha256_hex[..16],
        args.policy.as_str(),
        frontier.c1.len(),
        frontier.c2.len(),
        frontier.c3.len(),
        frontier.and_checks,
        jobs.len(),
    );

    let mut session = Session::new(
        args.tt_mb,
        args.shard_dir.clone(),
        manifest.entries.clone(),
        args.policy.as_str(),
    );
    let opts = BatchOptions {
        heavy_tail_is_c2: args.policy == Policy::SharpHeavyTail,
        max_total_evals: args.max_total_evals,
        heavy_budget_evals: args.heavy_budget_evals,
        heavy_sample: args.heavy_sample,
        max_jobs: args.max_jobs,
        max_runtime: args.max_runtime,
        stop_file: args.stop_file.clone(),
    };
    let summary = run_batch(&mut session, &jobs, &opts);

    session.rewrite_manifest(&args.manifest, &manifest.sha256_hex);
    println!(
        "harvest: policy {} stop={} screen_jobs {} screen_decisive {} \
         screen_evals {} heavy_jobs {} heavy_decisive {} new_shards {} wall {:.1}s",
        args.policy.as_str(),
        if summary.stop_reason.is_empty() {
            "exhausted"
        } else {
            &summary.stop_reason
        },
        summary.screen_jobs,
        summary.screen_decisive,
        summary.screen_evals,
        summary.heavy_jobs,
        summary.heavy_decisive,
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
