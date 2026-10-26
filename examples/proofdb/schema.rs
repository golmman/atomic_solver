//! The SQLite database and the canonical text dump
//! (`docs/spec/global_proof_store.md`, schema v2).
//!
//! Since plan22 the DB is the single source of truth and grows by the
//! incremental fold (`proofdb_merge`, one transaction per invocation); this
//! module keeps the tree-surface writers. [`write_db`] remains the
//! fixture/legacy fresh-init helper (a whole-tree write into a fresh file,
//! used by tests and fixtures — the fold's delta writer lives in
//! `merge/fold.rs`). The canonical dump is the byte-determinism gate
//! artifact: folding the identical shard set into an identical standing DB
//! must reproduce it byte-for-byte (ids are stable surrogate keys, D4).
//!
//! Schema v2 is additive over v1: the `censor` table relocates the harvest
//! work ledger (the frontier's censor history — facts of absence, not
//! representable in the tree model) into the DB, and the `drawn` table
//! records paths whose bounded search proved a draw (selection state that
//! prunes their subtrees; still no tree outcome). Consumers of the v1
//! surface (`nodes`/`shards`/`meta`) are unaffected; both scheduling tables
//! are ignorable by consumers.

use std::path::Path;

use rusqlite::Connection;

use super::GENERATOR;
use super::merge::{DepthStatus, PathTree};
use atomic_solver::notation::{move_to_bits, move_to_uci};
use atomic_solver::position::{Outcome, Position};

/// One `shards` table row (the shard's own claims, not merged refinements).
#[derive(Debug, Clone)]
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

/// A row-id assignment over a finalized [`PathTree`]: creation index → row
/// id, plus the row-id-sorted creation order the writers iterate. Ids are
/// stable surrogate keys (plan22 D4): existing rows keep their ids across
/// folds; only a fresh-init fold assigns over the whole tree (global lex
/// order — identical to the retired full rebuild's assignment).
#[derive(Debug, Clone)]
pub struct RowIds {
    /// Creation index → row id.
    pub id_of: Vec<i64>,
    /// Creation indices in row-id order (the writers' iteration order).
    pub order: Vec<usize>,
}

impl RowIds {
    /// The fresh-init assignment: finalize's row ids (global lexicographic
    /// path order; `finalize` must have run).
    #[must_use]
    pub fn from_finalize(tree: &super::merge::PathTree) -> Self {
        let mut id_of = vec![0i64; tree.nodes.len()];
        for (rank, &idx) in tree.row_order.iter().enumerate() {
            id_of[idx] = rank as i64;
        }
        Self {
            id_of,
            order: tree.row_order.clone(),
        }
    }

    /// The incremental fold's assignment: the nodes loaded from the DB keep
    /// their ids (`loaded_ids[creation index]`, dense over the loaded
    /// prefix), new nodes get `max_id+1…` in finalize's DFS (lexicographic)
    /// order filtered to the new nodes — parents sort before children.
    /// `finalize` must have run. A fresh-init fold (`loaded_ids` empty) is
    /// exactly finalize's assignment.
    #[must_use]
    pub fn fold(tree: &super::merge::PathTree, loaded_ids: &[i64], max_id: i64) -> Self {
        let mut id_of = vec![0i64; tree.nodes.len()];
        id_of[..loaded_ids.len()].copy_from_slice(loaded_ids);
        let mut next = max_id + 1;
        for &idx in &tree.row_order {
            if idx >= loaded_ids.len() {
                id_of[idx] = next;
                next += 1;
            }
        }
        // The writers iterate rows in row-id order; appended ids need not
        // follow the global DFS order, so sort explicitly.
        let mut order: Vec<usize> = (0..tree.nodes.len()).collect();
        order.sort_by_key(|&idx| id_of[idx]);
        Self { id_of, order }
    }
}

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

/// The proven-draw scheduling table: one row per path whose bounded search
/// returned a `Complete` draw. This is selection state, not proof data —
/// the tree model has no draw outcome yet — but it must survive sessions so
/// the walk prunes the drawn subtree instead of re-searching it. Consumers
/// ignore it exactly like `censor`.
const DRAWN_DDL: &str = "CREATE TABLE IF NOT EXISTS drawn (path TEXT PRIMARY KEY);";

