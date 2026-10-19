//! The legacy manifest reader (plan22 D2): the manifest is retired as the
//! merge input — each shard carries its own `<tag>.meta.json` sidecar — but
//! the merge still reads an existing manifest file as the migration/
//! metadata source for unmerged tags that lack a sidecar (and backfills
//! their sidecars). The writer (`write_manifest`, plan17's per-job sync
//! machinery) is gone with the manifest; only the read path survives.

use atomic_solver::position::Outcome;
use serde::Deserialize;

use super::{Manifest, ShardEntry};

/// Parse the manifest at `path`, requiring the six per-entry fields of the
/// retired `global_proof_store.md` manifest contract. Entries are returned
/// sorted by `tag` so the merge is independent of manifest order.
///
/// # Errors
/// Any I/O error or a malformed document.
pub fn read_manifest(path: &std::path::Path) -> Result<Manifest, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("cannot read manifest {}: {e}", path.display()))?;
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
    Ok(Manifest { entries })
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
}
