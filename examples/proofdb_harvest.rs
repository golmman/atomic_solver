//! `proofdb_harvest` — the harvest loop of the startpos proof-line database
//! (proofdb initiative; plan2/plan16; plan18 policy consolidation; plan22
//! sidecar production).
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
//! `reconstruct` → `validate_proof_tree`), writing the shard as a durable
//! `<tag>.bin` + `<tag>.meta.json` sidecar pair (plan22 D1 — the sidecar is
//! the commit marker). `Draw` from any cause is **censored** (no fact, no
//! shard): the visit bumps the record and commits one `censor`-table
//! transaction (plan20 D3 — the harvest's only DB writes; `nodes`/`shards`/
//! `meta` stay `proofdb_merge`-only). The merger runs after the batch as a
//! separate tool — this CLI never merges and never writes tree rows;
//! `--out-db` / `--dump` are the caller's `proofdb_merge` output targets,
//! recorded in the summary only.
//!
//! Unmerged-shard gate (plan22 D6, replacing the retired `built_from`
//! manifest-digest staleness check): before any work, the shard dir is
//! scanned — a sidecar whose tag is absent from the DB's `shards` table
//! refuses the session (exit 1, run `proofdb_merge` first), as does a
//! sidecar without its bin; a bin without a sidecar is the interrupted
//! job's orphan and is ignored. Session collision sets are seeded from the
//! DB's `shards` rows (the manifest is gone).
//!
//! Module map (all under `examples/proofdb/`): the job pipeline in
//! `session.rs`, the DB-facing engine in `db.rs`, the `frontier-walk`
//! selection loop + batch driver in `walk.rs` + `walk/`, the censor state
//! (incl. the bump-only censor hook) in `ledger.rs`.
//!
//! Output: one `job: {…}` JSON line per job (fields include `policy`,
//! `class`, and the walk census `pass`/`work_before`/`kind`), then the
//! `harvest:` summary line. On any defect the session aborts non-zero —
//! nothing inconsistent enters the durable layer: every finished job's
//! bin + sidecar pair is on disk (the sidecar written last, atomically), so
//! a crash or ctrl-c mid-session leaves at most the in-flight job's bin
//! unreferenced (deterministically overwritten by the retry), and every
//! committed censor stays committed (one transaction per censor, plan20
//! D4).

mod proofdb;
use proofdb::harvest_args::parse_args;

use std::time::Instant;

use proofdb::db::{check_unmerged_shards, load_db_rows};
use proofdb::schema::read_shard_rows;
use proofdb::session::{Session, fail};
use proofdb::walk::driver::{WalkOptions, run_walk_session};

fn main() {
    let args = parse_args();
    // The session scope (plan19): the `--moves` tokens joined by single
    // spaces (the path-key join); empty = the flag's absence = global.
    let scope = args.moves.join(" ");
    let t_start = Instant::now();
    // The rows connection (read-only, no CREATE): a missing DB is the
    // stale/absent-layer abort, not a fresh layer.
    let ro =
        rusqlite::Connection::open_with_flags(&args.db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap_or_else(|e| fail(&format!("cannot open DB {}: {e}", args.db.display())));
    let shard_rows = read_shard_rows(&ro).unwrap_or_else(|e| fail(&e));
    // The unmerged-shard gate (plan22 D6): any shard collected since the
    // last fold refuses the session. Exit 1 (refusal class), naming the tag.
    if let Err(e) = check_unmerged_shards(&args.shard_dir, &shard_rows) {
        eprintln!("proofdb_harvest: {e}");
        std::process::exit(1);
    }
    let db = load_db_rows(&args.db).unwrap_or_else(|e| fail(&e));
    // The censor-state connection (plan20 D3): read-write — the flag set
    // without SQLITE_OPEN_CREATE refuses to create a missing file, so an
    // absent DB is the stale/absent-layer abort, not a fresh layer.
    let censor = rusqlite::Connection::open_with_flags(
        &args.db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )
    .unwrap_or_else(|e| fail(&format!("cannot open DB {}: {e}", args.db.display())));
    eprintln!(
        "harvest: db {} ({} rows, fold digest {}) — policy {}{}",
        args.db.display(),
        db.rows.len(),
        &db.built_from[..16.min(db.built_from.len())],
        proofdb::harvest_args::POLICY,
        if scope.is_empty() {
            String::new()
        } else {
            format!("; scope {scope}")
        },
    );

    let seed_paths: std::collections::HashSet<String> =
        shard_rows.iter().map(|r| r.path.clone()).collect();
    let mut session = Session::new(
        args.tt_mb,
        args.shard_dir.clone(),
        seed_paths,
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
         new_shards {} or_closes {} wall {:.1}s",
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
        summary.or_closes,
        t_start.elapsed().as_secs_f64(),
    );

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
