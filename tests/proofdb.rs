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
