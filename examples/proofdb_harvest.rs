//! `proofdb_harvest` — the harvest loop of the startpos proof-line database
//! (proofdb initiative; plan2/plan16; plan18 policy consolidation).
//!
//! The single coverage policy is the plan16 `frontier-walk` shallowest-first
//! censor-frontier walk: a live selection loop over the censor state in the
//! DB's `censor` table (schema v2; the relocated work ledger, plan20 — the
//! pre-plan20 `--ledger <work.json>` sidecar is retired) with a flat
//! `--budget-evals` budget, no ladder; search visits on underworked
//! records, expansion into the first unhandled child otherwise, bump-only
//! censors, in-session complete rule. Per job: replay the full
//! startpos→node path (context contract — repetition verdicts are
//! path-dependent), run `Search::search_depth_with_prefix` under the job's
//! child-eval budget, and for a decisive outcome export a validator-clean
//! proof subtree via the product's offline pipeline (TT snapshot →
//! `reconstruct` → `validate_proof_tree`), writing the shard into the
//! standing shard directory and recording its manifest entry. `Draw` from
//! any cause is **censored** (no fact, no shard): the visit bumps the
//! record and commits one `censor`-table transaction (plan20 D3 — the
//! harvest's only DB writes; `nodes`/`shards`/`meta` stay
//! `proofdb_merge`-only). The merger runs after the batch as a separate
//! tool — this CLI never merges and never writes tree rows; `--out-db` /
//! `--dump` are the caller's `proofdb_merge` output targets, recorded in
//! the summary only.
//!
//! Module map (all under `examples/proofdb/`): the job pipeline in
//! `session.rs`, the DB-facing engine in `db.rs`, the `frontier-walk`
//! selection loop + batch driver in `walk.rs` + `walk/`, the censor state
//! (incl. the bump-only censor hook) in `ledger.rs`.
//!
//! Output: one `job: {…}` JSON line per job (fields include `policy`,
//! `class`, and the walk census `pass`/`work_before`/`kind`), then
//! `manifest:`/`harvest:` summary lines. On any defect the session aborts
//! non-zero — nothing inconsistent enters the durable layer: the manifest is
//! atomically rewritten after every decisive job (plan17), so a crash or
//! ctrl-c mid-session leaves at most the in-flight job's shard unreferenced
//! (deterministically overwritten by the retry), and every committed censor
//! stays committed (one transaction per censor, plan20 D4).

mod proofdb;
use proofdb::harvest_args::parse_args;

use std::time::Instant;

use proofdb::db::load_db_rows;
use proofdb::session::{Session, fail};
use proofdb::walk::driver::{WalkOptions, run_walk_session};

fn main() {
    let args = parse_args();
    // The session scope (plan19): the `--moves` tokens joined by single
    // spaces (the path-key join); empty = the flag's absence = global.
    let scope = args.moves.join(" ");
    let t_start = Instant::now();
    let manifest = proofdb::read_manifest(&args.manifest).unwrap_or_else(|e| fail(&e));
    let db = load_db_rows(&args.db, &manifest.sha256_hex).unwrap_or_else(|e| fail(&e));
    // The censor-state connection (plan20 D3): read-write — the flag set
    // without SQLITE_OPEN_CREATE refuses to create a missing file, so an
    // absent DB is the stale/absent-layer abort, not a fresh layer.
    let censor = rusqlite::Connection::open_with_flags(
        &args.db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )
    .unwrap_or_else(|e| fail(&format!("cannot open DB {}: {e}", args.db.display())));
    eprintln!(
        "harvest: db {} ({} rows, built_from {}) — policy {}{}",
        args.db.display(),
        db.rows.len(),
        &manifest.sha256_hex[..16],
        proofdb::harvest_args::POLICY,
        if scope.is_empty() {
            String::new()
        } else {
            format!("; scope {scope}")
        },
    );

    let mut session = Session::new(
        args.tt_mb,
        args.shard_dir.clone(),
        args.manifest.clone(),
        manifest.entries.clone(),
        proofdb::harvest_args::POLICY,
    );

    let opts = WalkOptions {
        max_total_evals: args.max_total_evals,
        max_jobs: args.max_jobs,
        max_runtime: args.max_runtime,
        stop_file: args.stop_file.clone(),
    };
    let summary = run_walk_session(&mut session, &db, &censor, args.budget_evals, &opts, &scope)
        .unwrap_or_else(|e| fail(&e));
    let summary_line = format!(
        "harvest: policy {} stop={} jobs {} decisive {} censored {} evals {} \
         new_shards {} wall {:.1}s",
        proofdb::harvest_args::POLICY,
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
    );

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
