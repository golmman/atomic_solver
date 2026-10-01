//! The sidecar work ledger (plan4 D1): the pick-up state of the
//! breadth-first PNS harvester, kept next to the working DB under `data/`
//! (gitignored working layer). One record per **known-open frontier node**
//! — a node the harvester has exposed (censor-time frontier exposure,
//! plan4 decision 11) or censored — with the non-derivable selection state:
//! how many passes failed at it and how much work was spent. Record
//! presence = known-open; the numbers themselves are *derived* at session
//! start from the DB tree + this ledger (see [`super::pns`]) and never
//! enter the DB (schema v1 stands) or the census facts (decision 3).
//!
//! Lineage gate (decision 6, enforced by [`super::pns::Pns::build`] which
//! owns the DB rows): a record whose path is now a *proven* DB row is
//! dropped (the node was decided after the ledger was written); a record
//! whose path does not replay legally from the startpos aborts the session
//! (the plan2 replay contract, now covering persisted state). Records on
//! open rows are kept (the row is undecided; the record carries its censor
//! history).
//!
//! The file is rewritten between jobs (crash-safe: a crash loses only the
//! in-flight job, never the exposed frontier) with deterministic, sorted
//! bytes — the same DB + the same ledger reproduce the same job sequence
//! (gate H5).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

/// One known-open frontier record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LedgerEntry {
    /// Cumulative child-evals spent at this node (all passes).
    pub work_done: u64,
    /// Number of censored (budget-exhausted) visits so far; the k-th visit
    /// of the node runs at the geometric-ladder budget `2^(k-1) × base`.
    pub passes_failed: u32,
}

/// The parsed ledger: path key (space-joined UCI, empty for the root) →
/// entry, in a `BTreeMap` so the serialized bytes are deterministic.
#[derive(Debug, Default)]
pub struct Ledger {
    entries: BTreeMap<String, LedgerEntry>,
}

#[derive(Deserialize)]
struct RawLedger {
    records: Vec<RawRecord>,
}

#[derive(Deserialize)]
struct RawRecord {
    path: Vec<String>,
    work_done: u64,
    passes_failed: u32,
}

impl Ledger {
    /// Load the ledger at `path`; a missing file is an empty (fresh) ledger.
    ///
    /// # Errors
    /// Any I/O error or malformed JSON (with the offending path/message).
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default_ok(),
            Err(e) => return Err(format!("cannot read ledger {}: {e}", path.display())),
        };
        let raw: RawLedger = serde_json::from_slice(&bytes)
            .map_err(|e| format!("ledger {} is not valid JSON: {e}", path.display()))?;
        let mut entries = BTreeMap::new();
        for r in raw.records {
            let key = r.path.join(" ");
            let e = LedgerEntry {
                work_done: r.work_done,
                passes_failed: r.passes_failed,
            };
            if entries.insert(key.clone(), e).is_some() {
                return Err(format!("ledger has a duplicate path {key:?}"));
            }
        }
        Ok(Self { entries })
    }

    fn default_ok() -> Result<Self, String> {
        Ok(Self::default())
    }

    /// The record at `key`, if any.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&LedgerEntry> {
        self.entries.get(key)
    }

    /// Is `key` a known-open record?
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// Number of records (census).
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate `(key, entry)` in sorted path order (the gate walks this).
    pub fn iter(&self) -> impl Iterator<Item = (&String, &LedgerEntry)> {
        self.entries.iter()
    }

    /// Remove the record at `key` (the lineage gate's decided-path drop).
    /// Returns whether a record was removed.
    pub fn remove(&mut self, key: &str) -> bool {
        self.entries.remove(key).is_some()
    }

    /// Insert a fresh record (`work_done = 0`, `passes_failed = 0`) — the
    /// censor-time exposure of a new frontier child. Returns `false` (and
    /// changes nothing) if a record already exists.
    pub fn insert_fresh(&mut self, key: &str) -> bool {
        self.entries
            .insert(key.to_string(), LedgerEntry::default())
            .is_none()
    }

    /// Bump the record at `key` with one censored visit: `passes_failed += 1`,
    /// `work_done += work`. Creates the record if absent (a censored open
    /// DB row gets its record on first censor). Returns the new entry.
    pub fn bump(&mut self, key: &str, work: u64) -> LedgerEntry {
        let e = self.entries.entry(key.to_string()).or_default();
        e.passes_failed += 1;
        e.work_done = e.work_done.saturating_add(work);
        *e
    }

    /// Rewrite the ledger file with deterministic sorted bytes (temp file +
    /// rename, so a crash never leaves a half-written ledger).
    ///
    /// # Errors
    /// Any I/O error, as a message.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        let mut out = String::from("{\"records\":[");
        for (i, (key, e)) in self.entries.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            // The root's empty key serializes as an empty path sequence.
            let path_json = if key.is_empty() {
                "[]".to_string()
            } else {
                serde_json::to_string(&key.split(' ').collect::<Vec<&str>>())
                    .expect("sequence serializes")
            };
            out.push_str(&format!(
                "{{\"path\":{path_json},\"work_done\":{},\"passes_failed\":{}}}",
                e.work_done, e.passes_failed
            ));
        }
        out.push_str("]}\n");
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, out.as_bytes())
            .map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path)
            .map_err(|e| format!("cannot rename {} into place: {e}", tmp.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
