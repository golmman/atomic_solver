//! Integration harness for the proofdb merger (examples-side tooling).
//!
//! `cargo test` does not run unit tests inside example targets, so the merger
//! modules are included here verbatim via `#[path]` (their inline unit tests
//! then run as part of this target). This file additionally covers the
//! end-to-end pipeline over a synthetic fixture shard.

#[path = "../examples/proofdb/mod.rs"]
mod proofdb;

use atomic_movegen::types::Move;

use atomic_solver::notation::uci_to_move;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofTree, validate_proof_tree};

use proofdb::manifest::read_manifest;
use proofdb::merge::PathTree;
use proofdb::schema::{ShardRow, canonical_dump, write_db};

// NOTE (plan22): the standing-layer pins moved to `tests/proofdb_fold.rs`
// — the byte-identical rebuild contract and the stale-DB `built_from`
// guard are retired with the manifest; their replacements are the
// deterministic fold + `proofdb_verify` (R1/R2/R8).

const ROOT_FEN: &str = Position::STARTPOS_FEN;
/// After 1.f3 e6 2.g4: Qd8-h4 explodes the White king (win in 1).
const TACTIC_FEN: &str = proofdb::fixture::TACTIC_FEN;
const TACTIC_PATH: &[&str] = &["f2f3", "e7e6", "g2g4"];

/// Build the win-in-1 fixture tree (root + terminal child) and serialize it.
fn fixture_bin() -> Vec<u8> {
    let mut buf = Vec::new();
    proofdb::fixture::win_in_1_tree(TACTIC_FEN)
        .to_bin(&mut buf)
        .expect("fixture serializes");
    buf
}

fn moves_from_path(ucis: &[impl AsRef<str>]) -> Vec<Move> {
    let mut pos = Position::from_fen(ROOT_FEN).unwrap();
    ucis.iter()
        .map(|u| {
            let mv = uci_to_move(u.as_ref(), &pos).expect("legal");
            pos.do_move(mv);
            mv
        })
        .collect()
}

#[test]
fn end_to_end_fixture_pipeline() {
    let dir = std::env::temp_dir().join(format!("proofdb_e2e_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("fixture.bin"), fixture_bin()).unwrap();
    let manifest_path = dir.join("manifest.json");
    std::fs::write(
        &manifest_path,
        format!(
            r#"{{"entries":[{{"tag":"fixture","file":"fixture.bin","fen":"{TACTIC_FEN}",
                "moves":["f2f3","e7e6","g2g4"],"outcome":"win","validate":"ok"}}]}}"#
        ),
    )
    .unwrap();
    let manifest = read_manifest(&manifest_path).unwrap();
    assert_eq!(manifest.entries.len(), 1);

    // The merger pipeline (as proofdb_merge drives it) over the fixture.
    let mut tree = PathTree::new();
    let mut rows = Vec::new();
    let mut graft_idx = 0usize;
    for entry in &manifest.entries {
        let bytes = std::fs::read(dir.join(&entry.file)).unwrap();
        let mut shard = ProofTree::from_bin(&mut bytes.as_slice()).unwrap();
        assert!(validate_proof_tree(&shard).is_ok());
        assert_eq!(shard.nodes[0].outcome, Some(entry.outcome));
        let moves = moves_from_path(&entry.moves);
        let graft = tree.graft_path(&moves, &entry.tag);
        graft_idx = graft;
        tree.overlay_subtree(graft, &shard, &entry.tag).unwrap();
        rows.push(ShardRow {
            tag: entry.tag.clone(),
            file: entry.file.clone(),
            sha256: proofdb::digest_hex(&bytes),
            root_fen: std::mem::take(&mut shard.root_fen),
            path: entry.moves.join(" "),
            outcome: entry.outcome,
            depth_bound: 1,
            n_nodes: 2,
        });
    }
    tree.finalize().unwrap();
    let skeleton = tree.reassemble_skeleton(graft_idx, TACTIC_FEN).unwrap();
    assert!(validate_proof_tree(&skeleton).is_ok());

    let db_path = dir.join("proofdb.db");
    write_db(
        &db_path,
        ROOT_FEN,
        &proofdb::digest_hex(&std::fs::read(&manifest_path).unwrap()),
        &rows,
        &tree,
        &[],
    )
    .unwrap();
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let schema_version: String = conn
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "2");
    let node_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(node_count, 5); // root + 3 graft ancestors + shard root + terminal
    let (outcome, bound, status, prov): (Option<String>, Option<i64>, Option<String>, String) =
        conn.query_row(
            "SELECT outcome, depth_bound, depth_status, provenance FROM nodes
             WHERE ply=3 AND move_uci='g2g4'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(outcome.as_deref(), Some("win"));
    assert_eq!(bound, Some(1));
    assert_eq!(status.as_deref(), Some("bound"));
    assert_eq!(prov, "fixture");

    // Determinism: rebuilding from the same shard set is byte-identical.
    let dump1 = canonical_dump(
        ROOT_FEN,
        &proofdb::digest_hex(&std::fs::read(&manifest_path).unwrap()),
        &tree,
    );
    let mut tree2 = PathTree::new();
    tree2.graft_path(&moves_from_path(TACTIC_PATH), "fixture");
    let bytes = std::fs::read(dir.join("fixture.bin")).unwrap();
    let shard = ProofTree::from_bin(&mut bytes.as_slice()).unwrap();
    let graft2 = tree2.graft_path(&moves_from_path(TACTIC_PATH), "fixture");
    tree2.overlay_subtree(graft2, &shard, "fixture").unwrap();
    tree2.finalize().unwrap();
    assert_eq!(
        dump1,
        canonical_dump(
            ROOT_FEN,
            &proofdb::digest_hex(&std::fs::read(&manifest_path).unwrap()),
            &tree2
        )
    );

    std::fs::remove_dir_all(&dir).ok();
}

// --- censor-state helpers (plan20; the ledger-union tooling is retired) ---

use proofdb::ledger::{Ledger, LedgerEntry};
use rusqlite::Connection;

