//! `proofdb_harvest` — the harvest loop of the startpos proof-line database
//! (proofdb initiative; plan2/plan3/plan4).
//!
//! Per job (one frontier position of the current DB, selected by the
//! `--policy` coverage policy): replay the full startpos→node path (context
//! contract — repetition verdicts are path-dependent), run
//! `Search::search_depth_with_prefix` under the job's child-eval budget, and
//! for a decisive outcome export a validator-clean proof subtree via the
//! product's offline pipeline (TT snapshot → `reconstruct` →
//! `validate_proof_tree`), writing the shard into the standing shard
//! directory and recording its manifest entry. `Draw` from any cause is
//! **censored** (no fact, no write). The merger runs after the batch as a
//! separate tool — this CLI never merges and never writes a DB; `--out-db` /
//! `--dump` are the caller's `proofdb_merge` output targets, recorded in the
//! summary only.
//!
//! Module map (all under `examples/proofdb/`): the job pipeline in
//! `session.rs`, the batch drivers (legacy screen pass and the breadth-PNS
//! queue) in `batch.rs`, the DB-facing engine (frontier classes,
//! AND-completeness assert, disjointness) in `frontier.rs` + `db.rs`, the
//! legacy policies in `policy.rs`, the PNS selection in `pns.rs` + `pns/`,
//! and the sidecar work ledger in `ledger.rs`.
//!
//! Coverage policies (`--policy`; default `breadth-pns`, the plan4 pivot):
//! `breadth-pns` — live PNS priority queue over the open frontier, the
//! sidecar ledger at `--ledger` as pick-up state, per-visit budgets on the
//! geometric ladder `2^(k-1) × base` (base = `--budget-evals` or 4M),
//! session cap = `--max-total-evals`; plus the plan3 legacy gradients
//! `open-deepest`, `sharp-siblings` (the plan3 default), `sharp-heavy-tail`
//! (heavy options ignored under `breadth-pns`).
//!
//! Output: one `job: {…}` JSON line per job (fields include `policy`,
//! `class`, `parent_bound`, and for `breadth-pns` the `pass`/`number`/
//! `work_before` triple), then `manifest:`/`harvest:` summary lines. On any
//! defect the session aborts non-zero before the manifest rewrite — nothing
//! inconsistent enters the durable layer (shard files of jobs completed
//! before the abort are unreferenced orphans and are deterministically
//! overwritten by the retry).

mod proofdb;

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use proofdb::batch::{BatchOptions, PnsOptions, run_batch, run_pns_batch};
use proofdb::db::load_db_rows;
use proofdb::frontier::extract_frontier;
use proofdb::ledger::Ledger;
use proofdb::pns::{BUDGET_PNS_BASE_EVALS, Pns};
use proofdb::policy::{Policy, jobs_for_policy};
use proofdb::session::{Session, fail};

struct Args {
    db: PathBuf,
    manifest: PathBuf,
    shard_dir: PathBuf,
    policy: Policy,
    ledger: PathBuf,
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
         --shard-dir <dir> [--policy <name>] [--ledger <path>] [--budget-evals <n>] \
         [--max-total-evals <n>] [--heavy-budget-evals <n>] [--heavy-sample <n>] \
         [--tt-mb <mb>] [--max-jobs <n>] [--max-runtime <s>] [--stop-file <path>] \
         [--out-db <grown.db>] [--dump <nodes.txt>]  (policies: breadth-pns | \
         open-deepest | sharp-siblings | sharp-heavy-tail)"
    );
    std::process::exit(1);
}

fn parse_args() -> Args {
    let mut a = Args {
        db: PathBuf::new(),
        manifest: PathBuf::new(),
        shard_dir: PathBuf::new(),
        policy: Policy::BreadthPns,
        ledger: PathBuf::from("data/proofdb_work.json"),
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
            "--ledger" => a.ledger = PathBuf::from(next()),
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
    // Decision 3: frontier paths are neither DB rows nor manifest paths;
    // the extractor asserts both. Its AND-completeness walk is also the
    // standing extraction invariant the PNS numbers assume (plan3 finding 5).
    let manifest_paths: HashSet<String> =
        manifest.entries.iter().map(|e| e.moves.join(" ")).collect();
    let frontier = extract_frontier(&args.db, &manifest.sha256_hex, &manifest_paths)
        .unwrap_or_else(|e| fail(&e));
    eprintln!(
        "harvest: db {} ({} nodes, built_from {}) — policy {} over frontier \
         C1 {} / C2 {} / C3 {} (AND-checks {})",
        args.db.display(),
        frontier.n_nodes,
        &manifest.sha256_hex[..16],
        args.policy.as_str(),
        frontier.c1.len(),
        frontier.c2.len(),
        frontier.c3.len(),
        frontier.and_checks,
    );

    let mut session = Session::new(
        args.tt_mb,
        args.shard_dir.clone(),
        manifest.entries.clone(),
        args.policy.as_str(),
    );

    let summary_line = if args.policy == Policy::BreadthPns {
        let ledger = Ledger::load(&args.ledger).unwrap_or_else(|e| fail(&e));
        let db = load_db_rows(&args.db, &manifest.sha256_hex).unwrap_or_else(|e| fail(&e));
        let base = if args.budget_evals > 0 {
            args.budget_evals
        } else {
            BUDGET_PNS_BASE_EVALS
        };
        let mut sel =
            Pns::build(&db, ledger, args.ledger.clone(), base).unwrap_or_else(|e| fail(&e));
        eprintln!("{}", sel.census.describe(base));
        let opts = PnsOptions {
            max_total_evals: args.max_total_evals,
            max_jobs: args.max_jobs,
            max_runtime: args.max_runtime,
            stop_file: args.stop_file.clone(),
        };
        let summary = run_pns_batch(&mut session, &mut sel, &opts).unwrap_or_else(|e| fail(&e));
        format!(
            "harvest: policy {} stop={} pns_jobs {} decisive {} evals {} new_shards {} wall {:.1}s",
            args.policy.as_str(),
            if summary.stop_reason.is_empty() {
                "exhausted"
            } else {
                &summary.stop_reason
            },
            summary.jobs,
            summary.decisive,
            summary.evals,
            session.new_shards,
            t_start.elapsed().as_secs_f64(),
        )
    } else {
        let mut jobs = jobs_for_policy(args.policy, &frontier);
        // Optional screen-budget override (probe knob; 0 = policy-resolved).
        if args.budget_evals > 0 {
            for j in &mut jobs {
                j.budget = args.budget_evals;
            }
        }
        eprintln!("harvest: {} jobs", jobs.len());
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
        format!(
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
        )
    };

    session.rewrite_manifest(&args.manifest, &manifest.sha256_hex);
    println!("{summary_line}");
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
