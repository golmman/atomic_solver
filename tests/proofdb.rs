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
    write_db(&db_path, ROOT_FEN, &manifest.sha256_hex, &rows, &tree).unwrap();
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let schema_version: String = conn
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "1");
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

// --- plan7 D1: the N-way ledger union (examples/proofdb/ledger_union.rs) ---

use proofdb::ledger::{Ledger, LedgerEntry};
use proofdb::ledger_union::union;

/// Build a ledger from `(space-joined path key, work_done, passes_failed)`
/// records (the key "" is the root, exactly as `Ledger::load` produces).
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

/// §2.2 coverage: for every input and every path in it, the union's record
/// has `passes_failed ≥` that input's — and at equal passes,
/// `work_done ≥` too (the same-pass max).
fn assert_coverage(union_led: &Ledger, inputs: &[Ledger]) {
    for input in inputs {
        for (k, e) in input.iter() {
            let u = union_led
                .get(k)
                .unwrap_or_else(|| panic!("coverage: missing {k:?}"));
            assert!(
                u.passes_failed > e.passes_failed
                    || (u.passes_failed == e.passes_failed && u.work_done >= e.work_done),
                "coverage violated at {k:?}: input {e:?}, union {u:?}"
            );
        }
    }
}

#[test]
fn union_rule_edges_pass_upgrade_and_work_tiebreak() {
    // Pass upgrade beats any work_done at a lower pass; the winning pass's
    // work is kept (a lower pass's higher work does not leak through).
    let a = ledger_of(&[("", 0, 0), ("a2a3", 100_000_000, 0)]);
    let b = ledger_of(&[("", 4_000_000, 1), ("a2a3", 4_000_000, 1)]);
    let (u, _) = union(&[a, b]);
    assert_eq!(
        u.get(""),
        Some(&LedgerEntry {
            work_done: 4_000_000,
            passes_failed: 1
        })
    );
    assert_eq!(
        u.get("a2a3"),
        Some(&LedgerEntry {
            work_done: 4_000_000,
            passes_failed: 1
        })
    );

    // Pass tie → the higher work_done wins.
    let a = ledger_of(&[("b1c3", 8_000_000, 1)]);
    let b = ledger_of(&[("b1c3", 4_000_000, 1)]);
    let (u, _) = union(&[a, b]);
    assert_eq!(
        u.get("b1c3"),
        Some(&LedgerEntry {
            work_done: 8_000_000,
            passes_failed: 1
        })
    );

    // Full pass+work tie → either record (identical) stands.
    let a = ledger_of(&[("b1c3", 8_000_000, 1)]);
    let (u, st) = union(&[a.clone(), a]);
    assert_eq!(
        u.get("b1c3"),
        Some(&LedgerEntry {
            work_done: 8_000_000,
            passes_failed: 1
        })
    );
    assert_eq!(st.union_records, 1);
    assert_eq!(st.new_paths, [0, 0]); // known in both inputs
    assert_eq!(st.sole_pass_upgrades, [0, 0]); // no strict pass upgrade anywhere
}

#[test]
fn union_disjoint_paths_and_nway_order_independence() {
    let a = ledger_of(&[("", 0, 0), ("a2a3", 4_000_000, 1)]);
    let b = ledger_of(&[("b1c3", 0, 0)]);
    let c = ledger_of(&[("a2a3 a7a5", 4_000_009, 1), ("c2c3", 4_000_000, 1)]);
    let base = union(&[a.clone(), b.clone(), c.clone()]).0;
    assert_eq!(base.len(), 5);
    // Every input order (and pairwise repetition) gives the same records.
    for perm in [
        vec![a.clone(), b.clone(), c.clone()],
        vec![c.clone(), b.clone(), a.clone()],
        vec![b.clone(), a.clone(), c.clone()],
        vec![c.clone(), a.clone(), b.clone()],
        vec![a.clone(), a.clone(), b.clone(), c.clone(), a.clone()],
    ] {
        let (u, _) = union(&perm);
        assert_eq!(entries(&u), entries(&base), "order dependence");
    }
    assert_coverage(&base, &[a.clone(), b.clone(), c.clone()]);
    // Attribution: a and c each solely know one path; c holds the sole
    // pass upgrade at "a2a3 a7a5"? No — no other input knows it, so it is
    // a new path, not an upgrade.
    let (_, st) = union(&[a, b, c.clone()]);
    assert_eq!(st.new_paths, [2, 1, 2]); // ""+a2a3 / b1c3 / a2a3 a7a5+c2c3
    assert_eq!(st.sole_pass_upgrades, [0, 0, 0]);
}

