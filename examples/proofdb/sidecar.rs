//! The per-shard metadata sidecar (plan22 D1): since the manifest is
//! retired as the merge input, every shard becomes self-describing. At
//! produce time the harvest writes `<tag>.bin` then `<tag>.meta.json`
//! (atomic tmp + rename, the manifest writer's pattern); the write order is
//! the commit marker:
//!
//! - **a sidecar present ⇒ its bin is complete** — sidecars exist only for
//!   validator-clean shards (the produce pipeline guarantees it, so there
//!   is no `validate` field);
//! - **a bin without a sidecar** is the interrupted job's orphan — skipped
//!   with a note everywhere, deterministically overwritten by the retry;
//! - **a sidecar without a bin** is a durable-layer defect (aborts).
//!
//! `sha256` is the bin's digest — the merge refuses a bin/sidecar mismatch,
//! and the standing layer uses the digest as the idempotence key (same tag,
//! different bytes ⇒ hard abort, never patched).

use std::path::{Path, PathBuf};

use atomic_solver::position::Outcome;

/// One sidecar record: everything the fold needs to load and cross-check a
/// shard, keyed by `tag`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarEntry {
    pub tag: String,
    pub file: String,
    /// The shard root position FEN (cross-checked against the replay).
    pub fen: String,
    /// UCI path from the startpos to the shard root.
    pub moves: Vec<String>,
    pub outcome: Outcome,
    /// SHA-256 hex of the bin bytes.
    pub sha256: String,
}

/// The sidecar file name for a tag.
#[must_use]
pub fn sidecar_file_name(tag: &str) -> String {
    format!("{tag}.meta.json")
}

/// The full sidecar path for a tag inside the shard dir.
#[must_use]
pub fn sidecar_path(shard_dir: &Path, tag: &str) -> PathBuf {
    shard_dir.join(sidecar_file_name(tag))
}

/// Serialize the sidecar (deterministic field order — `serde_json`'s maps
/// are sorted — pretty, trailing newline).
fn serialize(entry: &SidecarEntry) -> Result<Vec<u8>, String> {
    let doc = serde_json::json!({
        "tag": entry.tag,
        "file": entry.file,
        "fen": entry.fen,
        "moves": entry.moves,
        "outcome": entry.outcome.as_str(),
        "sha256": entry.sha256,
    });
    let mut bytes =
        serde_json::to_vec_pretty(&doc).map_err(|e| format!("sidecar serialize: {e}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Write the sidecar atomically: serialize → write `<tag>.meta.json.tmp`
/// next to the target → `fs::rename` over it. A crash mid-write never
/// truncates an existing sidecar (the rename is all-or-nothing) and a
/// leftover `.tmp` is never read by anyone.
///
/// # Errors
/// Any I/O error, as a message.
pub fn write_sidecar(shard_dir: &Path, entry: &SidecarEntry) -> Result<(), String> {
    let path = sidecar_path(shard_dir, &entry.tag);
    let bytes = serialize(entry)?;
    let tmp = shard_dir.join(format!("{}.tmp", sidecar_file_name(&entry.tag)));
    std::fs::write(&tmp, &bytes)
        .map_err(|e| format!("cannot write sidecar {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("cannot rename sidecar {} into place: {e}", path.display()))?;
    Ok(())
}

fn field_str(v: &serde_json::Value, field: &str) -> Result<String, String> {
    v.as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("sidecar field {field:?} must be a string"))
}

/// Parse a sidecar file.
///
/// # Errors
/// Any I/O error or a malformed document (missing/mistyped field, bad
/// outcome).
pub fn read_sidecar(path: &Path) -> Result<SidecarEntry, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("cannot read sidecar {}: {e}", path.display()))?;
    let raw: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&bytes)
        .map_err(|e| format!("sidecar {} is not valid JSON: {e}", path.display()))?;
    let get = |field: &str| -> Result<&serde_json::Value, String> {
        raw.get(field)
            .ok_or_else(|| format!("sidecar {} misses field {field:?}", path.display()))
    };
    let tag = field_str(get("tag")?, "tag")?;
    let file = field_str(get("file")?, "file")?;
    let fen = field_str(get("fen")?, "fen")?;
    let sha256 = field_str(get("sha256")?, "sha256")?;
    let outcome_s = field_str(get("outcome")?, "outcome")?;
    let outcome = outcome_s
        .parse::<Outcome>()
        .map_err(|_| format!("sidecar {}: bad outcome {outcome_s:?}", path.display()))?;
    let moves = get("moves")?
        .as_array()
        .ok_or_else(|| {
            format!(
                "sidecar {}: field \"moves\" must be an array",
                path.display()
            )
        })?
        .iter()
        .map(|m| field_str(m, "moves[]"))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SidecarEntry {
        tag,
        file,
        fen,
        moves,
        outcome,
        sha256,
    })
}

