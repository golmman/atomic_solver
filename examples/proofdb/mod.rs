//! Shared protocol for the `proofdb` tooling (`proofdb_merge`,
//! `proofdb_harvest`, `proofdb_verify`).
//!
//! This is example-side tooling for the startpos proof-line database; the
//! product solver (`src/`) is untouched. Since plan22 the SQLite database
//! (schema v2) is the **single source of truth** and grows by the
//! incremental fold; shards are ephemeral transport — self-describing
//! `<tag>.bin` + `<tag>.meta.json` sidecar pairs, deletable after merge —
//! and the censor state (the relocated work ledger — the frontier's censor
//! history, plan20) lives in the DB's `censor` table. The schema and
//! semantics are normative in `docs/spec/global_proof_store.md`.
//!
//! A **manifest** is retired: at merge time, if the legacy file exists, its
//! entries serve as metadata for unmerged tags that lack a sidecar and the
//! merge backfills their sidecars; sidecar metadata wins when both exist.
#![allow(dead_code)]

use atomic_solver::position::Outcome;

pub mod db;
pub mod flip;
pub mod harvest;
pub mod harvest_args;
pub mod ledger;
pub mod manifest;
pub mod merge;
pub mod schema;
pub mod session;
pub mod shard_export;
pub mod sidecar;
pub mod verify;
pub mod walk;

#[cfg(test)]
pub(crate) mod fixture;

/// Generator identification written into the DB `meta` table.
pub const GENERATOR: &str = concat!("proofdb_merge ", env!("CARGO_PKG_VERSION"));

/// Default root for *all* generated proofdb state (plan14): a production
/// run needs no path flags at all — invoked bare, every tool operates
/// wholly inside this gitignored working layer. The committed shard set at
/// `docs/plans/proofdb/shards/` is a development/validation fixture and is
/// never read or written by these defaults.
pub const DEFAULT_DB: &str = "data/proofdb/proofdb.db";
/// Default manifest path (legacy: the merge reads it only when the file
/// exists and uses it as the migration/metadata source, plan22 D2).
pub const DEFAULT_MANIFEST: &str = "data/proofdb/shards/manifest.json";
/// Default shard directory (the ephemeral transport layer; deletable
/// after merge, D8).
pub const DEFAULT_SHARD_DIR: &str = "data/proofdb/shards";
/// Default `proofdb_flip --out` report path (throwaway analysis output).
pub const DEFAULT_FLIP_OUT: &str = "data/proofdb/flip.json";

/// Create the parent directory of an output path (no-op for a bare file
/// name). Only for *write* paths — reading never creates anything (plan14
/// D3: a missing default DB is a clean abort naming the path).
///
/// # Errors
/// Any I/O error, as a message.
pub fn create_parent_dir(path: &std::path::Path) -> Result<(), String> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    Ok(())
}

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

/// The parsed legacy manifest (the merge's migration/metadata source, D2).
#[derive(Debug)]
pub struct Manifest {
    pub entries: Vec<ShardEntry>,
}

/// SHA-256 of `bytes` as lowercase hex.
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

    #[test]
    fn digest_hex_is_sha256() {
        assert_eq!(
            digest_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