/// Ensure the standing DB carries the fold's schema (plan22 D3): a fresh
/// file gets the full schema-v2 DDL; an existing file must be schema v2
/// (schema-version meta row = 2 *and* the censor table present — a v1 file
/// aborts with the rebuild hint). Run inside the fold's transaction.
///
/// # Errors
/// Any SQLite error, or a pre-v2 existing file, as a message.
pub fn ensure_fold_schema(conn: &Connection) -> Res<()> {
    let has_meta: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='meta'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("cannot inspect DB schema: {e}"))?;
    if has_meta == 0 {
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
        conn.execute_batch(&format!("{batch}{CENSOR_DDL}{DRAWN_DDL}"))
            .map_err(|e| format!("cannot create schema: {e}"))?;
        let mut stmt = conn
            .prepare("INSERT INTO meta VALUES (?1, ?2)")
            .map_err(|e| e.to_string())?;
        for (k, v) in [
            ("schema_version", "2"),
            ("root_fen", Position::STARTPOS_FEN),
        ] {
            stmt.execute(rusqlite::params![k, v])
                .map_err(|e| format!("cannot seed meta {k}: {e}"))?;
        }
        return Ok(());
    }
    let version: String = conn
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("meta row schema_version: {e}"))?;
    if version != "2" {
        return Err(format!(
            "DB schema version {version:?} is not v2 — the fold never rewrites an \
             existing file; move the old file aside and re-run proofdb_merge over \
             the shard set (a fresh fold recreates schema v2; --import-censor \
             migrates a legacy censor ledger)"
        ));
    }
    let has_censor: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='censor'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("cannot inspect DB schema: {e}"))?;
    if has_censor == 0 {
        return Err(
            "DB has no censor table (schema v1) — the fold never rewrites an \
             existing file; move the old file aside and re-run proofdb_merge \
             over the shard set (a fresh fold recreates schema v2)"
                .to_string(),
        );
    }
    // Additive scheduling table (step 1 of the proven-draw handling): an
    // existing v2 file gains an empty `drawn` table in place. This is the
    // only schema mutation the fold performs on an existing file, and it
    // adds no proof surface.
    conn.execute_batch(DRAWN_DDL)
        .map_err(|e| format!("cannot ensure drawn table: {e}"))?;
    Ok(())
}

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

/// Read every `drawn` row (the proven-draw scheduling state), ordered by
/// path. A missing table (a pre-step-1 v2 DB) reads as empty; the harvest's
/// next proven draw creates it on demand.
///
/// # Errors
/// Any SQLite error other than the missing table.
pub fn read_drawn_rows(conn: &Connection) -> Res<Vec<String>> {
    let present: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='drawn'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("cannot inspect schema: {e}"))?;
    if present == 0 {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare("SELECT path FROM drawn ORDER BY path")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Insert (or replace) one `drawn` scheduling row. `CREATE TABLE IF NOT
/// EXISTS` keeps a pre-step-1 v2 DB usable without a fold; the statement is
/// idempotent and only runs on the (rare) proven-draw path.
///
/// # Errors
/// Any SQLite error, as a message.
pub fn insert_drawn_row(conn: &Connection, path: &str) -> Res<()> {
    conn.execute_batch(DRAWN_DDL)
        .map_err(|e| format!("cannot ensure drawn table: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO drawn (path) VALUES (?1)",
        rusqlite::params![path],
    )
    .map_err(|e| format!("cannot commit drawn row {path:?}: {e}"))?;
    Ok(())
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
    conn.execute_batch(&format!("{batch}{CENSOR_DDL}{DRAWN_DDL}"))
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
    let ids = RowIds::from_finalize(tree);
    let mut stmt = conn
        .prepare("INSERT INTO nodes VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")
        .map_err(|e| e.to_string())?;
    for &ci in &ids.order {
        let row = ids.id_of[ci];
        let node = &tree.nodes[ci];
        let parent_row = node.parent.map(|p| ids.id_of[p]);
        let (mv_uci, mv_code) = node.mv.map_or((None, None), |mv| {
            (Some(move_to_uci(mv)), Some(i64::from(move_to_bits(mv))))
        });
        stmt.execute(rusqlite::params![
            row,
            parent_row,
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

/// Read every `shards` row, ordered by tag. The unmerged-shard gate's and
/// the harvest session-seeding's input (plan22 D6).
///
/// # Errors
/// Any SQLite error, as a message.
pub fn read_shard_rows(conn: &Connection) -> Res<Vec<ShardRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT tag, file, sha256, root_fen, path, outcome, depth_bound, n_nodes
             FROM shards ORDER BY tag",
        )
        .map_err(|e| e.to_string())?;
    let raw = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut rows = Vec::with_capacity(raw.len());
    for (tag, file, sha256, root_fen, path, outcome_s, bound, n_nodes) in raw {
        let outcome = outcome_s
            .parse::<Outcome>()
            .map_err(|_| format!("shards row {tag:?}: bad outcome {outcome_s:?}"))?;
        rows.push(ShardRow {
            tag,
            file,
            sha256,
            root_fen,
            path,
            outcome,
            depth_bound: u32::try_from(bound.max(0)).unwrap_or(u32::MAX),
            n_nodes: n_nodes.max(0) as usize,
        });
    }
    Ok(rows)
}

/// Render the canonical dump: `#`-prefixed meta lines, one TSV header, then
/// one line per node in row-id order (finalize's assignment). NULL fields
/// render as empty strings.
#[must_use]
pub fn canonical_dump(root_fen: &str, built_from: &str, tree: &PathTree) -> String {
    canonical_dump_with_ids(root_fen, built_from, tree, &RowIds::from_finalize(tree))
}

/// The dump over an explicit row-id assignment (the fold's stable-ids
/// variant).
#[must_use]
pub fn canonical_dump_with_ids(
    root_fen: &str,
    built_from: &str,
    tree: &PathTree,
    ids: &RowIds,
) -> String {
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
    for &ci in &ids.order {
        let row = ids.id_of[ci];
        let node = &tree.nodes[ci];
        let parent = node
            .parent
            .map_or(String::new(), |p| ids.id_of[p].to_string());
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
