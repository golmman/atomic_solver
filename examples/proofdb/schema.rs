//! The derived SQLite database and the canonical text dump
//! (`docs/spec/global_proof_store.md`, schema v2).
//!
//! The DB is a derived, rebuildable view of the shard set; this module is its
//! only writer (`nodes`/`shards`/`meta` — the harvest's censor-row carve-out
//! writes the `censor` table only). The canonical dump is the
//! byte-determinism gate artifact: rebuilding from an identical shard set
//! must reproduce it byte-for-byte.
//!
//! Schema v2 (plan20) is additive over v1: the `censor` table relocates the
//! harvest work ledger (the frontier's censor history — facts of absence,
//! not representable in the tree model) into the DB. Consumers of the v1
//! surface (`nodes`/`shards`/`meta`) are unaffected; the `censor` table is
//! selection state and ignorable by consumers.

use std::path::Path;

use rusqlite::Connection;

use super::GENERATOR;
use super::merge::{DepthStatus, PathTree};
use atomic_solver::notation::{move_to_bits, move_to_uci};
use atomic_solver::position::Outcome;

/// One `shards` table row (the shard's own claims, not merged refinements).
pub struct ShardRow {
    pub tag: String,
    pub file: String,
    pub sha256: String,
    pub root_fen: String,
    /// Space-separated UCI path; empty for a startpos-rooted shard.
    pub path: String,
    pub outcome: Outcome,
    pub depth_bound: u32,
    pub n_nodes: usize,
}

type Res<T> = Result<T, String>;

/// One `censor` table row (schema v2): the relocated work-ledger record.
/// `path` is the absolute space-joined UCI path key (`""` = the root),
/// byte-for-byte the ledger key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CensorRow {
    pub path: String,
    pub work_done: u64,
    pub passes_failed: u32,
}

const CENSOR_DDL: &str = "CREATE TABLE censor (path TEXT PRIMARY KEY, work_done INTEGER NOT NULL, \
     passes_failed INTEGER NOT NULL);";

