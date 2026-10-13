//! The manifest writer (plan2 D2): the deterministic required-fields-only
//! manifest format every proofdb tool reads and rewrites. Split from `mod.rs`
//! for the 10 KB file-size convention.
//!
//! Seed provenance extras are intentionally not carried here; they remain
//! reproducible from the plan manifests that produced the entries.

use super::{ShardEntry, digest_hex};

/// Write the manifest at `path`: one JSON object with an `entries` array
/// carrying the **required fields only** (`tag`, `file`, `fen`, `moves`,
/// `outcome`, `validate`), sorted by `tag`, in a fixed field order — the
/// bytes are therefore deterministic for a given entry set, and re-writing
/// an unchanged set reproduces them (and the `built_from` digest) exactly.
/// Every `moves` path and every tag must be unique; duplicates are an error
/// (the manifest contract, enforced at write time as well as read time).
/// Seed provenance extras are intentionally not carried here; they remain
/// reproducible from the plan manifests that produced the entries.
///
/// The bytes are written atomically (plan17 D3, the ledger's pattern):
/// serialize → write `manifest.json.tmp` next to the target → `fs::rename`
/// over the target — a crash mid-write never truncates the manifest (the
/// rename is all-or-nothing), and a leftover `.tmp` from a killed write is
/// never read by anyone and is overwritten by the next write.
///
/// Returns the SHA-256 hex digest of the written bytes (the value the DB's
/// `built_from` meta row will carry after the next merge).
///
/// # Errors
/// Any I/O error, or a duplicate tag/path among `entries`.
pub fn write_manifest(path: &std::path::Path, entries: &[ShardEntry]) -> Result<String, String> {
    let mut sorted: Vec<&ShardEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.tag.cmp(&b.tag));
    let mut seen_tags = std::collections::HashSet::new();
    let mut seen_paths = std::collections::HashSet::new();
    for e in &sorted {
        if !seen_tags.insert(e.tag.clone()) {
            return Err(format!("duplicate tag {:?}", e.tag));
        }
        if !seen_paths.insert(e.moves.join(" ")) {
            return Err(format!("duplicate moves path (tag {:?})", e.tag));
        }
    }
    let doc = serde_json::json!({
        "entries": sorted.iter().map(|e| serde_json::json!({
            "tag": e.tag,
            "file": e.file,
            "fen": e.fen,
            "moves": e.moves,
            "outcome": e.outcome.as_str(),
            "validate": e.validate,
        })).collect::<Vec<_>>(),
    });
    let bytes = serde_json::to_vec_pretty(&doc).map_err(|e| format!("manifest serialize: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes)
        .map_err(|e| format!("cannot write manifest {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("cannot rename manifest {} into place: {e}", path.display()))?;
    Ok(digest_hex(&bytes))
}

#[cfg(test)]
mod tests {
    use super::super::read_manifest;
    use super::*;
    use atomic_solver::position::Outcome;

    #[test]
    fn write_manifest_is_deterministic_and_round_trips() {
        let dir = std::env::temp_dir();
        let p1 = dir.join(format!("proofdb_wm1_{}.json", std::process::id()));
        let p2 = dir.join(format!("proofdb_wm2_{}.json", std::process::id()));
        let entries = vec![
            ShardEntry {
                tag: "h_abc".into(),
                file: "h_abc.bin".into(),
                fen: "F2".into(),
                moves: vec!["a2a3".into()],
                outcome: Outcome::Win,
                validate: "ok".into(),
            },
            ShardEntry {
                tag: "seed".into(),
                file: "seed.bin".into(),
                fen: "F1".into(),
                moves: vec![],
                outcome: Outcome::Loss,
                validate: "ok".into(),
            },
        ];
        let d1 = write_manifest(&p1, &entries).unwrap();
        // Out-of-order construction must not change the bytes.
        let mut reversed = entries.clone();
        reversed.reverse();
        let d2 = write_manifest(&p2, &reversed).unwrap();
        assert_eq!(std::fs::read(&p1).unwrap(), std::fs::read(&p2).unwrap());
        assert_eq!(d1, d2);
        // Round-trip through read_manifest (which sorts by tag itself).
        let m = read_manifest(&p1).unwrap();
        assert_eq!(m.entries.len(), 2);
        assert_eq!(m.entries[0].tag, "h_abc");
        assert_eq!(m.entries[1].tag, "seed");
        assert_eq!(m.sha256_hex, d1);
        // Duplicate tag/path are rejected at write time too.
        let mut dup = entries.clone();
        dup[1].tag = dup[0].tag.clone();
        let p3 = dir.join(format!("proofdb_wm3_{}.json", std::process::id()));
        assert!(
            write_manifest(&p3, &dup)
                .unwrap_err()
                .contains("duplicate tag")
        );
        let mut dup2 = entries.clone();
        dup2[1].moves = dup2[0].moves.clone();
        let p4 = dir.join(format!("proofdb_wm4_{}.json", std::process::id()));
        assert!(
            write_manifest(&p4, &dup2)
                .unwrap_err()
                .contains("duplicate moves path")
        );
        for p in [&p1, &p2, &p3, &p4] {
            std::fs::remove_file(p).ok();
        }
    }

    /// Plan17 task 1: a successful write leaves no `.json.tmp` residue.
    #[test]
    fn write_manifest_leaves_no_tmp_residue() {
        let dir = std::env::temp_dir();
        let p = dir.join(format!("proofdb_wmtmp_{}.json", std::process::id()));
        let entries = vec![ShardEntry {
            tag: "h_x".into(),
            file: "h_x.bin".into(),
            fen: "F".into(),
            moves: vec![],
            outcome: Outcome::Win,
            validate: "ok".into(),
        }];
        write_manifest(&p, &entries).unwrap();
        assert!(!p.with_extension("json.tmp").exists(), "tmp residue");
        std::fs::remove_file(&p).ok();
    }

    /// Plan17 task 1: an I/O failure at the tmp write (simulated root-proof
    /// way: a directory occupying the tmp path — permission bits are not a
    /// portable failure injector under a root test runner) returns the error
    /// and leaves the previous manifest file byte-identical.
    #[test]
    fn write_manifest_io_failure_leaves_previous_file_intact() {
        let dir = std::env::temp_dir();
        let p = dir.join(format!("proofdb_wmfail_{}.json", std::process::id()));
        let old = b"{\"entries\":[]}";
        std::fs::write(&p, old).unwrap();
        // Occupy the tmp path with a directory: the tmp write fails.
        let tmp = p.with_extension("json.tmp");
        std::fs::create_dir(&tmp).unwrap();
        let entries = vec![ShardEntry {
            tag: "h_y".into(),
            file: "h_y.bin".into(),
            fen: "F".into(),
            moves: vec![],
            outcome: Outcome::Loss,
            validate: "ok".into(),
        }];
        let err = write_manifest(&p, &entries).unwrap_err();
        assert!(err.contains("cannot write manifest"), "unexpected: {err}");
        assert_eq!(
            std::fs::read(&p).unwrap(),
            old,
            "previous manifest clobbered"
        );
        std::fs::remove_dir(&tmp).ok();
        std::fs::remove_file(&p).ok();
    }
}
