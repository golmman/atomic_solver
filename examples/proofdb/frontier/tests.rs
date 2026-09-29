//! Frontier-extraction unit tests (plan3 D1): class extraction with the
//! AND-completeness assert on a merger-built fixture DB, decision-3
//! manifest disjointness, stale-DB abort. Included by `frontier.rs`; also
//! runs via `tests/proofdb.rs`.

use super::*;
use crate::proofdb::fixture::fixture_db;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("proofdb_fro_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn classes_are_extracted_with_and_assert() {
    let dir = tmp_dir("classes");
    let (db, digest) = fixture_db(&dir);
    let f = extract_frontier(&db, &digest, &HashSet::new()).unwrap();
    assert_eq!(f.root_fen, Position::STARTPOS_FEN);
    assert_eq!(f.n_nodes, 9);
    assert_eq!(f.c1.len(), 5, "root + 4 graft ancestors are open");
    // C1 deepest-first (plan2's rule), root last.
    let c1_paths: Vec<String> = f.c1.iter().map(|j| j.path.join(" ")).collect();
    assert_eq!(c1_paths[0], "f2f3 e7e6");
    assert_eq!(c1_paths[1], "g2g4 e7e6");
    assert_eq!(c1_paths[2], "f2f3");
    assert_eq!(c1_paths[3], "g2g4");
    assert_eq!(c1_paths[4], "");
    // C2: unexpanded children of the two win nodes (bound 1, ply 4); the
    // stored proving child d8h4 is a row and must not appear.
    assert!(!f.c2.is_empty());
    for j in &f.c2 {
        assert_eq!(j.class, JobClass::SharpSibling);
        assert_eq!(j.ply, 4);
        assert_eq!(j.parent_bound, Some(1));
        assert!(!j.path.ends_with(&["d8h4".to_string()]));
    }
    let win_paths: HashSet<String> = f.c2.iter().map(|j| j.path[..3].join(" ")).collect();
    assert_eq!(
        win_paths,
        HashSet::from(["f2f3 e7e6 g2g4".to_string(), "g2g4 e7e6 f2f3".to_string()]),
        "both win nodes contribute C2 jobs"
    );
    // C3: unexpanded children of the open nodes; the root's stored graft
    // moves f2f3/g2g4 are rows and must not appear.
    assert!(f.c3.iter().any(|j| j.path == vec!["d2d4".to_string()]));
    for j in &f.c3 {
        assert_eq!(j.class, JobClass::OpenChild);
        assert!(j.parent_bound.is_none());
    }
    assert!(!f.c3.iter().any(|j| j.path == vec!["f2f3".to_string()]));
    assert!(!f.c3.iter().any(|j| j.path == vec!["g2g4".to_string()]));
    // The AND-completeness assert walked both loss terminals.
    assert_eq!(f.and_checks, 2);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn frontier_paths_reject_manifest_entries() {
    let dir = tmp_dir("manifest");
    let (db, digest) = fixture_db(&dir);
    let f = extract_frontier(&db, &digest, &HashSet::new()).unwrap();
    // C2/C3 paths must not be manifest paths (decision 3).
    let clash = f.c3[0].path.clone();
    let err = extract_frontier(&db, &digest, &HashSet::from([clash.join(" ")])).unwrap_err();
    assert!(err.contains("already has a shard"), "{err}");
    // C1 (open rows) too.
    let err = extract_frontier(&db, &digest, &HashSet::from(["f2f3".to_string()])).unwrap_err();
    assert!(err.contains("already has a shard"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn stale_db_digest_aborts() {
    let dir = tmp_dir("stale");
    let (db, digest) = fixture_db(&dir);
    let stale = format!("{:0>64}", "0");
    assert!(stale != digest);
    let err = extract_frontier(&db, &stale, &HashSet::new()).unwrap_err();
    assert!(err.contains("built_from"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn and_completeness_violation_aborts() {
    // A handcrafted DB whose single proven loss node lacks a stored row for
    // one legal reply — unbuildable by the merger (its fixpoint rejects it),
    // so the DB is written directly here.
    let dir = tmp_dir("andviol");
    let db = dir.join("broken.db");
    let digest = "0".repeat(64);
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(&format!(
        "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
         CREATE TABLE nodes (
           id INTEGER PRIMARY KEY, parent_id INTEGER, ply INTEGER NOT NULL,
           move_uci TEXT, move_code INTEGER, outcome TEXT, depth_bound INTEGER,
           depth_status TEXT, provenance TEXT);
         INSERT INTO meta VALUES ('built_from', '{digest}'), ('root_fen', '{}');
         INSERT INTO nodes VALUES (0, NULL, 0, NULL, NULL, NULL, NULL, NULL, '');
         INSERT INTO nodes VALUES (1, 0, 1, 'e2e4', NULL, 'loss', 1, NULL, 'x');",
        Position::STARTPOS_FEN
    ))
    .unwrap();
    let err = extract_frontier(&db, &digest, &HashSet::new()).unwrap_err();
    assert!(err.contains("AND-completeness"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}