/// Build a censor-state map from `(space-joined path key, work_done,
/// passes_failed)` records (the key "" is the root, exactly as
/// `Ledger::load_db` produces).
fn ledger_of(records: &[(&str, u64, u32)]) -> Ledger {
    let mut m = std::collections::BTreeMap::new();
    for (key, work, passes) in records {
        let inserted = m.insert(
            (*key).to_string(),
            LedgerEntry {
                work_done: *work,
                passes_failed: *passes,
            },
        );
        assert!(inserted.is_none(), "duplicate test key {key:?}");
    }
    Ledger::from_entries(m)
}

fn entries(led: &Ledger) -> Vec<(String, u64, u32)> {
    led.iter()
        .map(|(k, e)| (k.clone(), e.work_done, e.passes_failed))
        .collect()
}

/// An in-memory censor DB (the schema-v2 table shape `write_db` creates) —
/// the driver tests' censor-state backend where persistence isn't asserted.
fn censor_mem() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE censor (path TEXT PRIMARY KEY, work_done INTEGER NOT NULL, \
         passes_failed INTEGER NOT NULL);",
    )
    .unwrap();
    conn
}

/// A file-backed censor DB at `<dir>/<name>` (persists across connections —
/// the second-session pick-up tests).
fn censor_file(dir: &std::path::Path, name: &str) -> Connection {
    let conn = Connection::open(dir.join(name)).unwrap();
    conn.execute_batch(
        "CREATE TABLE censor (path TEXT PRIMARY KEY, work_done INTEGER NOT NULL, \
         passes_failed INTEGER NOT NULL);",
    )
    .unwrap();
    conn
}

// --- plan22: the fold replaces the full rebuild (the standing-layer pins
// live in tests/proofdb_fold.rs — R1/R2; the byte-identical rebuild
// contract and the stale-DB built_from guard are retired with it) ---

// --- plan14: the CLI defaults are centralized under data/proofdb/ ---

/// Every generated-proofdb default must live wholly under the gitignored
/// working layer `data/proofdb/` (plan14): the shard layer co-located under
/// `data/proofdb/shards/`, and every default path directory-qualified (a
/// bare file name in the cwd is exactly what this item removed).
#[test]
fn cli_defaults_are_centralized_under_data_proofdb() {
    use proofdb::{DEFAULT_DB, DEFAULT_FLIP_OUT, DEFAULT_MANIFEST, DEFAULT_SHARD_DIR};

    assert_eq!(DEFAULT_DB, "data/proofdb/proofdb.db");
    assert_eq!(DEFAULT_MANIFEST, "data/proofdb/shards/manifest.json");
    assert_eq!(DEFAULT_SHARD_DIR, "data/proofdb/shards");
    assert_eq!(DEFAULT_FLIP_OUT, "data/proofdb/flip.json");

    for path in [
        DEFAULT_DB,
        DEFAULT_MANIFEST,
        DEFAULT_SHARD_DIR,
        DEFAULT_FLIP_OUT,
    ] {
        let p = std::path::Path::new(path);
        assert!(
            p.starts_with("data/proofdb"),
            "default {path} outside data/proofdb/"
        );
        assert!(
            p.parent().is_some_and(|d| !d.as_os_str().is_empty()),
            "default {path} is not directory-qualified"
        );
    }
}

// --- plan25: the export fill budgets are operator knobs ---

/// The harvest's export budgets reach `ReconstructConfig` (plan25 D4/A5):
/// the defaults are the previously hardcoded values (2 M / 100 M), the fill
/// base and the depth cap stay fixed, and an explicit budget pair overrides
/// exactly the two fill budgets (the cap now governs root holes only, so it is
/// not an operator knob).
#[test]
fn export_budgets_reach_the_reconstruct_config() {
    use atomic_solver::reconstruct::ReconstructConfig;

    let default = proofdb::session::ExportBudgets::default();
    assert_eq!(default.attempt, 2_000_000);
    assert_eq!(default.total, 100_000_000);
    let cfg = default.config(128);
    assert_eq!(cfg.tt_mb, 128);
    assert_eq!(cfg.fill_base, 8);
    assert_eq!(cfg.fill_depth_cap, 32);
    assert_eq!(cfg.fill_attempt_budget, 2_000_000);
    assert_eq!(cfg.fill_total_budget, 100_000_000);

    let explicit = proofdb::session::ExportBudgets {
        attempt: 20_000_000,
        total: 0,
    };
    let cfg = explicit.config(64);
    assert_eq!(cfg.fill_attempt_budget, 20_000_000);
    assert_eq!(cfg.fill_total_budget, 0);
    assert_eq!(
        cfg.fill_depth_cap,
        ReconstructConfig::default().fill_depth_cap
    );
}

/// A rejected export's message carries the walk census and the fill
/// accounting (plan25 D5/A6): the abort line is the operator's only
/// diagnostic, so `holes: …` and `fill_evals=N` must be in it. Fixture: an
/// empty search TT (the root is a hole) with a one-eval attempt budget, so
/// the fill is censored and the export is rejected.
#[test]
fn export_rejection_carries_the_walk_census() {
    use atomic_solver::reconstruct::ReconstructConfig;
    use atomic_solver::search::dfpn::Search;
    use proofdb::shard_export::export_validated_shard;

    let fen = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";
    let search = Search::new(16);
    let config = ReconstructConfig {
        tt_mb: 16,
        fill_attempt_budget: 1,
        ..ReconstructConfig::default()
    };
    let err = export_validated_shard(&search, fen, &config)
        .expect_err("an unsolved snapshot must be rejected");
    assert!(
        err.contains("BudgetExhausted: fill at root censored at bound 8"),
        "unexpected: {err}"
    );
    assert!(err.contains("(holes: hit=0"), "census missing: {err}");
    assert!(
        err.contains("fill_evals="),
        "fill accounting missing: {err}"
    );
}

// --- plan16 D1: the frontier-walk shallowest-first ledger walk (walk.rs) ---

use proofdb::walk::census::RootState;
use proofdb::walk::driver::{WalkOptions, run_walk_batch, run_walk_session};
use proofdb::walk::{WalkFrontier, root_state};

use proofdb::harvest::replay_job_path;
use proofdb::session::Session;

