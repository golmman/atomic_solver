//! The censor state (plan20): the harvester's pick-up state, relocated from
//! the sidecar `work.json` into the DB's `censor` table (schema v2,
//! `docs/spec/global_proof_store.md`). One record per **known-open frontier
//! node** — a node the harvester has censed (or exposed) — with the
//! non-derivable selection state: how many passes failed at it and how much
//! work was spent. Record presence = known-open; the numbers are censor
//! history (*facts of absence* — a censored draw job is not representable
//! in the tree model), never census facts. The DB is the single source of
//! truth for them: the in-memory [`Ledger`] is the same `BTreeMap` the
//! file-backed ledger always was; only the persistence backend changed.
//!
//! Lineage gate (decision 6, enforced by the walk frontier's construction,
//! which owns the DB rows): a record whose path is now a *proven* DB row is
//! dropped (the node was decided after the record was written); a record
//! whose path does not replay legally from the startpos aborts the session
//! (the plan2 replay contract, now covering persisted state). Records on
//! open rows are kept (the row is undecided; the record carries its censor
//! history).
//!
//! Durability (plan20 D3/D4, the file ledger's per-censor atomic-save
//! discipline): [`on_censored`] bumps the map and commits one `INSERT OR
//! REPLACE` transaction on the DB connection — a censor is durable the
//! moment the call returns, and a failed commit aborts the batch with the
//! DB's last committed state consistent (the in-flight censor is lost,
//! matching the plan17 loss window). Writer carve-out: the harvest writes
//! **`censor`/`drawn` scheduling rows only**; `proofdb_merge` stays the sole
//! writer of `nodes`/`shards`/`meta` and preserves `censor` across its
//! rebuild (D5).
//!
//! Proven draws (step 1 of the draw-model work): a `Draw` with
//! `ExitReason::Complete` is not a fact of absence. [`on_proven_draw`]
//! records it in the path-keyed `drawn` table; the in-memory [`Ledger`]
//! carries the same set so the frontier can drop the path and its subtree
//! without re-searching it. This is scheduling state: the tree model still
//! has no draw outcome.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rusqlite::Connection;

use super::schema::{CensorRow, insert_censor_row, insert_drawn_row, read_drawn_rows};

/// One known-open frontier record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LedgerEntry {
    /// Cumulative child-evals spent at this node (all passes).
    pub work_done: u64,
    /// Number of censored (budget-exhausted) visits so far.
    pub passes_failed: u32,
}

/// The censor map: path key (space-joined UCI, empty for the root) →
/// entry, in a `BTreeMap` so the iteration order is deterministic.
#[derive(Debug, Default, Clone)]
pub struct Ledger {
    entries: BTreeMap<String, LedgerEntry>,
    /// Paths whose value is a proven draw (a `Complete` Draw job). These
    /// are not censor records; they prune the path and its subtree from
    /// candidacy across sessions.
    drawn: BTreeSet<String>,
}

