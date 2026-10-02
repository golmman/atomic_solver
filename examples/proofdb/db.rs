//! The DB content the harvest loop works against (plan2 D2, plan3 D1):
//! read-only extraction of all nodes in path form, with the `built_from`
//! digest guard (no silent divergence between DB and shards).

use std::path::Path;

use rusqlite::OpenFlags;

use atomic_solver::position::Outcome;
use atomic_solver::position::Position;

type Res<T> = Result<T, String>;

/// Length-safe digest prefix for error messages. A DB-stored `built_from`
/// is arbitrary content (foreign/corrupted DB) — rendering it must never
/// panic; the message may print the value in full instead of truncating.
fn short(s: &str) -> &str {
    s.get(..16).unwrap_or(s)
}

/// One DB node in path form (the frontier-extraction input).
#[derive(Debug)]
pub struct DbRow {
    pub path: Vec<String>,
    pub ply: usize,
    pub outcome: Option<Outcome>,
    pub depth_bound: Option<u32>,
}

/// The DB content the frontier extraction works against (extracted
/// read-only).
#[derive(Debug)]
pub struct DbContent {
    pub root_fen: String,
    pub rows: Vec<DbRow>,
}

/// Load all DB nodes with their paths, outcomes, and depth bounds,
/// read-only, aborting unless `meta.built_from` equals `manifest_sha256`
/// and `root_fen` is the startpos.
///
/// # Errors
/// Any SQLite/I/O error, a digest mismatch (stale DB), a non-startpos root,
/// a broken parent chain, a ply inconsistency, or an unexpected outcome.
pub fn load_db_rows(db_path: &Path, manifest_sha256: &str) -> Res<DbContent> {
    let conn = rusqlite::Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("cannot open DB {}: {e}", db_path.display()))?;
    let meta = |key: &str| -> Res<String> {
        conn.query_row("SELECT value FROM meta WHERE key=?1", [key], |r| r.get(0))
            .map_err(|e| format!("meta row {key:?}: {e}"))
    };
    let built_from: String = meta("built_from")?;
    if built_from != manifest_sha256 {
        return Err(format!(
            "DB built_from {} != manifest digest {} (stale DB or stale manifest — \
             re-merge the standing shard set first)",
            short(&built_from),
            short(manifest_sha256)
        ));
    }
    let root_fen: String = meta("root_fen")?;
    if root_fen != Position::STARTPOS_FEN {
        return Err(format!("DB root_fen is not the startpos: {root_fen}"));
    }
    let mut stmt = conn
        .prepare(
            "SELECT id, parent_id, ply, move_uci, outcome, depth_bound
             FROM nodes ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let raw = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<i64>>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    // Row id → (parent id, ply, move) for the path walk; rows are dense ids
    // 0..n by construction, but resolve through the map regardless.
    let by_id: std::collections::HashMap<i64, (Option<i64>, i64, Option<String>)> =
        raw.iter().map(|r| (r.0, (r.1, r.2, r.3.clone()))).collect();
    let mut rows = Vec::with_capacity(raw.len());
    for (_, parent, ply, mv_uci, outcome, bound) in &raw {
        let mut chain: Vec<String> = Vec::new();
        let mut cur = Some((*parent, *ply, mv_uci.clone()));
        while let Some((p, pl, mv)) = cur {
            match mv {
                Some(m) => chain.push(m),
                None if pl == 0 => {}
                None => return Err("non-root node without a move".to_string()),
            }
            cur = p
                .map(|p| {
                    by_id
                        .get(&p)
                        .cloned()
                        .ok_or_else(|| "broken parent chain (unresolvable parent_id)".to_string())
                })
                .transpose()?;
        }
        chain.reverse();
        // Parent ply must be exactly one below the child's (DB integrity).
        if *ply > 0 {
            let p_id = parent.expect("non-root has a parent");
            let p_ply = by_id.get(&p_id).expect("parent resolves").1;
            if p_ply != *ply - 1 {
                return Err(format!("node at ply {ply} has a parent at ply {p_ply}"));
            }
        }
        let outcome = match outcome.as_deref() {
            None => None,
            Some("win") => Some(Outcome::Win),
            Some("loss") => Some(Outcome::Loss),
            Some(o) => return Err(format!("unexpected node outcome {o:?}")),
        };
        let depth_bound = match bound {
            None => None,
            Some(b) if *b >= 0 => Some(u32::try_from(*b).expect("positive i64 fits u32")),
            Some(b) => return Err(format!("negative depth_bound {b}")),
        };
        rows.push(DbRow {
            ply: *ply as usize,
            path: chain,
            outcome,
            depth_bound,
        });
    }
    Ok(DbContent { root_fen, rows })
}