/// Read every `censor` row, ordered by path. A missing table (a schema-v1
/// DB) reads as empty — the merger's preserve-and-rebuild (plan20 D5) must
/// not fail on a pre-v2 file.
///
/// # Errors
/// Any SQLite error other than the missing table.
pub fn read_censor_rows(conn: &Connection) -> Res<Vec<CensorRow>> {
    let present: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='censor'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("cannot inspect schema: {e}"))?;
    if present == 0 {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare("SELECT path, work_done, passes_failed FROM censor ORDER BY path")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(CensorRow {
                path: r.get(0)?,
                work_done: r.get::<_, i64>(1)?.max(0) as u64,
                passes_failed: r.get::<_, i64>(2)?.max(0) as u32,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Insert (or replace) one `censor` row. Each call is one committed
/// transaction (autocommit; default journal mode) — the harvest's
/// per-censor durability discipline (plan20 D3).
///
/// # Errors
/// Any SQLite error, as a message.
pub fn insert_censor_row(conn: &Connection, row: &CensorRow) -> Res<()> {
    let work = i64::try_from(row.work_done).map_err(|_| {
        format!(
            "censor work_done {} exceeds the i64 storage width",
            row.work_done
        )
    })?;
    conn.execute(
        "INSERT OR REPLACE INTO censor (path, work_done, passes_failed) VALUES (?1, ?2, ?3)",
        rusqlite::params![row.path, work, i64::from(row.passes_failed)],
    )
    .map_err(|e| format!("cannot commit censor row {:?}: {e}", row.path))?;
    Ok(())
}

/// Write the database at `path` (replacing any existing file).
///
/// `censor` carries the preserved/imported censor state (plan20 D5/D6) —
/// written inside the same fresh-file creation so a rebuild never loses it.
///
/// # Errors
/// Any I/O or SQLite error, as a message.
pub fn write_db(
    path: &Path,
    root_fen: &str,
    built_from: &str,
    shard_rows: &[ShardRow],
    tree: &PathTree,
    censor: &[CensorRow],
) -> Res<()> {
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|e| format!("cannot replace {}: {e}", path.display()))?;
    }
    let conn =
        Connection::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let batch = "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
         CREATE TABLE shards (
           tag TEXT PRIMARY KEY, file TEXT, sha256 TEXT,
           root_fen TEXT, path TEXT,
           outcome TEXT, depth_bound INTEGER, n_nodes INTEGER
         );
         CREATE TABLE nodes (
           id INTEGER PRIMARY KEY,
           parent_id INTEGER,
           ply INTEGER NOT NULL,
           move_uci TEXT,
           move_code INTEGER,
           outcome TEXT,
           depth_bound INTEGER,
           depth_status TEXT,
           provenance TEXT
         );
         CREATE INDEX idx_nodes_parent ON nodes(parent_id);
         ";
    conn.execute_batch(&format!("{batch}{CENSOR_DDL}"))
        .map_err(|e| format!("cannot create schema: {e}"))?;
    let node_count = tree.row_order.len().to_string();
    let meta: [(&str, &str); 5] = [
        ("schema_version", "2"),
        ("root_fen", root_fen),
        ("node_count", &node_count),
        ("built_from", built_from),
        ("generator", GENERATOR),
    ];
    let mut stmt = conn
        .prepare("INSERT INTO meta VALUES (?1, ?2)")
        .map_err(|e| e.to_string())?;
    for (k, v) in &meta {
        stmt.execute(rusqlite::params![k, v])
            .map_err(|e| e.to_string())?;
    }
    let mut stmt = conn
        .prepare("INSERT INTO shards VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")
        .map_err(|e| e.to_string())?;
    for s in shard_rows {
        stmt.execute(rusqlite::params![
            s.tag,
            s.file,
            s.sha256,
            s.root_fen,
            s.path,
            s.outcome.as_str(),
            s.depth_bound,
            s.n_nodes as i64,
        ])
        .map_err(|e| format!("cannot insert shard {}: {e}", s.tag))?;
    }
    let mut stmt = conn
        .prepare("INSERT INTO nodes VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")
        .map_err(|e| e.to_string())?;
    for (row, &ci) in tree.row_order.iter().enumerate() {
        let node = &tree.nodes[ci];
        let parent_row = node.parent.map(|p| tree.row_of[p]);
        let (mv_uci, mv_code) = node.mv.map_or((None, None), |mv| {
            (Some(move_to_uci(mv)), Some(i64::from(move_to_bits(mv))))
        });
        stmt.execute(rusqlite::params![
            row as i64,
            parent_row.map(|p| p as i64),
            node.ply as i64,
            mv_uci,
            mv_code,
            node.outcome.map(|o| o.as_str().to_string()),
            tree.depth_bound(ci),
            tree.depth_status(ci).map(DepthStatus::as_str),
            node.provenance
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(","),
        ])
        .map_err(|e| format!("cannot insert node {row}: {e}"))?;
    }
    for c in censor {
        insert_censor_row(&conn, c)?;
    }
    Ok(())
}

/// Render the canonical dump: `#`-prefixed meta lines, one TSV header, then
/// one line per node in row-id order. NULL fields render as empty strings.
#[must_use]
pub fn canonical_dump(root_fen: &str, built_from: &str, tree: &PathTree) -> String {
    let mut out = String::new();
    out.push_str("# proofdb canonical dump v1 (docs/spec/global_proof_store.md)\n");
    out.push_str("# schema_version 2\n");
    out.push_str("# root_fen ");
    out.push_str(root_fen);
    out.push('\n');
    out.push_str("# node_count ");
    out.push_str(&tree.row_order.len().to_string());
    out.push('\n');
    out.push_str("# built_from ");
    out.push_str(built_from);
    out.push('\n');
    out.push_str("# generator ");
    out.push_str(GENERATOR);
    out.push('\n');
    out.push_str(
        "id\tparent_id\tply\tmove_uci\tmove_code\toutcome\tdepth_bound\tdepth_status\tprovenance\n",
    );
    for (row, &ci) in tree.row_order.iter().enumerate() {
        let node = &tree.nodes[ci];
        let parent = node
            .parent
            .map_or(String::new(), |p| tree.row_of[p].to_string());
        let (mv_uci, mv_code) = node.mv.map_or((String::new(), String::new()), |mv| {
            (move_to_uci(mv), move_to_bits(mv).to_string())
        });
        let provenance = node
            .provenance
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        let outcome = node
            .outcome
            .map_or(String::new(), |o| o.as_str().to_string());
        let bound = tree
            .depth_bound(ci)
            .map_or(String::new(), |d| d.to_string());
        let status = tree
            .depth_status(ci)
            .map_or(String::new(), |s| s.as_str().to_string());
        out.push_str(&format!(
            "{row}\t{parent}\t{}\t{mv_uci}\t{mv_code}\t{outcome}\t{bound}\t{status}\t{provenance}\n",
            node.ply,
        ));
    }
    out
}