impl Ledger {
    /// Load the censor state from the DB connection (schema v2). A missing
    /// `censor` table is a clean abort with the upgrade hint — a schema-v1
    /// DB must be rebuilt by `proofdb_merge` first (plan20 D1).
    ///
    /// # Errors
    /// Any SQLite error, or a missing `censor` table.
    pub fn load_db(conn: &Connection) -> Result<Self, String> {
        let present: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='censor'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| format!("cannot inspect DB schema: {e}"))?;
        if present == 0 {
            return Err(
                "DB has no censor table (schema v1) — re-run proofdb_merge to \
                 rebuild it with schema v2 (the censor state lives in the DB \
                 since plan20)"
                    .to_string(),
            );
        }
        let mut stmt = conn
            .prepare("SELECT path, work_done, passes_failed FROM censor ORDER BY path")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let mut entries = BTreeMap::new();
        for (path, work, passes) in rows {
            if entries
                .insert(
                    path,
                    LedgerEntry {
                        work_done: work.max(0) as u64,
                        passes_failed: passes.max(0) as u32,
                    },
                )
                .is_some()
            {
                return Err("censor table has a duplicate path".to_string());
            }
        }
        let drawn: BTreeSet<String> = read_drawn_rows(conn)?.into_iter().collect();
        Ok(Self { entries, drawn })
    }

    /// Parse a legacy JSON work ledger (`work.json`, the pre-plan20 sidecar
    /// format) — migration only (`proofdb_merge --import-censor`, plan20
    /// D6); the format is otherwise retired.
    ///
    /// # Errors
    /// Any I/O error or malformed JSON (with the offending path/message).
    pub fn load_legacy_json(path: &Path) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct RawLedger {
            records: Vec<RawRecord>,
        }
        #[derive(serde::Deserialize)]
        struct RawRecord {
            path: Vec<String>,
            work_done: u64,
            passes_failed: u32,
        }
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            // The import flag names an existing file — a missing ledger is
            // an operator error, not a fresh state (the harvest's old
            // fresh-init semantic died with the file backend).
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
        Ok(Self {
            entries,
            drawn: BTreeSet::new(),
        })
    }

    /// Build a ledger from an already-keyed map (keys are space-joined UCI
    /// paths, empty for the root, exactly as [`Ledger::load_db`] produces
    /// them; the `BTreeMap` keeps the iteration sorted).
    #[must_use]
    pub fn from_entries(entries: BTreeMap<String, LedgerEntry>) -> Self {
        Self {
            entries,
            drawn: BTreeSet::new(),
        }
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

    /// Is `key` a proven-draw scheduling record?
    #[must_use]
    pub fn is_drawn(&self, key: &str) -> bool {
        self.drawn.contains(key)
    }

    /// Iterate the proven-draw paths in sorted order (the frontier's
    /// scope-relative drop set).
    pub fn drawn_iter(&self) -> impl Iterator<Item = &String> {
        self.drawn.iter()
    }

    /// Number of records (census).
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.drawn.is_empty()
    }

    /// Iterate `(key, entry)` in sorted path order (the gate walks this).
    pub fn iter(&self) -> impl Iterator<Item = (&String, &LedgerEntry)> {
        self.entries.iter()
    }

    /// Record `key` as a proven draw in memory (the DB commit is
    /// [`on_proven_draw`]'s job).
    pub fn mark_drawn(&mut self, key: &str) {
        self.drawn.insert(key.to_string());
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

    /// Upsert under the import max rule (plan20 D6, per field): `work_done`
    /// and `passes_failed` each keep the max of the stored vs the incoming
    /// value; a new key is inserted. Total, deterministic, idempotent, and
    /// never decreases a stored value (a stale import is harmless).
    pub fn upsert_max(&mut self, key: &str, entry: LedgerEntry) {
        let cur = self.entries.entry(key.to_string()).or_default();
        cur.work_done = cur.work_done.max(entry.work_done);
        cur.passes_failed = cur.passes_failed.max(entry.passes_failed);
    }
}

/// The proven-draw hook: record `key` in the DB's `drawn` scheduling table
/// and in the in-memory ledger. No censor row is written — a proven draw is
/// not a fact of absence. Idempotent (a path is proven once).
///
/// # Errors
/// A `drawn` commit failure (the batch aborts; the DB's last committed
/// state stays consistent).
pub fn on_proven_draw(ledger: &mut Ledger, censor: &Connection, key: &str) -> Result<(), String> {
    insert_drawn_row(censor, key)?;
    ledger.mark_drawn(key);
    Ok(())
}

/// The bump-only censor hook (plan8 D1, now the walk's hook): bump the
/// record at `key` with the censored visit's evals and commit it to the
/// DB's `censor` table in one transaction (plan20 D3/D4 — a commit failure
/// aborts the batch; the DB's last committed state is consistent).
///
/// # Errors
/// A censor commit failure (including a `work_done` beyond the i64 storage
/// width), as a message.
pub fn on_censored(
    ledger: &mut Ledger,
    censor: &Connection,
    key: &str,
    child_evals: u64,
) -> Result<(), String> {
    // Compute the post-bump entry and commit it before touching the map —
    // a failed commit leaves the in-memory state untouched (plan20 D4; the
    // batch aborts on the error either way).
    let cur = ledger.get(key).copied().unwrap_or_default();
    insert_censor_row(
        censor,
        &CensorRow {
            path: key.to_string(),
            work_done: cur.work_done.saturating_add(child_evals),
            passes_failed: cur.passes_failed + 1,
        },
    )?;
    ledger.bump(key, child_evals);
    Ok(())
}

#[cfg(test)]
mod tests;
