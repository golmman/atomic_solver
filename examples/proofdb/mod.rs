//! Shared protocol for the `proofdb` tooling (`proofdb_merge`,
//! `proofdb_harvest`).
//!
//! This is example-side tooling for the startpos proof-line database; the
//! product solver (`src/`) is untouched. The durable truth is the validated
//! shard set (binary proof trees, `docs/spec/proof_tree_dump.md`) plus its
//! manifest; the SQLite database is a derived, rebuildable view whose schema
//! and semantics are normative in `docs/spec/global_proof_store.md`.
//!
//! A **manifest** lists the shards to merge. Required per-entry fields are
//! `tag`, `file`, `fen`, `moves` (UCI path from the startpos to the shard
//! root), `outcome` (`win`/`loss`, side-to-move perspective), and `validate`
//! (only `ok` enters a merge). Unknown extra fields are ignored.
#![allow(dead_code)]

use atomic_solver::position::Outcome;
use serde::Deserialize;

pub mod harvest;
pub mod manifest;
pub mod merge;
pub mod schema;
pub mod session;
pub mod shard_export;

pub use manifest::write_manifest;

/// Generator identification written into the DB `meta` table.
pub const GENERATOR: &str = concat!("proofdb_merge ", env!("CARGO_PKG_VERSION"));

/// One manifest entry (the required subset; extra JSON fields are ignored).
#[derive(Debug, Clone)]
pub struct ShardEntry {
    pub tag: String,
    pub file: String,
    pub fen: String,
    pub moves: Vec<String>,
    pub outcome: Outcome,
    pub validate: String,
}

/// The parsed manifest: the shard list in file order plus the raw bytes'
/// SHA-256 digest (recorded as the DB `built_from` meta row).
#[derive(Debug)]
pub struct Manifest {
    pub entries: Vec<ShardEntry>,
    pub sha256_hex: String,
}

#[derive(Deserialize)]
struct RawEntry {
    tag: serde_json::Value,
    file: serde_json::Value,
    fen: serde_json::Value,
    moves: serde_json::Value,
    outcome: serde_json::Value,
    validate: serde_json::Value,
}

#[derive(Deserialize)]
struct RawManifest {
    entries: Vec<RawEntry>,
}

fn as_str(v: &serde_json::Value, field: &str, tag: &str) -> Result<String, String> {
    v.as_str().map_or_else(
        || {
            Err(format!(
                "manifest entry {tag:?}: field {field:?} must be a string"
            ))
        },
        |s| Ok(s.to_string()),
    )
}

/// Parse the manifest at `path`, requiring the six per-entry fields of the
/// `global_proof_store.md` manifest contract. Entries are returned sorted by
/// `tag` so the merge is independent of manifest order.
pub fn read_manifest(path: &std::path::Path) -> Result<Manifest, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("cannot read manifest {}: {e}", path.display()))?;
    let digest = digest_hex(&bytes);
    let raw: RawManifest = serde_json::from_slice(&bytes)
        .map_err(|e| format!("manifest {} is not valid JSON: {e}", path.display()))?;
    let mut entries = Vec::with_capacity(raw.entries.len());
    for e in raw.entries {
        let tag = as_str(&e.tag, "tag", "?")?;
        let file = as_str(&e.file, "file", &tag)?;
        let fen = as_str(&e.fen, "fen", &tag)?;
        let validate = as_str(&e.validate, "validate", &tag)?;
        let outcome = {
            let s = as_str(&e.outcome, "outcome", &tag)?;
            s.parse::<Outcome>()
                .map_err(|_| format!("manifest entry {tag:?}: bad outcome {s:?}"))?
        };
        let moves = e
            .moves
            .as_array()
            .ok_or_else(|| format!("manifest entry {tag:?}: field \"moves\" must be an array"))?
            .iter()
            .map(|m| as_str(m, "moves[]", &tag))
            .collect::<Result<Vec<_>, _>>()?;
        entries.push(ShardEntry {
            tag,
            file,
            fen,
            moves,
            outcome,
            validate,
        });
    }
    entries.sort_by(|a, b| a.tag.cmp(&b.tag));
    let mut seen = std::collections::HashSet::new();
    let mut paths = std::collections::HashSet::new();
    for e in &entries {
        if !seen.insert(e.tag.clone()) {
            return Err(format!("manifest has duplicate tag {:?}", e.tag));
        }
        if !paths.insert(e.moves.join(" ")) {
            return Err(format!(
                "manifest has a duplicate moves path (tag {:?})",
                e.tag
            ));
        }
    }
    Ok(Manifest {
        entries,
        sha256_hex: digest,
    })
}

/// SHA-256 of `bytes` as lowercase hex (manifest digest for `built_from`).
#[must_use]
pub fn digest_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("proofdb_test_{}_{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn manifest_requires_fields_and_sorts_by_tag() {
        let p = write_tmp(
            "ok.json",
            r#"{"n":1,"entries":[
                {"tag":"b","file":"b.bin","fen":"F2","moves":["a2a3"],"outcome":"win","validate":"ok","extra":7},
                {"tag":"a","file":"a.bin","fen":"F1","moves":[],"outcome":"loss","validate":"ok"}]}"#,
        );
        let m = read_manifest(&p).unwrap();
        assert_eq!(m.entries.len(), 2);
        assert_eq!(m.entries[0].tag, "a");
        assert!(m.entries[0].moves.is_empty());
        assert_eq!(m.entries[1].moves, vec!["a2a3".to_string()]);
        assert_eq!(m.sha256_hex.len(), 64);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn manifest_rejects_missing_moves_and_duplicate_tags() {
        let p = write_tmp(
            "nomoves.json",
            r#"{"entries":[{"tag":"a","file":"a.bin","fen":"F","outcome":"win","validate":"ok"}]}"#,
        );
        assert!(read_manifest(&p).unwrap_err().contains("moves"));
        let p = write_tmp(
            "duptag.json",
            r#"{"entries":[
                {"tag":"a","file":"x.bin","fen":"F","moves":[],"outcome":"win","validate":"ok"},
                {"tag":"a","file":"y.bin","fen":"F","moves":[],"outcome":"win","validate":"ok"}]}"#,
        );
        assert!(read_manifest(&p).unwrap_err().contains("duplicate tag"));
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn digest_hex_is_sha256() {
        assert_eq!(
            digest_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