/// A crafted DB content (the and-close section's helper, kept for the walk
/// and crash-sync tests): rows of (space-joined path, ply, outcome).
fn walk_db(rows: &[(&str, usize, Option<Outcome>)]) -> proofdb::db::DbContent {
    let rows = rows
        .iter()
        .map(|&(p, ply, o)| proofdb::db::DbRow {
            path: if p.is_empty() {
                Vec::new()
            } else {
                p.split(' ').map(str::to_string).collect()
            },
            ply,
            outcome: o,
            depth_bound: o.map(|_| 1),
            depth_status: None,
        })
        .collect();
    proofdb::db::DbContent {
        root_fen: Position::STARTPOS_FEN.to_string(),
        built_from: "0".repeat(64),
        rows,
    }
}

fn walk_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("proofdb_walk_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn walk_opts(max_jobs: usize, max_total_evals: u64) -> WalkOptions {
    WalkOptions {
        max_total_evals,
        max_jobs,
        max_runtime: 0,
        stop_file: std::path::PathBuf::new(),
    }
}

/// Plan16 task 1: the driver's root-expansion phase on the fixture DB at a
/// tiny budget — the first visit is the virtual-root search (class C1), the
/// following ones are expansion visits on fresh children (class C3, kind
/// `expand`); the censor state grows by exactly the censor count (the D4
/// signature) and a fresh run reproduces the post-run censor rows.
#[test]
fn walk_driver_root_expansion_and_ledger_growth() {
    let dir = walk_dir("walk_driver");
    let db_path = proofdb::fixture::fixture_db(&dir);
    let db = proofdb::db::load_db_rows(&db_path).unwrap();

    let run = |censor: &Connection| {
        let mut session = Session::new(
            4,
            dir.join("shards"),
            std::collections::HashSet::new(),
            "frontier-walk",
        );
        let (mut frontier, mut census) = WalkFrontier::new(&Ledger::default(), &db, "");
        census.root = root_state(&db, &session, "");
        assert_eq!(census.records, 0);
        assert_eq!(census.root, RootState::Open);
        let mut ledger = Ledger::default();
        let summary = run_walk_batch(
            &mut session,
            &db,
            &mut frontier,
            &mut ledger,
            censor,
            1_000,
            &walk_opts(5, 0),
            "",
        )
        .unwrap();
        (summary, ledger)
    };
    let censor1 = censor_file(&dir, "censor1.db");
    let (summary, ledger) = run(&censor1);
    assert_eq!(summary.jobs, 5);
    assert_eq!(summary.stop_reason, "max-jobs");
    assert_eq!(summary.decisive, 0);
    assert_eq!(summary.censored, 5);
    // The D4 signature: censor-state growth == fresh-node censor count.
    assert_eq!(summary.fresh_censored, 5);
    assert_eq!(ledger.len(), 5);
    // Visit 1: the virtual root (class C1, kind search, pass 1,
    // work_before 0); visits 2+: the root's expansion targets (class C3,
    // kind expand), lex order — and the fixture's stored rows (f2f3, g2g4)
    // are skipped as handled territory. The censor keys pin the sequence.
    let keys: Vec<String> = ledger.iter().map(|(k, _)| k.clone()).collect();
    assert_eq!(keys, vec!["", "a2a3", "a2a4", "b1a3", "b1c3"]);
    let root = ledger.get("").unwrap();
    assert!(
        root.work_done >= 1_000,
        "root censor spent at least the budget"
    );
    assert_eq!(root.passes_failed, 1);
    // Determinism: a fresh session over the same inputs reproduces the
    // post-run censor rows (row-for-row — the censor state lives in the
    // DB since plan20, so the old byte-pin becomes a row pin).
    let censor2 = censor_file(&dir, "censor2.db");
    let (summary2, ledger2) = run(&censor2);
    assert_eq!(summary2.jobs, summary.jobs);
    assert_eq!(entries(&ledger), entries(&ledger2));
    assert_eq!(
        entries(&Ledger::load_db(&censor1).unwrap()),
        entries(&Ledger::load_db(&censor2).unwrap())
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// Plan16 task 1: the complete rule (D3) — a candidate whose every legal
/// child is handled (here: the root's 20 first moves all carry ledger
/// records at work ≥ B) is complete-marked by a pick-only iteration; no
/// job runs, the loop cannot stall, and the session exhausts.
#[test]
fn walk_complete_rule_no_livelock_and_exhaustion() {
    let dir = walk_dir("walk_complete");
    let db = walk_db(&[("", 0, None)]);
    // The root's 20 first-move children, all at work ≥ the budget.
    let (pos, _) = replay_job_path(&[]).unwrap();
    let mut children: Vec<String> = pos
        .legal_moves_vec()
        .iter()
        .map(|&m| atomic_solver::notation::move_to_uci(m))
        .collect();
    children.sort();
    assert_eq!(children.len(), 20);
    let mut m = std::collections::BTreeMap::new();
    m.insert(
        "".to_string(),
        proofdb::ledger::LedgerEntry {
            work_done: 2_000,
            passes_failed: 1,
        },
    );
    for uci in &children {
        m.insert(
            uci.clone(),
            proofdb::ledger::LedgerEntry {
                work_done: 2_000,
                passes_failed: 1,
            },
        );
    }
    let ledger = Ledger::from_entries(m);

    let mut session = Session::new(
        4,
        dir.join("shards"),
        std::collections::HashSet::new(),
        "frontier-walk",
    );
    let (mut frontier, census) = WalkFrontier::new(&ledger, &db, "");
    assert_eq!(census.records, 21);
    let mut led = ledger.clone();
    // The root completes pick-only (no job); its 20 children then become
    // the pick and expand into fresh grandchildren — capped at 3 searches.
    let summary = run_walk_batch(
        &mut session,
        &db,
        &mut frontier,
        &mut led,
        &censor_mem(),
        1_000,
        &walk_opts(3, 0),
        "",
    )
    .unwrap();
    // The root was complete-marked by a pick-only iteration (the no-livelock
    // design-out); the searches that ran are its children's expansions.
    assert_eq!(summary.stop_reason, "max-jobs");
    assert_eq!(summary.jobs, 3);
    assert_eq!(summary.completes, 1);
    assert_eq!(summary.fresh_censored, 3);
    // The complete-marked root's ledger record is retained (complete ≠
    // deletion) and the three fresh grandchildren joined the ledger.
    assert_eq!(led.len(), 24);
    assert_eq!(led.get("").unwrap().work_done, 2_000);
    std::fs::remove_dir_all(&dir).ok();
}

/// Plan16 task 1: the root rule's exhausted case — a proven-stored root
/// (proven DB row) ends the session before any job.
#[test]
fn walk_root_proven_exhausts() {
    let dir = walk_dir("walk_proven_root");
    let db = walk_db(&[("", 0, Some(Outcome::Win))]);
    let ledger = ledger_of(&[("a2a3", 100, 1)]);
    let mut session = Session::new(
        4,
        dir.join("shards"),
        std::collections::HashSet::new(),
        "frontier-walk",
    );
    let (mut frontier, mut census) = WalkFrontier::new(&ledger, &db, "");
    // The record sits behind the proven root row: ancestor drop (the empty
    // prefix), the root itself is proven-stored.
    assert_eq!((census.lineage, census.ancestor), (0, 1));
    census.root = root_state(&db, &session, "");
    assert_eq!(census.root, RootState::Proven);
    let mut led = ledger.clone();
    let summary = run_walk_batch(
        &mut session,
        &db,
        &mut frontier,
        &mut led,
        &censor_mem(),
        1_000,
        &walk_opts(0, 0),
        "",
    )
    .unwrap();
    assert_eq!(summary.stop_reason, "");
    assert_eq!(summary.jobs, 0);
    std::fs::remove_dir_all(&dir).ok();
}

/// Plan16 task 5 (plan22 shape): the fresh-layer boot on a temp layer —
/// the R0 bootstrap fold (a fresh schema → the root-only DB) → capped
/// `frontier-walk` batch (durable bin + sidecar pairs) → fold → the grown
/// DB has more than the root. The walk does not self-retire (D6), so the
/// batch is eval-capped; 20k budget, 10M cap (measured: the first decisive
/// shard lands at ~4.4M evals).
#[test]
fn walk_boot_from_empty_layer() {
    use proofdb::merge::fold::fold;

    let dir = walk_dir("walk_boot");
    // The R0 bootstrap fold: a fresh (empty-file) DB folds the empty shard
    // dir into the root-only standing layer.
    std::fs::create_dir_all(dir.join("shards")).unwrap();
    let mut conn = Connection::open(dir.join("proofdb.db")).unwrap();
    fold(&mut conn, &dir.join("shards"), None, None).unwrap();
    let db = proofdb::db::load_db_rows(&dir.join("proofdb.db")).unwrap();
    assert_eq!(db.rows.len(), 1, "the root-only bootstrap DB");

    let mut session = Session::new(
        4,
        dir.join("shards"),
        std::collections::HashSet::new(),
        "frontier-walk",
    );
    let censor_conn = Connection::open(dir.join("proofdb.db")).unwrap();
    let summary = run_walk_session(
        &mut session,
        &db,
        &censor_conn,
        20_000,
        &walk_opts(0, 10_000_000),
        "",
    )
    .unwrap();
    assert_eq!(summary.stop_reason, "budget");
    assert!(summary.decisive >= 1, "boot yield: >= 1 decisive shard");
    assert_eq!(session.new_shards, summary.decisive);
    // Plan22 D1: every decisive job left a durable bin + sidecar pair on
    // disk (the sidecar is the commit marker); no manifest exists, no tmp
    // residue remains.
    let pairs = sidecar_pairs(&dir.join("shards"));
    assert_eq!(pairs, summary.decisive, "bin + sidecar pairs on disk");
    assert!(!dir.join("shards/manifest.json").exists(), "no manifest");
    let residue: Vec<_> = std::fs::read_dir(dir.join("shards"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(residue.is_empty(), "tmp residue: {residue:?}");
    // The D4 signature over a full session: the censor rows landed in the
    // boot DB (plan20 — the censor state lives in the DB).
    let ledger = Ledger::load_db(&Connection::open(dir.join("proofdb.db")).unwrap()).unwrap();
    assert_eq!(ledger.len(), summary.fresh_censored);

    // Fold the grown set (the harvest never merges) → DB > 1 node.
    let out = fold(&mut conn, &dir.join("shards"), None, None).unwrap();
    assert_eq!(out.report.folded, summary.decisive, "all shards folded");
    assert_eq!(out.report.node_count, out.report.proven + out.report.open);
    assert!(out.report.node_count > 1, "grown DB has more than the root");
    std::fs::remove_dir_all(&dir).ok();
}

/// Count the durable `<tag>.bin` + `<tag>.meta.json` pairs in `dir`
/// (plan22 D1: a pair is the produced-shard shape; bins without sidecars
/// are in-flight orphans).
fn sidecar_pairs(dir: &std::path::Path) -> usize {
    let mut bins = std::collections::HashSet::new();
    let mut sides = std::collections::HashSet::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(t) = name.strip_suffix(".bin") {
            bins.insert(t.to_string());
        } else if let Some(t) = name.strip_suffix(".meta.json") {
            sides.insert(t.to_string());
        }
    }
    bins.intersection(&sides).count()
}

/// Step 1 of the proven-draw handling: an on-path repetition
/// (`g1f3 g8f6 f3g1 f6g8` returns to the startpos) is a `Complete` Draw at
/// zero child-evals. It is a proof at that path, not a fact of absence, so
/// the visit records a `drawn` row and prunes the path — no censor bump and
/// no re-pick. The walk moves on to the virtual root.
#[test]
fn walk_zero_eval_complete_draw_is_proven_and_pruned() {
    let dir = walk_dir("walk_repetition");
    let db = walk_db(&[("", 0, None)]);
    let rep = "g1f3 g8f6 f3g1 f6g8";
    let ledger = ledger_of(&[(rep, 4, 1)]);
    let mut session = Session::new(
        4,
        dir.join("shards"),
        std::collections::HashSet::new(),
        "frontier-walk",
    );
    let (mut frontier, mut census) = WalkFrontier::new(&ledger, &db, "");
    assert_eq!(census.records, 1);
    census.root = root_state(&db, &session, "");
    assert_eq!(census.root, RootState::Open);
    let mut led = ledger.clone();
    let summary = run_walk_batch(
        &mut session,
        &db,
        &mut frontier,
        &mut led,
        &censor_mem(),
        20_000,
        &walk_opts(4, 0),
        "",
    )
    .unwrap();
    // Visit 1: the repetition record (0 evals, Complete Draw, drawn and
    // pruned); visits 2–4: the virtual root search (C1) and two root
    // expansions.
    assert_eq!(summary.jobs, 4);
    assert_eq!(summary.stop_reason, "max-jobs");
    assert_eq!(summary.censored, 3);
    assert_eq!(summary.drawn, 1);
    assert_eq!(summary.completes, 0, "a draw is not an exhaustion complete");
    // The record is not bumped (no censor), keeps its history, and carries
    // the durable drawn mark; the root record was created by the root censor.
    let e = led.get(rep).unwrap();
    assert_eq!((e.work_done, e.passes_failed), (4, 1));
    assert!(led.is_drawn(rep));
    assert!(led.contains(""));
    std::fs::remove_dir_all(&dir).ok();
}

// --- plan22: the session's durable layer (sidecar pairs) ---

/// Plan22 R9 (the plan17 crash-window contracts, sidecar world): a
/// censor-only session touches no durable file (the shard dir stays
/// empty), and the produced-shard shape is the bin + sidecar pair with the
/// sidecar's sha256 equal to its bin's digest (the fold's corruption
/// cross-check and idempotence key). The interrupted-write abort itself is
/// `fail()` — a process exit, untestable in-process; the bin-without-
/// sidecar orphan window is covered in `tests/proofdb_fold.rs` (R5).
#[test]
fn session_censored_jobs_touch_no_durable_file_and_pairs_are_self_describing() {
    use proofdb::sidecar::read_sidecar;

    // (a) The censor-only session: tiny budget on the quiet fixture, every
    // job censors — nothing durable is produced, the shard dir stays empty.
    let dir = walk_dir("p22_censored");
    let db_path = proofdb::fixture::fixture_db(&dir);
    let db = proofdb::db::load_db_rows(&db_path).unwrap();
    std::fs::create_dir_all(dir.join("shards")).unwrap();
    let mut session = Session::new(
        4,
        dir.join("shards"),
        std::collections::HashSet::new(),
        "frontier-walk",
    );
    let (mut frontier, mut census) = WalkFrontier::new(&Ledger::default(), &db, "");
    census.root = root_state(&db, &session, "");
    assert_eq!(census.root, RootState::Open);
    let mut led = Ledger::default();
    let summary = run_walk_batch(
        &mut session,
        &db,
        &mut frontier,
        &mut led,
        &censor_mem(),
        1_000,
        &walk_opts(5, 0),
        "",
    )
    .unwrap();
    assert_eq!(summary.decisive, 0);
    assert_eq!(summary.censored, 5);
    assert_eq!(session.new_shards, 0);
    let durable: Vec<_> = std::fs::read_dir(dir.join("shards"))
        .unwrap()
        .flatten()
        .collect();
    assert!(
        durable.is_empty(),
        "censored jobs wrote nothing: {durable:?}"
    );
    std::fs::remove_dir_all(&dir).ok();

    // (b) The produced-shard shape: the session's sidecar carries the bin
    // digest, and a read-back round-trip agrees (the self-describing pair).
    let dir = walk_dir("p22_pair");
    std::fs::create_dir_all(dir.join("shards")).unwrap();
    let bin = dir.join("shards/h_pair.bin");
    let bytes = fixture_bin();
    std::fs::write(&bin, &bytes).unwrap();
    let entry = proofdb::sidecar::SidecarEntry {
        tag: "h_pair".to_string(),
        file: "h_pair.bin".to_string(),
        fen: TACTIC_FEN.to_string(),
        moves: TACTIC_PATH.iter().map(|s| s.to_string()).collect(),
        outcome: Outcome::Win,
        sha256: proofdb::digest_hex(&bytes),
    };
    proofdb::sidecar::write_sidecar(&dir.join("shards"), &entry).unwrap();
    let side = read_sidecar(&dir.join("shards/h_pair.meta.json")).unwrap();
    assert_eq!(side.tag, "h_pair");
    assert_eq!(side.file, "h_pair.bin");
    assert_eq!(
        side.sha256,
        proofdb::digest_hex(&std::fs::read(&bin).unwrap())
    );
    assert_eq!(side.moves, entry.moves);
    assert_eq!(side.outcome, Outcome::Win);
    assert!(
        !dir.join("shards/h_pair.meta.json.tmp").exists(),
        "tmp residue"
    );
    std::fs::remove_dir_all(&dir).ok();
}

// --- plan19: the scoped session (`--moves` re-anchor) ---

/// Plan19 task 3: the scoped session's root rule — with an empty ledger the
/// virtual-root search runs at the scope key (class C1, the scope root's
/// special position), and its expansion children stay inside the scope by
/// construction.
#[test]
fn walk_scope_virtual_root_and_expansion_stay_in_scope() {
    let dir = walk_dir("walk_scope_root");
    // An open row at the scope root (the fresh-layer bootstrap,
    // transplanted); no game-root row needed for a scoped session.
    let db = walk_db(&[("a2a3", 1, None)]);
    let mut session = Session::new(
        4,
        dir.join("shards"),
        std::collections::HashSet::new(),
        "frontier-walk",
    );
    let ledger = Ledger::default();
    let (mut frontier, mut census) = WalkFrontier::new(&ledger, &db, "a2a3");
    assert_eq!(census.out_of_scope, 0);
    assert_eq!(census.scope, "a2a3");
    census.root = root_state(&db, &session, "a2a3");
    assert_eq!(census.root, RootState::Open);
    let mut led = Ledger::default();
    let summary = run_walk_batch(
        &mut session,
        &db,
        &mut frontier,
        &mut led,
        &censor_mem(),
        1_000,
        &walk_opts(3, 0),
        "a2a3",
    )
    .unwrap();
    assert_eq!(summary.stop_reason, "max-jobs");
    assert_eq!(summary.jobs, 3);
    assert_eq!(summary.censored, 3);
    // Visit 1: the virtual root at the scope key (ledger key == scope);
    // visits 2–3: expansion children of the scope root, in scope by
    // construction (absolute keys carry the scope prefix).
    let keys: Vec<String> = led.iter().map(|(k, _)| k.clone()).collect();
    assert_eq!(keys, vec!["a2a3", "a2a3 a7a5", "a2a3 a7a6"]);
    std::fs::remove_dir_all(&dir).ok();
}

/// Plan19 task 3: a proven-stored scope root ends the session before any
/// job (`exhausted`, the stderr note fires in the binary; ruling b: no
/// work beneath proven nodes) — out-of-scope records are never picked.
#[test]
fn walk_scope_proven_scope_root_exhausts() {
    let dir = walk_dir("walk_scope_proven");
    let db = walk_db(&[("a2a3", 1, Some(Outcome::Win))]);
    let ledger = ledger_of(&[("b1c3", 100, 1)]);
    let mut session = Session::new(
        4,
        dir.join("shards"),
        std::collections::HashSet::new(),
        "frontier-walk",
    );
    let (mut frontier, census) = WalkFrontier::new(&ledger, &db, "a2a3");
    assert_eq!(census.out_of_scope, 1);
    assert_eq!(root_state(&db, &session, "a2a3"), RootState::Proven);
    let mut led = ledger.clone();
    let summary = run_walk_batch(
        &mut session,
        &db,
        &mut frontier,
        &mut led,
        &censor_mem(),
        1_000,
        &walk_opts(0, 0),
        "a2a3",
    )
    .unwrap();
    assert_eq!(summary.stop_reason, "");
    assert_eq!(summary.jobs, 0);
    // The shared ledger is untouched by the exhausted scoped session.
    assert_eq!(entries(&led), entries(&ledger));
    std::fs::remove_dir_all(&dir).ok();
}

/// Plan19 task 4: the scoped session end-to-end through
/// `run_walk_session` — the census line describes the scoped world and the
/// ledger only grows inside the scope; out-of-scope records are preserved
/// byte-identically.
#[test]
fn walk_scope_session_pickup_and_preservation() {
    let dir = walk_dir("walk_scope_session");
    let db_path = proofdb::fixture::fixture_db(&dir);
    let db = proofdb::db::load_db_rows(&db_path).unwrap();
    // A shared ledger with an out-of-scope record (work ≥ B so it would be
    // an expansion candidate in a global session — but out of scope here).
    // A shared censor DB with an out-of-scope record (work ≥ B so it would
    // be an expansion candidate in a global session — but out of scope
    // here); seeded the way a standing layer carries it (plan20).
    let censor = censor_file(&dir, "censor.db");
    censor
        .execute(
            "INSERT INTO censor (path, work_done, passes_failed) VALUES ('b1c3', 50000, 1)",
            [],
        )
        .unwrap();

    let mut session = Session::new(
        4,
        dir.join("shards"),
        std::collections::HashSet::new(),
        "frontier-walk",
    );
    let summary = run_walk_session(
        &mut session,
        &db,
        &censor,
        1_000,
        &walk_opts(2, 0),
        "f2f3 e7e6",
    )
    .unwrap();
    assert_eq!(summary.stop_reason, "max-jobs");
    assert_eq!(summary.jobs, 2);
    // Every job path is the scope or a descendant; the first visit is the
    // virtual root at the open scope row. Reloaded from the DB: the censor
    // commits are durable (plan20 D3).
    let ledger = Ledger::load_db(&censor).unwrap();
    let in_scope: Vec<&String> = ledger
        .iter()
        .map(|(k, _)| k)
        .filter(|k| *k == "f2f3 e7e6" || k.starts_with("f2f3 e7e6 "))
        .collect();
    assert_eq!(in_scope.len(), 2, "two scoped censors: {in_scope:?}");
    assert!(ledger.contains("f2f3 e7e6"), "virtual root recorded");
    // The out-of-scope record survives untouched.
    assert_eq!(ledger.get("b1c3").unwrap().work_done, 50_000);
    assert_eq!(ledger.get("b1c3").unwrap().passes_failed, 1);
    std::fs::remove_dir_all(&dir).ok();
}

// --- plan21: the OR-node sibling skip ---

/// Plan21 G2 (rule A): a session over a statically or-closed layer visits
/// only the non-pruned records — the or-closed parent (white-to-move, a
/// proven `Loss` child row) and everything under it is never visited, and
/// the dropped ledger records stay untouched (candidacy-only drop).
#[test]
fn walk_or_closed_layer_visits_only_open_records() {
    let dir = walk_dir("walk_orclosed_static");
    // The or-closed parent "a2a3 b1b2" (white-to-move, ply 2) is proven by
    // the crafted loss child at ply 3 (rows are data — but the *visited*
    // paths must replay, and "a2a3"/"a2a3 a7a5" do).
    let db = walk_db(&[
        ("", 0, None),
        ("a2a3", 1, None),
        ("a2a3 b1b2 c2c3", 3, Some(Outcome::Loss)),
    ]);
    let ledger = ledger_of(&[
        ("a2a3", 100, 1),
        ("a2a3 a7a5", 100, 1),
        ("a2a3 b1b2", 100, 1),
        ("a2a3 b1b2 c2c3 e2e4", 100, 1),
    ]);
    let mut session = Session::new(
        4,
        dir.join("shards"),
        std::collections::HashSet::new(),
        "frontier-walk",
    );
    let (mut frontier, census) = WalkFrontier::new(&ledger, &db, "a2a3");
    assert_eq!(
        (census.lineage, census.ancestor, census.or_closed),
        (0, 0, 2)
    );
    let mut led = ledger.clone();
    let summary = run_walk_batch(
        &mut session,
        &db,
        &mut frontier,
        &mut led,
        &censor_mem(),
        1_000,
        &walk_opts(2, 0),
        "a2a3",
    )
    .unwrap();
    assert_eq!(summary.stop_reason, "max-jobs");
    assert_eq!(summary.jobs, 2);
    assert_eq!(summary.or_closes, 0);
    // Visits: the scope-root record, then its lex-first legal black reply
    // (a7a5); nothing at-or-under the or-closed parent. The ledger keeps
    // every seeded record (drops are candidacy-only) — the visited ones are
    // bumped to the budget, the dropped ones untouched.
    let bumped = |led: &Ledger, k: &str| {
        let e = led.get(k).unwrap();
        assert!(e.work_done >= 1_000, "{k} bumped to the budget: {e:?}");
        assert_eq!(e.passes_failed, 2, "{k} censored once");
    };
    bumped(&led, "a2a3");
    bumped(&led, "a2a3 a7a5");
    assert_eq!(led.get("a2a3 b1b2").unwrap().work_done, 100);
    assert_eq!(led.get("a2a3 b1b2 c2c3 e2e4").unwrap().work_done, 100);
    std::fs::remove_dir_all(&dir).ok();
}

/// Plan21 G2 (rule B + D5): an in-session decisive `loss` child or-closes
/// its white-to-move parent: the scope root's expansion target — the
/// win-in-1 knight capture `e5d7` that explodes the black king — decides as
/// a terminal loss, the parent or-closes, and the session exhausts with no
/// further visit at-or-under the parent. The or-close is not a complete
/// (D8); the seeded scope-root record is untouched; a full re-run
/// reproduces the summary and censor state (determinism).
#[test]
fn walk_in_session_or_close_after_decisive_loss() {
    use atomic_solver::notation::move_to_uci;

    let dir = walk_dir("walk_orclosed_dynamic");
    // The scope root: white to move (absolute ply 4), every legal child
    // crafted as a stored open row except the win-in-1 `e5d7` — the walk's
    // expansion target, whose position is a terminal loss for the defender.
    let scope = "g1f3 a7a5 f3e5 a5a4";
    let path: Vec<String> = scope.split(' ').map(str::to_string).collect();
    let (pos, _) = replay_job_path(&path).unwrap();
    let mut ucis: Vec<String> = pos
        .legal_moves_vec()
        .iter()
        .map(|&m| move_to_uci(m))
        .collect();
    ucis.sort();
    assert!(
        ucis.iter().any(|u| u == "e5d7"),
        "win-in-1 is a legal child"
    );
    let rows: Vec<proofdb::db::DbRow> = ucis
        .iter()
        .filter(|u| u.as_str() != "e5d7")
        .map(|u| proofdb::db::DbRow {
            path: format!("{scope} {u}")
                .split(' ')
                .map(str::to_string)
                .collect(),
            ply: 5,
            outcome: None,
            depth_bound: None,
            depth_status: None,
        })
        .collect();
    let db = proofdb::db::DbContent {
        root_fen: Position::STARTPOS_FEN.to_string(),
        built_from: "0".repeat(64),
        rows,
    };
    // The scope root itself: a record at the budget (an expansion candidate;
    // never a search visit — the flat budget is spent on its children).
    let ledger = ledger_of(&[(scope, 1_000, 1)]);

    let run = |censor: &Connection| {
        let mut session = Session::new(
            4,
            dir.join("shards"),
            std::collections::HashSet::new(),
            "frontier-walk",
        );
        let (mut frontier, census) = WalkFrontier::new(&ledger, &db, scope);
        assert_eq!(census.or_closed, 0, "nothing or-closed at session start");
        let mut led = ledger.clone();
        let summary = run_walk_batch(
            &mut session,
            &db,
            &mut frontier,
            &mut led,
            censor,
            1_000,
            &walk_opts(5, 0),
            scope,
        )
        .unwrap();
        (summary, led)
    };
    let (summary, led) = run(&censor_mem());
    // The single expansion job: decisive loss at the win-in-1 child, then
    // the or-close of the scope root exhausts the session.
    assert_eq!(summary.stop_reason, "", "exhausted");
    assert_eq!(summary.jobs, 1);
    assert_eq!(summary.decisive, 1);
    assert_eq!(summary.censored, 0);
    assert_eq!(summary.or_closes, 1);
    assert_eq!(summary.completes, 0, "an or-close is not a complete (D8)");
    assert_eq!(session_new_shards(&dir), 1);
    // The seeded scope-root record is untouched; no record appeared at or
    // under the parent beyond it (the decided subtree was swept before it
    // could gain one).
    assert_eq!(entries(&led), entries(&ledger));
    // Determinism: a fresh session reproduces summary + censor state.
    let (summary2, led2) = run(&censor_mem());
    assert_eq!(summary2.jobs, summary.jobs);
    assert_eq!(summary2.or_closes, summary.or_closes);
    assert_eq!(summary2.completes, summary.completes);
    assert_eq!(entries(&led), entries(&led2));
    std::fs::remove_dir_all(&dir).ok();
}

/// The shard count the or-close driver test's session produced (the
/// durable layer on disk is the session's own record; the two runs share
/// one shard dir, so the bin + sidecar pair count pins the decisive
/// total).
fn session_new_shards(dir: &std::path::Path) -> usize {
    sidecar_pairs(&dir.join("shards"))
}

// --- plan20: censor state in the DB (schema v2; D5/D6) ---

/// S1: the censor table round-trips through `write_db`/`read_censor_rows`,
/// the schema-version row says 2, and the v1 surfaces are unchanged.
#[test]
fn censor_table_roundtrip_and_schema_version_row() {
    let dir = walk_dir("p20_roundtrip");
    let mut pt = proofdb::merge::PathTree::new();
    pt.finalize().unwrap();
    let rows = [
        proofdb::schema::CensorRow {
            path: String::new(),
            work_done: 4_000_000,
            passes_failed: 1,
        },
        proofdb::schema::CensorRow {
            path: "a2a3".to_string(),
            work_done: 0, // a zero-eval censor (repetition) is retained
            passes_failed: 1,
        },
    ];
    let db = dir.join("proofdb.db");
    proofdb::schema::write_db(&db, Position::STARTPOS_FEN, "d", &[], &pt, &rows).unwrap();
    let conn = Connection::open(&db).unwrap();
    let version: String = conn
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(version, "2");
    let got = proofdb::schema::read_censor_rows(&conn).unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].path, ""); // path order: the root first
    assert_eq!((got[0].work_done, got[0].passes_failed), (4_000_000, 1));
    assert_eq!(got[1].path, "a2a3");
    assert_eq!((got[1].work_done, got[1].passes_failed), (0, 1));
    // The ledger view agrees.
    let ledger = Ledger::load_db(&conn).unwrap();
    assert_eq!(ledger.len(), 2);
    assert!(ledger.contains("a2a3"));
    std::fs::remove_dir_all(&dir).ok();
}

/// S1/S3 (D5): reading a schema-v1 DB (no censor table) is empty, not an
/// error — the merger's preserve-and-rebuild must not fail on a pre-v2 file.
#[test]
fn censor_read_on_a_v1_db_is_empty() {
    let dir = walk_dir("p20_v1");
    let conn = Connection::open(dir.join("v1.db")).unwrap();
    conn.execute_batch("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);")
        .unwrap();
    assert!(proofdb::schema::read_censor_rows(&conn).unwrap().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

/// S3 (D5, normative edge case): a rebuild preserves censor rows exactly
/// (row-for-row, including keys never proven) — the merger reads the old
/// DB's rows before replacing the file and re-inserts them into the fresh
/// one; a rebuild on a missing DB starts with an empty censor state.
#[test]
fn rebuild_preserves_censor_rows_exactly() {
    let dir = walk_dir("p20_preserve");
    let mut pt = proofdb::merge::PathTree::new();
    pt.finalize().unwrap();
    let censor_rows = vec![proofdb::schema::CensorRow {
        path: "g1f3 g8f6 f3g1 f6g8".to_string(),
        work_done: 4,
        passes_failed: 2,
    }];
    // Round 1: a DB carrying censor rows.
    let db = dir.join("proofdb.db");
    proofdb::schema::write_db(&db, Position::STARTPOS_FEN, "d", &[], &pt, &censor_rows).unwrap();
    // Round 2: the merge's preserve-and-rebuild (read → replace → insert).
    let old = proofdb::schema::read_censor_rows(&Connection::open(&db).unwrap()).unwrap();
    proofdb::schema::write_db(&db, Position::STARTPOS_FEN, "d", &[], &pt, &old).unwrap();
    let after = proofdb::schema::read_censor_rows(&Connection::open(&db).unwrap()).unwrap();
    assert_eq!(after, censor_rows, "row-for-row preservation");
    // A rebuild on a missing DB → empty censor (fresh layer).
    std::fs::remove_file(&db).unwrap();
    assert!(!db.exists());
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE censor (path TEXT PRIMARY KEY, work_done INTEGER NOT NULL, \
         passes_failed INTEGER NOT NULL);",
    )
    .unwrap();
    assert!(proofdb::schema::read_censor_rows(&conn).unwrap().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

/// S2 integration: censor rows committed by one session are visible to a
/// second session's frontier (pick-up across sessions, the plan20 analogue
/// of the file-ledger gate). Runs through `run_walk_session` — the CLI's
/// own assembly path — on a file-backed censor DB.
#[test]
fn censor_commits_are_visible_to_a_second_session() {
    let dir = walk_dir("p20_second_session");
    let db_path = proofdb::fixture::fixture_db(&dir);
    let db = proofdb::db::load_db_rows(&db_path).unwrap();
    let censor = censor_file(&dir, "censor.db");

    let run = || {
        let mut session = Session::new(
            4,
            dir.join("shards"),
            std::collections::HashSet::new(),
            "frontier-walk",
        );
        run_walk_session(&mut session, &db, &censor, 1_000, &walk_opts(2, 0), "").unwrap()
    };
    let first = run();
    assert_eq!(first.fresh_censored, 2);
    // A second session picks the committed rows up: they are its census
    // records, and its work_before values continue from them (no re-censor
    // from zero).
    let second = run();
    let ledger = Ledger::load_db(&censor).unwrap();
    assert_eq!(ledger.len(), first.fresh_censored + second.fresh_censored);
    assert_eq!(second.jobs, 2);
    for (_, e) in ledger.iter() {
        assert!(
            e.work_done >= 1_000,
            "picked up at its recorded work: {e:?}"
        );
        assert!(e.passes_failed >= 1);
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// S3 (D6 import primitives): a legacy JSON ledger merges under the
/// per-field max rule — idempotent for a repeated import, never decreasing
/// a stored (newer) value, new keys inserted, and a `work_done` beyond the
/// i64 storage width saturates on import. These are the exact primitives
/// `proofdb_merge --import-censor` composes.
#[test]
fn legacy_import_max_rule_idempotence_and_clamp() {
    let dir = walk_dir("p20_import");
    let legacy_path = dir.join("work.json");
    std::fs::write(
        &legacy_path,
        format!(
            r#"{{"records":[
                {{"path":["a2a3"],"work_done":100,"passes_failed":1}},
                {{"path":["b1c3"],"work_done":{},"passes_failed":1}}]}}"#,
            u64::MAX
        ),
    )
    .unwrap();

    // The standing (newer) censor state.
    let mut standing = ledger_of(&[("a2a3", 4_000_000, 2)]);
    let legacy = Ledger::load_legacy_json(&legacy_path).unwrap();
    assert_eq!(legacy.len(), 2);

    // Import: per-field max; the stale record at "a2a3" changes nothing;
    // "b1c3" is new with the clamped work value.
    for (k, e) in legacy.iter() {
        let e = LedgerEntry {
            work_done: e.work_done.min(i64::MAX as u64),
            passes_failed: e.passes_failed,
        };
        standing.upsert_max(k, e);
    }
    assert_eq!(
        standing.get("a2a3"),
        Some(&LedgerEntry {
            work_done: 4_000_000,
            passes_failed: 2
        })
    );
    let b = standing.get("b1c3").unwrap();
    assert_eq!(b.work_done, i64::MAX as u64, "saturating clamp");
    assert_eq!(b.passes_failed, 1);

    // Idempotence: importing the same ledger again changes nothing.
    let before = entries(&standing);
    for (k, e) in Ledger::load_legacy_json(&legacy_path).unwrap().iter() {
        let e = LedgerEntry {
            work_done: e.work_done.min(i64::MAX as u64),
            passes_failed: e.passes_failed,
        };
        standing.upsert_max(k, e);
    }
    assert_eq!(entries(&standing), before, "idempotent import");
    std::fs::remove_dir_all(&dir).ok();
}