#[test]
fn union_idempotent_and_normalize_single_input() {
    let a = ledger_of(&[
        ("", 0, 0),
        ("a2a3", 4_000_000, 1),
        ("a2a3 a7a5", 4_000_009, 1),
    ]);
    let b = ledger_of(&[("a2a3", 8_000_000, 2), ("c2c3", 0, 0)]);
    let (u, _) = union(&[a.clone(), b.clone()]);
    // Idempotence: union(u, inputs) = u (record-set equality).
    let (u2, st2) = union(&[u.clone(), a.clone(), b.clone()]);
    assert_eq!(entries(&u2), entries(&u));
    assert_eq!(st2.union_records, 4);
    assert_eq!(st2.new_paths, [0, 0, 0]);
    assert_eq!(st2.sole_pass_upgrades, [0, 0, 0]);
    assert_coverage(&u2, &[u.clone(), a.clone(), b.clone()]);
    // N = 1 normalizes: the record set passes through unchanged.
    let (one, st) = union(std::slice::from_ref(&a));
    assert_eq!(entries(&one), entries(&a));
    assert_eq!(st.union_records, 3);
    assert_eq!(st.new_paths, [3]);
    // Empty input set: the empty ledger (the CLI requires N ≥ 1; the
    // function is total).
    let (empty, st) = union(&[]);
    assert_eq!(empty.len(), 0);
    assert_eq!(st.union_records, 0);
}