/// The shard-dir scan both the fold and the unmerged-shard gate run: bin
/// tags (files ending `.bin`, stem = tag) sorted, and sidecar tags (files
/// ending exactly `.meta.json`; a leftover `.tmp` from a crashed sidecar
/// write is ignored — the bin is then an orphan, D1's skip-with-note).
#[derive(Debug, Default)]
pub struct DirScan {
    /// Bin tags, sorted (the fold's deterministic processing order).
    pub bin_tags: Vec<String>,
    /// Sidecar tags (sorted set).
    pub sidecar_tags: std::collections::BTreeSet<String>,
}

/// Scan the shard dir.
///
/// # Errors
/// Any directory-read I/O error.
pub fn scan_shard_dir(shard_dir: &Path) -> Result<DirScan, String> {
    let mut scan = DirScan::default();
    let entries = std::fs::read_dir(shard_dir)
        .map_err(|e| format!("cannot read shard dir {}: {e}", shard_dir.display()))?;
    for e in entries {
        let e = e.map_err(|e| format!("cannot read shard dir {}: {e}", shard_dir.display()))?;
        let name = e.file_name();
        let Some(name) = name.to_str() else { continue };
        if let Some(tag) = name.strip_suffix(".bin") {
            scan.bin_tags.push(tag.to_string());
        } else if let Some(tag) = name.strip_suffix(".meta.json") {
            scan.sidecar_tags.insert(tag.to_string());
        }
    }
    scan.bin_tags.sort();
    Ok(scan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomic_solver::position::Outcome;

    fn entry() -> SidecarEntry {
        SidecarEntry {
            tag: "h_abc".into(),
            file: "h_abc.bin".into(),
            fen: "FEN".into(),
            moves: vec!["a2a3".into()],
            outcome: Outcome::Win,
            sha256: "d".repeat(64),
        }
    }

    fn workdir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("proofdb_sidecar_{name}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn round_trips_and_is_deterministic() {
        let dir = workdir("roundtrip");
        write_sidecar(&dir, &entry()).unwrap();
        let p = sidecar_path(&dir, "h_abc");
        let got = read_sidecar(&p).unwrap();
        assert_eq!(got, entry());
        let bytes1 = std::fs::read(&p).unwrap();
        write_sidecar(&dir, &entry()).unwrap();
        assert_eq!(bytes1, std::fs::read(&p).unwrap(), "deterministic bytes");
        assert!(!p.with_extension("meta.json.tmp").exists(), "tmp residue");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_rejects_missing_and_malformed_fields() {
        let dir = workdir("rejects");
        assert!(read_sidecar(&sidecar_path(&dir, "h_missing")).is_err());
        let bad = dir.join("h_bad.meta.json");
        std::fs::write(&bad, br#"{"tag":"h_bad","fen":"F"}"#).unwrap();
        let err = read_sidecar(&bad).unwrap_err();
        assert!(err.contains("file"), "unexpected: {err}");
        std::fs::write(&bad, br#"{"tag":"h_bad","file":"h_bad.bin","fen":"F","moves":[],"outcome":"wat","sha256":"x"}"#)
            .unwrap();
        assert!(read_sidecar(&bad).unwrap_err().contains("outcome"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// An I/O failure at the tmp write (a directory occupying the tmp path
    /// — the root-proof failure injector) returns the error and leaves the
    /// previous sidecar byte-identical.
    #[test]
    fn write_failure_leaves_previous_file_intact() {
        let dir = workdir("iofail");
        let old = br#"{"old":1}"#;
        let path = sidecar_path(&dir, "h_abc");
        std::fs::write(&path, old).unwrap();
        let tmp = dir.join(format!("{}.tmp", sidecar_file_name("h_abc")));
        std::fs::create_dir(&tmp).unwrap();
        let err = write_sidecar(&dir, &entry()).unwrap_err();
        assert!(err.contains("cannot write sidecar"), "unexpected: {err}");
        assert_eq!(std::fs::read(&path).unwrap(), old, "previous clobbered");
        std::fs::remove_dir(&tmp).ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_sees_bins_and_sidecars_and_ignores_tmp_and_other_files() {
        let dir = workdir("scan");
        std::fs::write(dir.join("h_b.bin"), b"").unwrap();
        std::fs::write(dir.join("h_a.bin"), b"").unwrap();
        std::fs::write(dir.join("h_a.meta.json"), b"{}").unwrap();
        std::fs::write(dir.join("h_c.meta.json"), b"{}").unwrap();
        // Crash residue and unrelated files are ignored.
        std::fs::write(dir.join("h_d.meta.json.tmp"), b"{}").unwrap();
        std::fs::write(dir.join("manifest.json"), b"{}").unwrap();
        let scan = scan_shard_dir(&dir).unwrap();
        assert_eq!(scan.bin_tags, vec!["h_a".to_string(), "h_b".to_string()]);
        assert_eq!(
            scan.sidecar_tags.into_iter().collect::<Vec<_>>(),
            vec!["h_a".to_string(), "h_c".to_string()]
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
