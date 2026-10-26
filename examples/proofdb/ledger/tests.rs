//! Censor-state unit tests (plan20 S1/S2): map semantics, DB-backed
//! load/persist roundtrip, the per-censor commit discipline, the legacy
//! JSON import parser, and the max-rule upsert. Included by `ledger.rs`;
//! also runs via `tests/proofdb.rs`.

use super::*;
use std::path::PathBuf;

fn tmp_db(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("proofdb_ledger_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("censor.db")
}

/// A file-backed connection carrying the `censor` table (the schema-v2
/// shape `write_db` produces).
fn censor_db(path: &PathBuf) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE censor (path TEXT PRIMARY KEY, work_done INTEGER NOT NULL, \
         passes_failed INTEGER NOT NULL);",
    )
    .unwrap();
    conn
}

fn entry(work: u64, passes: u32) -> LedgerEntry {
    LedgerEntry {
        work_done: work,
        passes_failed: passes,
    }
}

#[test]
fn fresh_bump_and_entry_semantics() {
    let mut l = Ledger::default();
    assert!(l.is_empty());
    assert!(l.insert_fresh("a b"));
    assert!(!l.insert_fresh("a b"), "second insert is a no-op");
    let e = l.bump("a b", 1_000);
    assert_eq!((e.work_done, e.passes_failed), (1_000, 1));
    let e = l.bump("a b", 500);
    assert_eq!((e.work_done, e.passes_failed), (1_500, 2));
    // Bumping an unknown record creates it (a censored open DB row).
    let e = l.bump("", 4_000_000);
    assert_eq!((e.work_done, e.passes_failed), (4_000_000, 1));
    assert_eq!(l.len(), 2);
    assert!(l.contains(""));
    assert!(l.remove(""));
    assert!(!l.contains(""));
}

#[test]
fn load_db_empty_and_missing_table_is_a_clean_abort() {
    // Schema-v2 shape: the table exists (even empty) → a fresh ledger.
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE censor (path TEXT PRIMARY KEY, work_done INTEGER NOT NULL, \
         passes_failed INTEGER NOT NULL);",
    )
    .unwrap();
    assert!(Ledger::load_db(&conn).unwrap().is_empty());

    // Schema-v1 shape: no censor table → clean abort naming the upgrade.
    let bare = Connection::open_in_memory().unwrap();
    bare.execute_batch("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);")
        .unwrap();
    let err = Ledger::load_db(&bare).unwrap_err();
    assert!(err.contains("no censor table"), "unexpected: {err}");
    assert!(err.contains("proofdb_merge"), "upgrade hint: {err}");
}

#[test]
fn censor_commits_are_durable_per_bump() {
    let path = tmp_db("durable");
    let conn = censor_db(&path);
    let mut l = Ledger::default();
    on_censored(&mut l, &conn, "", 4_000_000).unwrap();
    on_censored(&mut l, &conn, "a2a3", 1_000).unwrap();
    on_censored(&mut l, &conn, "", 500).unwrap();
    assert_eq!(l.get(""), Some(&entry(4_000_500, 2)));
    assert_eq!(l.get("a2a3"), Some(&entry(1_000, 1)));

    // Reopen the file: every censor is already committed (plan20 D3).
    let conn2 = Connection::open(&path).unwrap();
    let l2 = Ledger::load_db(&conn2).unwrap();
    assert_eq!(l2.len(), 2);
    assert_eq!(l2.get(""), Some(&entry(4_000_500, 2)));
    assert_eq!(l2.get("a2a3"), Some(&entry(1_000, 1)));
}

#[test]
fn proven_draw_is_durable_and_not_a_censor() {
    let path = tmp_db("drawn");
    let conn = censor_db(&path);
    let mut l = Ledger::default();
    on_proven_draw(&mut l, &conn, "a2a3 b1c2").unwrap();
    assert!(l.is_drawn("a2a3 b1c2"));
    assert!(!l.contains("a2a3 b1c2"), "no censor record is created");
    assert_eq!(l.len(), 0, "drawn is not a censor record");

    // A fresh session reloads the scheduling mark from the DB.
    let l2 = Ledger::load_db(&Connection::open(&path).unwrap()).unwrap();
    assert!(l2.is_drawn("a2a3 b1c2"));
    assert!(!l2.contains("a2a3 b1c2"));
    assert_eq!(
        l2.drawn_iter().cloned().collect::<Vec<_>>(),
        vec!["a2a3 b1c2"]
    );

    // A pre-step-1 DB without the drawn table still loads (empty drawn set);
    // the first proven draw creates the table on demand.
    let bare = Connection::open_in_memory().unwrap();
    bare.execute_batch(
        "CREATE TABLE censor (path TEXT PRIMARY KEY, work_done INTEGER NOT NULL, \
         passes_failed INTEGER NOT NULL);",
    )
    .unwrap();
    assert!(
        Ledger::load_db(&bare)
            .unwrap()
            .drawn_iter()
            .next()
            .is_none()
    );
    let mut l3 = Ledger::default();
    on_proven_draw(&mut l3, &bare, "").unwrap();
    assert!(l3.is_drawn(""));
}