#[test]
fn union_ledger_save_roundtrip_is_deterministic() {
    let dir = std::env::temp_dir().join(format!("proofdb_union_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = ledger_of(&[("", 0, 0), ("a2a3", 4_000_000, 1), ("a2a3 a7a5", 0, 0)]);
    let b = ledger_of(&[("a2a3", 8_000_000, 2)]);
    let (u, _) = union(&[a, b]);
    u.save(&dir.join("u1.json")).unwrap();
    // A rebuild from the written bytes + a second save are byte-identical
    // (the gate's determinism check, at fixture scale).
    let reloaded = Ledger::load(&dir.join("u1.json")).unwrap();
    reloaded.save(&dir.join("u2.json")).unwrap();
    let b1 = std::fs::read(dir.join("u1.json")).unwrap();
    let b2 = std::fs::read(dir.join("u2.json")).unwrap();
    assert_eq!(b1, b2);
    // The union's record: the pass-2 record wins, "a2a3 a7a5" stays fresh.
    let got: Vec<_> = reloaded
        .iter()
        .map(|(k, e)| (k.clone(), e.passes_failed))
        .collect();
    assert_eq!(
        got,
        vec![
            ("".to_string(), 0),
            ("a2a3".to_string(), 2),
            ("a2a3 a7a5".to_string(), 0),
        ]
    );
    std::fs::remove_dir_all(&dir).ok();
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
    )
    .unwrap();

    // The pinned standing DB: byte-identical rebuild + node census.
    assert_eq!(
        proofdb::digest_hex(&std::fs::read(&db_path).unwrap()),
        "0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8",
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
    use proofdb::{
        DEFAULT_DB, DEFAULT_FLIP_OUT, DEFAULT_LEDGER, DEFAULT_MANIFEST, DEFAULT_SHARD_DIR,
    };

    assert_eq!(DEFAULT_DB, "data/proofdb/proofdb.db");
    assert_eq!(DEFAULT_MANIFEST, "data/proofdb/shards/manifest.json");
    assert_eq!(DEFAULT_SHARD_DIR, "data/proofdb/shards");
    assert_eq!(DEFAULT_LEDGER, "data/proofdb/work.json");
    assert_eq!(DEFAULT_FLIP_OUT, "data/proofdb/flip.json");

    for path in [
        DEFAULT_DB,
        DEFAULT_MANIFEST,
        DEFAULT_SHARD_DIR,
        DEFAULT_LEDGER,
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

use proofdb::walk::driver::{WalkOptions, run_walk_batch, run_walk_session};
use proofdb::walk::{RootState, WalkFrontier, root_state};

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
/// `expand`); the ledger grows by exactly the censor count (the D4
/// signature) and a fresh run reproduces the post-run ledger bytes.
#[test]
fn walk_driver_root_expansion_and_ledger_growth() {
    let dir = walk_dir("walk_driver");
    let (db_path, digest) = proofdb::fixture::fixture_db(&dir);
    let manifest = read_manifest(&dir.join("manifest.json")).unwrap();
    let db = proofdb::db::load_db_rows(&db_path, &digest).unwrap();

    let run = |ledger_path: &std::path::Path| {
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
            ledger_path,
            1_000,
            &walk_opts(5, 0),
            "",
        )
        .unwrap();
        (summary, ledger)
    };
    let (summary, ledger) = run(&dir.join("ledger.json"));
    assert_eq!(summary.jobs, 5);
    assert_eq!(summary.stop_reason, "max-jobs");
    assert_eq!(summary.decisive, 0);
    assert_eq!(summary.censored, 5);
    // The D4 signature: ledger growth == fresh-node censor count.
    assert_eq!(summary.fresh_censored, 5);
    assert_eq!(ledger.len(), 5);
    // Visit 1: the virtual root (class C1, kind search, pass 1,
    // work_before 0); visits 2+: the root's expansion targets (class C3,
    // kind expand), lex order — and the fixture's stored rows (f2f3, g2g4)
    // are skipped as handled territory. The ledger keys pin the sequence.
    let keys: Vec<String> = ledger.iter().map(|(k, _)| k.clone()).collect();
    assert_eq!(keys, vec!["", "a2a3", "a2a4", "b1a3", "b1c3"]);
    let root = ledger.get("").unwrap();
    assert!(
        root.work_done >= 1_000,
        "root censor spent at least the budget"
    );
    assert_eq!(root.passes_failed, 1);
    // Determinism: a fresh session over the same inputs reproduces the
    // post-run ledger byte-for-byte.
    let ledger_path2 = dir.join("ledger2.json");
    let (summary2, ledger2) = run(&ledger_path2);
    assert_eq!(summary2.jobs, summary.jobs);
    assert_eq!(entries(&ledger), entries(&ledger2));
    assert_eq!(
        std::fs::read(dir.join("ledger.json")).unwrap(),
        std::fs::read(&ledger_path2).unwrap()
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
        &dir.join("ledger.json"),
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
        &dir.join("ledger.json"),
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
    let summary = run_walk_session(
        &mut session,
        &db,
        &dir.join("work.json"),
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
    // The D4 signature over a full session.
    let ledger = Ledger::load(&dir.join("work.json")).unwrap();
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
        &dir.join("ledger.json"),
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
        &dir.join("ledger.json"),
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
        &dir.join("ledger.json"),
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
        &dir.join("ledger.json"),
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
    let ledger_path = dir.join("work.json");
    let pre = b"{\"records\":[{\"path\":[\"b1c3\"],\"work_done\":50000,\"passes_failed\":1}]}";
    std::fs::write(&ledger_path, pre).unwrap();

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
        &ledger_path,
        1_000,
        &walk_opts(2, 0),
        "f2f3 e7e6",
    )
    .unwrap();
    assert_eq!(summary.stop_reason, "max-jobs");
    assert_eq!(summary.jobs, 2);
    // Every job path is the scope or a descendant; the first visit is the
    // virtual root at the open scope row.
    let ledger = Ledger::load(&ledger_path).unwrap();
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
