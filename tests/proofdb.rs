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
use atomic_solver::proof_tree::{ProofNode, ProofTree, validate_proof_tree};

use proofdb::merge::PathTree;
use proofdb::read_manifest;
use proofdb::schema::{ShardRow, canonical_dump, write_db};

const ROOT_FEN: &str = Position::STARTPOS_FEN;
/// After 1.f3 e6 2.g4: Qd8-h4 explodes the White king (win in 1).
const TACTIC_FEN: &str = "rnbqkbnr/pppp1ppp/4p3/8/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2";
const TACTIC_PATH: &[&str] = &["f2f3", "e7e6", "g2g4"];

/// Build the win-in-1 fixture tree (root + terminal child) and serialize it.
fn fixture_bin() -> Vec<u8> {
    let mut pos = Position::from_fen(TACTIC_FEN).unwrap();
    let mv = uci_to_move("d8h4", &pos).expect("d8h4 is legal and wins");
    pos.do_move(mv);
    let child = ProofNode {
        parent: Some(std::num::NonZeroU32::new(1).expect("nonzero")),
        first_child: None,
        next_sibling: None,
        mv,
        hash: 0,
        outcome: Some(Outcome::Loss),
        depth: 0,
    };
    let root = ProofNode {
        parent: None,
        first_child: Some(std::num::NonZeroU32::new(1).expect("nonzero")),
        next_sibling: None,
        mv: atomic_movegen::types::Move::NONE,
        hash: 0,
        outcome: Some(Outcome::Win),
        depth: 1,
    };
    let tree = ProofTree {
        root_fen: TACTIC_FEN.to_string(),
        nodes: vec![root, child],
    };
    assert!(
        validate_proof_tree(&tree).is_ok(),
        "fixture must be a valid proof"
    );
    let mut buf = Vec::new();
    tree.to_bin(&mut buf).expect("fixture serializes");
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
    write_db(&db_path, ROOT_FEN, &manifest.sha256_hex, &rows, &tree, &[]).unwrap();
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
    let dump1 = canonical_dump(ROOT_FEN, &manifest.sha256_hex, &tree);
    let mut tree2 = PathTree::new();
    tree2.graft_path(&moves_from_path(TACTIC_PATH), "fixture");
    let bytes = std::fs::read(dir.join("fixture.bin")).unwrap();
    let shard = ProofTree::from_bin(&mut bytes.as_slice()).unwrap();
    let graft2 = tree2.graft_path(&moves_from_path(TACTIC_PATH), "fixture");
    tree2.overlay_subtree(graft2, &shard, "fixture").unwrap();
    tree2.finalize().unwrap();
    assert_eq!(
        dump1,
        canonical_dump(ROOT_FEN, &manifest.sha256_hex, &tree2)
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

// --- plan12 D3: the derived-DB gate as a repeatable test ---

/// The standing shard set (committed under `docs/plans/proofdb/shards/`)
/// must rebuild the standing DB byte-identically from a clean checkout —
/// the durable-layer gate (initiative constraint 3), pinned here against
/// drift. Digest and node counts are the plan12-audit pinned values
/// (report12 §1.1); a shard-set change is expected to turn this red.
#[test]
fn standing_layer_rebuilds_byte_identical() {
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let shards = crate_root.join("docs/plans/proofdb/shards");
    let manifest_path = shards.join("manifest.json");
    assert!(
        manifest_path.is_file(),
        "standing shard set not found at {} — this test runs inside the\n\
         atomic_solver checkout (it pins the committed durable layer);\n\
         see docs/proofdb_pipeline.md",
        manifest_path.display()
    );
    let manifest = proofdb::read_manifest(&manifest_path).unwrap();
    assert_eq!(manifest.entries.len(), 262, "standing manifest entry count");

    // The proofdb_merge pipeline, faithfully: per shard — validate=='ok',
    // parse + replay-validate, outcome cross-check, startpos path replay,
    // graft + overlay; then finalize, per-graft skeleton re-validation, DB.
    let mut tree = proofdb::merge::PathTree::new();
    let mut rows = Vec::new();
    let mut graft_roots = Vec::new();
    for entry in &manifest.entries {
        assert_eq!(entry.validate, "ok", "only 'ok' enters a merge");
        let bytes = std::fs::read(shards.join(&entry.file))
            .unwrap_or_else(|e| panic!("shard {}: {e}", entry.tag));
        let shard = atomic_solver::proof_tree::ProofTree::from_bin(&mut bytes.as_slice())
            .unwrap_or_else(|e| panic!("shard {}: {e}", entry.tag));
        atomic_solver::proof_tree::validate_proof_tree(&shard)
            .unwrap_or_else(|d| panic!("shard {}: {d:?}", entry.tag));
        assert_eq!(
            shard.nodes[0].outcome,
            Some(entry.outcome),
            "shard {}",
            entry.tag
        );
        let mut pos = Position::from_fen(Position::STARTPOS_FEN).unwrap();
        let moves: Vec<atomic_movegen::types::Move> = entry
            .moves
            .iter()
            .map(|u| {
                let mv = atomic_solver::notation::uci_to_move(u, &pos)
                    .unwrap_or_else(|| panic!("shard {}: illegal {u}", entry.tag));
                pos.do_move(mv);
                mv
            })
            .collect();
        let graft = tree.graft_path(&moves, &entry.tag);
        tree.overlay_subtree(graft, &shard, &entry.tag)
            .unwrap_or_else(|e| panic!("shard {}: {e}", entry.tag));
        graft_roots.push((graft, entry.fen.clone()));
        rows.push(proofdb::schema::ShardRow {
            tag: entry.tag.clone(),
            file: entry.file.clone(),
            sha256: proofdb::digest_hex(&bytes),
            root_fen: shard.root_fen,
            path: entry.moves.join(" "),
            outcome: entry.outcome,
            depth_bound: shard.nodes[0].depth,
            n_nodes: shard.nodes.len(),
        });
    }
    tree.finalize().unwrap();
    for (root_id, fen) in &graft_roots {
        let skeleton = tree.reassemble_skeleton(*root_id, fen).unwrap();
        assert!(
            atomic_solver::proof_tree::validate_proof_tree(&skeleton).is_ok(),
            "skeleton re-validation failed at graft {:?}",
            tree.path_uci(*root_id)
        );
    }

    let dir = std::env::temp_dir().join(format!("proofdb_rebuild_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("rebuilt.db");
    proofdb::schema::write_db(
        &db_path,
        Position::STARTPOS_FEN,
        &manifest.sha256_hex,
        &rows,
        &tree,
        &[],
    )
    .unwrap();

    // The pinned standing DB: byte-identical rebuild + node census. The
    // plan20 schema-v2 fixture digest (the censor table is additive; the
    // pre-plan20 pin was 0d929f4c…).
    assert_eq!(
        proofdb::digest_hex(&std::fs::read(&db_path).unwrap()),
        "406752cc93fb5fd8667615b4720fbd39c20a4df5b014dde97bf8311a22130fb6",
        "rebuilt DB digest (the standing data/proofdb.db)"
    );
    assert_eq!(tree.nodes.len(), 55_703, "merged node count");
    assert_eq!(
        tree.nodes.iter().filter(|n| n.outcome.is_some()).count(),
        55_628,
        "proven node count"
    );
    std::fs::remove_dir_all(&dir).ok();
}

// --- plan12 D1: the stale-DB guard is length-safe on a malformed DB ---

/// A foreign/corrupted DB with a short (or non-ASCII) `built_from` must
/// produce the clean stale-DB abort (an `Err` naming `built_from`), never a
/// slice panic (the pre-plan12 behavior: exit 134, core dump).
#[test]
fn stale_db_guard_is_length_safe_on_malformed_built_from() {
    let dir = std::env::temp_dir().join(format!("proofdb_g1_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (db_path, digest) = proofdb::fixture::fixture_db(&dir);
    for bogus in ["deadbeef", "", "méé-méé-méé-méé-beyond-16-bytes"] {
        let tampered = dir.join("tampered.db");
        std::fs::copy(&db_path, &tampered).unwrap();
        let conn = rusqlite::Connection::open(&tampered).unwrap();
        conn.execute("UPDATE meta SET value=?1 WHERE key='built_from'", [bogus])
            .unwrap();
        let err = proofdb::db::load_db_rows(&tampered, &digest).unwrap_err();
        assert!(
            err.contains("built_from"),
            "message names the meta row: {err}"
        );
        assert!(
            err.contains("stale DB"),
            "stale-DB abort, not a panic: {err}"
        );
        assert!(err.contains(bogus), "value printed in full: {err}");
        std::fs::remove_file(&tampered).ok();
    }
    std::fs::remove_dir_all(&dir).ok();
}

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
        })
        .collect();
    proofdb::db::DbContent {
        root_fen: Position::STARTPOS_FEN.to_string(),
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
    let (db_path, digest) = proofdb::fixture::fixture_db(&dir);
    let manifest = read_manifest(&dir.join("manifest.json")).unwrap();
    let db = proofdb::db::load_db_rows(&db_path, &digest).unwrap();

    let run = |censor: &Connection| {
        let mut session = Session::new(
            4,
            dir.join("shards"),
            dir.join("shards/manifest.json"),
            manifest.entries.clone(),
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
        dir.join("shards/manifest.json"),
        Vec::new(),
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
        dir.join("shards/manifest.json"),
        Vec::new(),
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

/// Plan16 task 5: the fresh-layer boot at a small budget on a temp layer —
/// the R0 bootstrap merge (root-only DB) → capped `frontier-walk` batch →
/// merge → the grown DB has more than the root. The walk does not
/// self-retire (D6), so the batch is eval-capped; 20k budget, 10M cap
/// (measured: the first decisive shard lands at ~4.4M evals).
#[test]
fn walk_boot_from_empty_layer() {
    let dir = walk_dir("walk_boot");
    // The R0 bootstrap shape: empty manifest + root-only DB.
    let manifest_bytes = b"{\"entries\":[]}".to_vec();
    let digest = proofdb::digest_hex(&manifest_bytes);
    std::fs::write(dir.join("manifest.json"), &manifest_bytes).unwrap();
    let mut pt = proofdb::merge::PathTree::new();
    pt.finalize().unwrap();
    proofdb::schema::write_db(
        &dir.join("proofdb.db"),
        Position::STARTPOS_FEN,
        &digest,
        &[],
        &pt,
        &[],
    )
    .unwrap();
    let db = proofdb::db::load_db_rows(&dir.join("proofdb.db"), &digest).unwrap();

    let mut session = Session::new(
        4,
        dir.join("shards"),
        dir.join("shards/manifest.json"),
        Vec::new(),
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
    // Plan17 D1: the per-job sync — the manifest on disk already references
    // every shard the batch wrote, before the post-batch rewrite runs.
    let pre_sync = read_manifest(&dir.join("shards/manifest.json")).unwrap();
    assert_eq!(
        pre_sync.entries.len(),
        summary.decisive,
        "manifest not current pre-rewrite"
    );
    assert!(
        !dir.join("shards/manifest.json.tmp").exists(),
        "tmp residue"
    );
    // The D4 signature over a full session: the censor rows landed in the
    // boot DB (plan20 — the censor state lives in the DB).
    let ledger = Ledger::load_db(&Connection::open(dir.join("proofdb.db")).unwrap()).unwrap();
    assert_eq!(ledger.len(), summary.fresh_censored);
    // The CLI's post-batch manifest rewrite (the batch never writes it).
    session.rewrite_manifest(&dir.join("shards/manifest.json"), &digest);

    // Merge the grown set (the harvest never merges) → DB > 1 node.
    let manifest = read_manifest(&dir.join("shards/manifest.json")).unwrap();
    assert_eq!(manifest.entries.len(), summary.decisive);
    let mut tree = proofdb::merge::PathTree::new();
    let mut rows = Vec::new();
    for entry in &manifest.entries {
        let bytes = std::fs::read(dir.join("shards").join(&entry.file)).unwrap();
        let mut shard = atomic_solver::proof_tree::ProofTree::from_bin(&mut bytes.as_slice())
            .unwrap_or_else(|e| panic!("shard {}: {e}", entry.tag));
        atomic_solver::proof_tree::validate_proof_tree(&shard)
            .unwrap_or_else(|d| panic!("shard {}: {d:?}", entry.tag));
        assert_eq!(shard.nodes[0].outcome, Some(entry.outcome));
        let graft = tree.graft_path(&moves_from_path(&entry.moves), &entry.tag);
        tree.overlay_subtree(graft, &shard, &entry.tag).unwrap();
        rows.push(proofdb::schema::ShardRow {
            tag: entry.tag.clone(),
            file: entry.file.clone(),
            sha256: proofdb::digest_hex(&bytes),
            root_fen: std::mem::take(&mut shard.root_fen),
            path: entry.moves.join(" "),
            outcome: entry.outcome,
            depth_bound: shard.nodes[0].depth,
            n_nodes: shard.nodes.len(),
        });
    }
    tree.finalize().unwrap();
    let grown = dir.join("grown.db");
    proofdb::schema::write_db(
        &grown,
        Position::STARTPOS_FEN,
        &manifest.sha256_hex,
        &rows,
        &tree,
        &[],
    )
    .unwrap();
    let conn = rusqlite::Connection::open(&grown).unwrap();
    let node_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
        .unwrap();
    assert!(node_count > 1, "grown DB has more than the root");
    std::fs::remove_dir_all(&dir).ok();
}

/// Plan16 D3's zero-eval no-progress guard: a ledger record on an on-path
/// repetition (`g1f3 g8f6 f3g1 f6g8` returns to the startpos) censors at
/// zero child-evals — a non-budget Draw that cannot advance the flat
/// budget. The visit bumps the record (passes, +0 work), complete-marks
/// the node, and the walk moves on to the virtual root instead of
/// re-picking the stalled record forever.
#[test]
fn walk_zero_eval_censor_guard_completes_the_node() {
    let dir = walk_dir("walk_repetition");
    let db = walk_db(&[("", 0, None)]);
    let rep = "g1f3 g8f6 f3g1 f6g8";
    let ledger = ledger_of(&[(rep, 4, 1)]);
    let mut session = Session::new(
        4,
        dir.join("shards"),
        dir.join("shards/manifest.json"),
        Vec::new(),
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
    // Visit 1: the repetition record (0 evals, censored, complete-marked);
    // visits 2–4: the virtual root search (C1) and two root expansions.
    assert_eq!(summary.jobs, 4);
    assert_eq!(summary.stop_reason, "max-jobs");
    assert_eq!(summary.censored, 4);
    assert_eq!(summary.completes, 1);
    // The stalled record stays at its 4 evals (the bump added nothing) but
    // keeps its pass count; the root record was created by the root censor.
    let e = led.get(rep).unwrap();
    assert_eq!((e.work_done, e.passes_failed), (4, 2));
    assert!(led.contains(""));
    std::fs::remove_dir_all(&dir).ok();
}

// --- plan17: the per-job crash-consistent manifest sync (D1–D4) ---

/// Plan17 task 3: censored jobs must not touch the manifest file — censors
/// live in the ledger only, so the on-disk manifest bytes are unchanged by
/// a censor-only session (and no `.tmp` residue appears).
#[test]
fn session_censored_jobs_leave_manifest_bytes_untouched() {
    let dir = walk_dir("p17_censored");
    let (db_path, digest) = proofdb::fixture::fixture_db(&dir);
    let manifest = read_manifest(&dir.join("manifest.json")).unwrap();
    let db = proofdb::db::load_db_rows(&db_path, &digest).unwrap();
    let manifest_path = dir.join("shards/manifest.json");
    std::fs::create_dir_all(dir.join("shards")).unwrap();
    // The current manifest content, serialized at the session's sync path.
    let before = std::fs::read(dir.join("manifest.json")).unwrap();
    std::fs::write(&manifest_path, &before).unwrap();

    let mut session = Session::new(
        4,
        dir.join("shards"),
        manifest_path.clone(),
        manifest.entries.clone(),
        "frontier-walk",
    );
    let (mut frontier, mut census) = WalkFrontier::new(&Ledger::default(), &db, "");
    census.root = root_state(&db, &session, "");
    assert_eq!(census.root, RootState::Open);
    let mut led = Ledger::default();
    // Tiny budget on the quiet fixture: every job censors (the walk driver
    // test pins the same shape), so nothing durable is produced.
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
    // The manifest file was never touched.
    assert_eq!(std::fs::read(&manifest_path).unwrap(), before);
    assert!(
        !manifest_path.with_extension("json.tmp").exists(),
        "tmp residue"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// Plan17 task 3 / D4: the interrupted-write window. A session state where
/// the shard `.bin` is already written but the manifest sync fails (the tmp
/// path occupied by a directory — the root-proof failure injector of the
/// writer tests) returns the error: the previous manifest stays intact and
/// the written shard is exactly one unreferenced orphan, overwritten by the
/// retry. (The process-exit abort itself is `fail()` — untestable
/// in-process; the error path of the sync is what carries the contract.)
#[test]
fn session_manifest_write_failure_leaves_previous_manifest_and_one_orphan() {
    use proofdb::session::Session;
    let dir = walk_dir("p17_orphan");
    let shard_dir = dir.join("shards");
    std::fs::create_dir_all(&shard_dir).unwrap();
    let manifest_path = shard_dir.join("manifest.json");
    // The previous (consistent) manifest on disk.
    let old = b"{\"entries\":[]}".to_vec();
    std::fs::write(&manifest_path, &old).unwrap();
    // The in-flight job's shard: written, not yet referenced.
    let orphan = shard_dir.join("h_orphan.bin");
    std::fs::write(&orphan, b"orphan-bytes").unwrap();
    // The failure injector: a directory occupying the tmp path.
    std::fs::create_dir(manifest_path.with_extension("json.tmp")).unwrap();

    let session = Session::new(
        4,
        shard_dir.clone(),
        manifest_path.clone(),
        Vec::new(),
        "frontier-walk",
    );
    let err = session.sync_manifest().unwrap_err();
    assert!(err.contains("cannot write manifest"), "unexpected: {err}");
    // The previous manifest is intact; the shard is the one orphan; the
    // failed tmp attempt changed nothing durable beyond it.
    assert_eq!(std::fs::read(&manifest_path).unwrap(), old);
    assert!(orphan.is_file());
    let m = read_manifest(&manifest_path).unwrap();
    assert_eq!(m.entries.len(), 0);
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
        dir.join("shards/manifest.json"),
        Vec::new(),
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
        dir.join("shards/manifest.json"),
        Vec::new(),
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
    let (db_path, digest) = proofdb::fixture::fixture_db(&dir);
    let manifest = read_manifest(&dir.join("manifest.json")).unwrap();
    let db = proofdb::db::load_db_rows(&db_path, &digest).unwrap();
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
        dir.join("shards/manifest.json"),
        manifest.entries.clone(),
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
        dir.join("shards/manifest.json"),
        Vec::new(),
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
        })
        .collect();
    let db = proofdb::db::DbContent {
        root_fen: Position::STARTPOS_FEN.to_string(),
        rows,
    };
    // The scope root itself: a record at the budget (an expansion candidate;
    // never a search visit — the flat budget is spent on its children).
    let ledger = ledger_of(&[(scope, 1_000, 1)]);

    let run = |censor: &Connection| {
        let mut session = Session::new(
            4,
            dir.join("shards"),
            dir.join("shards/manifest.json"),
            Vec::new(),
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
/// manifest on disk is the session's own record; the two runs share one
/// shard dir, so the file count pins the decisive total).
fn session_new_shards(dir: &std::path::Path) -> usize {
    let manifest = read_manifest(&dir.join("shards/manifest.json")).unwrap();
    manifest.entries.len()
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
    let (db_path, digest) = proofdb::fixture::fixture_db(&dir);
    let manifest = read_manifest(&dir.join("manifest.json")).unwrap();
    let db = proofdb::db::load_db_rows(&db_path, &digest).unwrap();
    let censor = censor_file(&dir, "censor.db");

    let run = || {
        let mut session = Session::new(
            4,
            dir.join("shards"),
            dir.join("shards/manifest.json"),
            manifest.entries.clone(),
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