#[test]
fn censor_commit_failure_aborts_with_consistent_db() {
    let path = tmp_db("commitfail");
    let conn = censor_db(&path);
    let mut l = Ledger::default();
    on_censored(&mut l, &conn, "a2a3", 100).unwrap();

    // The failure injector: a read-only connection to the same file (the
    // commit must fail cleanly, the batch aborts, the DB keeps its last
    // committed state — plan20 D4).
    let ro =
        Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let err = on_censored(&mut l, &ro, "b1c3", 7).unwrap_err();
    assert!(
        err.contains("cannot commit censor row"),
        "unexpected: {err}"
    );
    // The failed censor left no row; the earlier one is intact.
    let l2 = Ledger::load_db(&Connection::open(&path).unwrap()).unwrap();
    assert_eq!(l2.len(), 1);
    assert_eq!(l2.get("a2a3"), Some(&entry(100, 1)));
    assert!(!l2.contains("b1c3"));
}

#[test]
fn censor_overflow_is_a_clean_abort_not_a_wrap() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE censor (path TEXT PRIMARY KEY, work_done INTEGER NOT NULL, \
         passes_failed INTEGER NOT NULL);",
    )
    .unwrap();
    let mut l = Ledger::default();
    l.bump("x", u64::MAX);
    let err = on_censored(&mut l, &conn, "x", 1).unwrap_err();
    assert!(
        err.contains("exceeds the i64 storage width"),
        "unexpected: {err}"
    );
}

#[test]
fn legacy_json_parses_and_rejects_defects() {
    let dir = std::env::temp_dir().join(format!("proofdb_ledger_json_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("work.json");

    // A missing file is an operator error (the import flag names it).
    std::fs::remove_file(&p).ok();
    assert!(
        Ledger::load_legacy_json(&p)
            .unwrap_err()
            .contains("cannot read ledger")
    );

    std::fs::write(&p, "{\"records\":[]}").unwrap();
    assert!(Ledger::load_legacy_json(&p).unwrap().is_empty());

    std::fs::write(
        &p,
        r#"{"records":[
            {"path":[],"work_done":4000000,"passes_failed":1},
            {"path":["a2a3"],"work_done":0,"passes_failed":0}]}"#,
    )
    .unwrap();
    let l = Ledger::load_legacy_json(&p).unwrap();
    assert_eq!(l.get(""), Some(&entry(4_000_000, 1)));
    assert!(l.contains("a2a3"));

    // Malformed JSON and duplicate paths are hard errors.
    std::fs::write(&p, "not json").unwrap();
    assert!(
        Ledger::load_legacy_json(&p)
            .unwrap_err()
            .contains("not valid JSON")
    );
    std::fs::write(
        &p,
        r#"{"records":[
            {"path":["a"],"work_done":0,"passes_failed":0},
            {"path":["a"],"work_done":1,"passes_failed":1}]}"#,
    )
    .unwrap();
    assert!(
        Ledger::load_legacy_json(&p)
            .unwrap_err()
            .contains("duplicate path")
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn upsert_max_is_per_field_idempotent_and_never_decreases() {
    let mut l = Ledger::default();
    // New key: inserted as-is.
    l.upsert_max("a2a3", entry(4_000_000, 1));
    assert_eq!(l.get("a2a3"), Some(&entry(4_000_000, 1)));
    // Stale import (both fields lower): nothing decreases (plan20 §3).
    l.upsert_max("a2a3", entry(100, 0));
    assert_eq!(l.get("a2a3"), Some(&entry(4_000_000, 1)));
    // Independent field maxima both land.
    l.upsert_max("a2a3", entry(8_000_000, 3));
    assert_eq!(l.get("a2a3"), Some(&entry(8_000_000, 3)));
    l.upsert_max("a2a3", entry(9_000_000, 2));
    assert_eq!(l.get("a2a3"), Some(&entry(9_000_000, 3)));
    // Idempotence: importing the same records twice changes nothing.
    let snapshot: Vec<(String, LedgerEntry)> = l.iter().map(|(k, e)| (k.clone(), *e)).collect();
    l.upsert_max("a2a3", entry(9_000_000, 3));
    let after: Vec<(String, LedgerEntry)> = l.iter().map(|(k, e)| (k.clone(), *e)).collect();
    assert_eq!(snapshot, after, "idempotent");
}
