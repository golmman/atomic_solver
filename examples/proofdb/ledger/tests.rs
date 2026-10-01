//! Work-ledger unit tests (plan4 D1): deterministic bytes, load/save
//! roundtrip, fresh/bump semantics, malformed-input aborts. Included by
//! `ledger.rs`; also runs via `tests/proofdb.rs`.

use super::*;

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("proofdb_ledger_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("proofdb_work.json")
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
fn save_is_deterministic_and_roundtrips() {
    let path = tmp("roundtrip");
    let mut l = Ledger::default();
    l.insert_fresh("g2g4 e7e6 f2f3");
    l.bump("", 4_000_000);
    l.insert_fresh("a2a3");
    l.save(&path).unwrap();
    let bytes1 = std::fs::read(&path).unwrap();
    l.save(&path).unwrap();
    let bytes2 = std::fs::read(&path).unwrap();
    assert_eq!(bytes1, bytes2, "same ledger → same bytes");
    let l2 = Ledger::load(&path).unwrap();
    assert_eq!(l2.len(), 3);
    assert_eq!(
        l2.get("").copied(),
        Some(LedgerEntry {
            work_done: 4_000_000,
            passes_failed: 1,
        })
    );
    assert!(l2.contains("a2a3"));
    assert!(l2.contains("g2g4 e7e6 f2f3"));
    // Sorted path order: the empty path (root) first.
    let s = String::from_utf8(bytes1).unwrap();
    assert!(s.starts_with("{\"records\":[{\"path\":[],"));
    assert!(s.contains("\"path\":[\"a2a3\"]"));
    // Save creates missing parent directories.
    let nested = path.with_extension("json").parent().unwrap().join("sub");
    let p2 = nested.join("l.json");
    l.save(&p2).unwrap();
    assert!(p2.exists());
}

#[test]
fn load_missing_file_is_empty_and_errors_are_aborts() {
    let path = tmp("missing");
    std::fs::remove_file(&path).ok();
    assert!(Ledger::load(&path).unwrap().is_empty(), "fresh init");

    std::fs::write(&path, "{\"records\":[]}").unwrap();
    assert!(Ledger::load(&path).unwrap().is_empty());

    // Malformed JSON and duplicate paths are hard errors.
    std::fs::write(&path, "not json").unwrap();
    assert!(Ledger::load(&path).unwrap_err().contains("not valid JSON"));
    std::fs::write(
        &path,
        r#"{"records":[
            {"path":["a"],"work_done":0,"passes_failed":0},
            {"path":["a"],"work_done":1,"passes_failed":1}]}"#,
    )
    .unwrap();
    assert!(Ledger::load(&path).unwrap_err().contains("duplicate path"));
    std::fs::remove_file(&path).ok();
}
