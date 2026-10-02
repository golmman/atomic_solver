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
//! the `and-close` completion-gradient policy (plan8, ladder plan9 §2) in
//! `and_close.rs` + `and_close/`, and the sidecar work ledger in
//! `ledger.rs`.
//!
//! Coverage policies (`--policy`; default `breadth-pns`, the plan4 pivot):
//! `breadth-pns` — the plan6 selection mechanism (eligibility, pacing,
//! rationing; the plan4 breadth-first queue as its degenerate case), the
//! sidecar ledger at `--ledger` as pick-up state, per-visit budgets on the
//! revisit ladder `2^(k-1) × base` (base = `--budget-evals` or 4M),
//! session cap = `--max-total-evals`; `--pns-config <file>` loads the
//! mechanism knobs (TOML, plan6 §2; compiled defaults when absent) and the
//! effective config is echoed on the session-start `pns:` line; the plan8
//! `and-close` — the missing
//! replies of the active open rows in the completion-gradient or fresh
//! order (`--and-close-order`), the strict-growth ladder
//! `max(2^(k-1) × base, 2 × work_done)` per reply (plan9 §2) and a
//! bump-only censor hook (no exposure); plus the plan3 legacy gradients
//! `open-deepest`, `sharp-siblings`, `sharp-heavy-tail` (heavy options and
//! `--pns-config` are effective only under `breadth-pns`).
//!
//! Output: one `job: {…}` JSON line per job (fields include `policy`,
//! `class`, for `breadth-pns` the `kind`/`pass`/`number`/`work_before`
//! quadruple, and for `and-close` the `pass`/`work_before` pair with
//! `number`/`kind` null), then `manifest:`/`harvest:` summary lines. On
//! any defect the session aborts non-zero before the manifest rewrite —
//! nothing inconsistent enters the durable layer (shard files of jobs
//! completed before the abort are unreferenced orphans and are
//! deterministically overwritten by the retry).

mod proofdb;
use proofdb::harvest_args::parse_args;

use std::collections::HashSet;
use std::time::Instant;

use proofdb::and_close::driver::{AndCloseOptions, run_and_close_session};
use proofdb::batch::{BatchOptions, run_batch};
use proofdb::db::load_db_rows;
use proofdb::frontier::extract_frontier;
use proofdb::ledger::Ledger;
use proofdb::pns::driver::{PnsOptions, run_pns_batch};
use proofdb::pns::{BUDGET_PNS_BASE_EVALS, Pns, PnsConfig};
use proofdb::policy::{Policy, jobs_for_policy};
use proofdb::session::{Session, fail};

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
        let cfg = match &args.pns_config {
            Some(p) => PnsConfig::load(p).unwrap_or_else(|e| fail(&e)),
            None => PnsConfig::default(),
        };
        let mut sel = Pns::build(
            &db,
            ledger,
            args.ledger.clone(),
            base,
            cfg,
            args.max_total_evals,
        )
        .unwrap_or_else(|e| fail(&e));
        eprintln!("{}; {}", sel.census.describe(base), cfg.describe());
        let opts = PnsOptions {
            max_total_evals: args.max_total_evals,
            max_jobs: args.max_jobs,
            max_runtime: args.max_runtime,
            stop_file: args.stop_file.clone(),
        };
        let summary = run_pns_batch(&mut session, &mut sel, &opts).unwrap_or_else(|e| fail(&e));
        format!(
            "harvest: policy {} stop={} pns_jobs {} rungs {} decisive {} evals {} \
             new_shards {} wall {:.1}s",
            args.policy.as_str(),
            if summary.stop_reason.is_empty() {
                "exhausted"
            } else {
                &summary.stop_reason
            },
            summary.jobs,
            summary.rungs,
            summary.decisive,
            summary.evals,
            session.new_shards,
            t_start.elapsed().as_secs_f64(),
        )
    } else if args.policy == Policy::AndClose {
        // Plan8 D1: the completion-gradient harvest (assembly + census
        // echo + batch live in the driver module for the file-size
        // convention on this target root).
        let opts = AndCloseOptions {
            max_total_evals: args.max_total_evals,
            max_jobs: args.max_jobs,
            max_runtime: args.max_runtime,
            stop_file: args.stop_file.clone(),
            max_budget: args.and_close_max_budget,
        };
        let summary = run_and_close_session(
            &mut session,
            &args.db,
            &manifest.sha256_hex,
            &args.ledger,
            args.and_close_order,
            args.budget_evals,
            &opts,
        )
        .unwrap_or_else(|e| fail(&e));
        format!(
            "harvest: policy {} stop={} jobs {} decisive {} censored {} evals {} \
             new_shards {} wall {:.1}s",
            args.policy.as_str(),
            if summary.stop_reason.is_empty() {
                "exhausted"
            } else {
                &summary.stop_reason
            },
            summary.jobs,
            summary.decisive,
            summary.censored,
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
