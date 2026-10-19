//! The incremental fold (plan22 D3/D4): the standing DB is the source of
//! truth, and `proofdb_merge` grafts new shards into it in one transaction
//! per invocation.
//!
//! Per fold: open the standing DB read-write (a missing file folds
//! everything into a fresh schema-v2 DB; a pre-v2 file aborts), load the
//! merged tree from the `nodes` rows via a replay-validated DFS
//! ([`load_standing_tree`] — the fold is self-validating on load: a
//! structurally broken standing DB aborts before any write), fold every
//! *unmerged* shard (sidecar dir scan; tags already in the `shards` table
//! are skipped after a sha256 cross-check), run the depth-refinement
//! fixpoint, re-validate every folded graft's skeleton, then write the
//! delta ([`write_fold_delta`]): new node rows, UPDATEs for existing rows
//! whose recomputed `(outcome, depth_bound, depth_status, provenance)`
//! differ, one `shards` insert per folded tag, and the meta refresh. Any
//! defect rolls the whole invocation back — conflicts abort without
//! writes, and a rolled-back fold re-runs from scratch idempotently.
//!
//! Row ids (D4): existing rows keep their ids forever; new rows get
//! `max(id)+1…` in finalize's lexicographic order (parents sort before
//! children). A fresh-init fold assigns over the whole tree in global
//! lexicographic path order — id-identical to the retired full rebuild.
//!
//! Recorded limitation (accepted, D3): the load holds the whole merged tree
//! in RAM (O(N) ≈ ~200 B/node). A streaming row-surgery fold is the future
//! step if multi-GB layers materialize — not built here.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use atomic_movegen::types::Move;

use atomic_solver::notation::{move_to_bits, move_to_uci, uci_to_move};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofTree, validate_proof_tree};
use rusqlite::Connection;

use super::super::ledger::{Ledger, LedgerEntry};
use super::super::schema::{
    CensorRow, RowIds, ShardRow, ensure_fold_schema, insert_censor_row, read_shard_rows,
};
use super::super::sidecar::{self, SidecarEntry};
use super::super::{GENERATOR, Manifest, ShardEntry, digest_hex};
use super::{PathNode, PathTree};

type Res<T> = Result<T, String>;

/// One raw `nodes` row, in query order.
type RawRow = (
    i64,
    Option<i64>,
    i64,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<i64>,
    Option<String>,
    String,
);

/// One loaded node row's stored variable fields (the compare-and-update
/// inputs).
#[derive(Debug, Clone)]
pub struct StoredRow {
    pub id: i64,
    pub outcome: Option<String>,
    pub depth_bound: Option<i64>,
    pub depth_status: Option<String>,
    pub provenance: String,
}

/// The standing layer's DB-derived state: the loaded nodes' row ids (dense
/// over creation indices `0..ids.len()` — the loader reuses the pre-created
/// root as creation index 0), their stored variable fields, and the
/// `shards`-table rows.
pub struct LoadedState {
    pub ids: Vec<i64>,
    pub stored: Vec<StoredRow>,
    pub shard_rows: Vec<ShardRow>,
    pub max_id: i64,
}

/// The standing layer as loaded from the DB.
pub struct StandingLoad {
    pub tree: PathTree,
    pub state: LoadedState,
}

/// The fold's census (per invocation; printed by the CLI).
#[derive(Debug, Default)]
pub struct FoldReport {
    pub folded: usize,
    pub already: usize,
    pub orphans: usize,
    pub sidecars_backfilled: usize,
    pub shard_nodes_sum: usize,
    pub skeleton_revalidations: usize,
    pub inserted_nodes: usize,
    pub updated_nodes: usize,
    pub node_count: usize,
    pub proven: usize,
    pub open: usize,
    pub overlay_insertions: usize,
    pub ancestor_insertions: usize,
    pub open_upgraded: usize,
    pub imported_censor: usize,
    pub fold_digest: String,
}

/// The fold's output: the finalized tree (for the CLI's dump and
/// `--sample-lines`), the row-id assignment the delta was written with, and
/// the census.
pub struct FoldOutput {
    pub tree: PathTree,
    pub ids: RowIds,
    pub report: FoldReport,
}

/// The fold's shard metadata input: everything [`load_shard`] cross-checks.
/// Built from a sidecar (validate = `ok` implicitly, sha present) or a
/// legacy manifest entry (D2's migration/metadata source).
#[derive(Debug, Clone)]
pub struct ShardMeta {
    pub tag: String,
    pub file: String,
    pub fen: String,
    pub moves: Vec<String>,
    pub outcome: Outcome,
    pub validate: String,
    pub sha256: Option<String>,
}

impl ShardMeta {
    /// From a sidecar (the normal source since plan22).
    #[must_use]
    pub fn from_sidecar(e: &SidecarEntry) -> Self {
        Self {
            tag: e.tag.clone(),
            file: e.file.clone(),
            fen: e.fen.clone(),
            moves: e.moves.clone(),
            outcome: e.outcome,
            validate: "ok".to_string(),
            sha256: Some(e.sha256.clone()),
        }
    }

    /// From a legacy manifest entry (D2): the manifest is metadata for
    /// unmerged tags that lack a sidecar; its `validate` gate still applies.
    #[must_use]
    pub fn from_manifest_entry(e: &ShardEntry) -> Self {
        Self {
            tag: e.tag.clone(),
            file: e.file.clone(),
            fen: e.fen.clone(),
            moves: e.moves.clone(),
            outcome: e.outcome,
            validate: e.validate.clone(),
            sha256: None,
        }
    }
}

/// Load, cross-check, replay-validate, and hash-verify one shard. Returns
/// the tree, its file digest, and the metadata path as resolved `Move`s
/// (the startpos replay doubles as the hash-fidelity check).
///
/// # Errors
/// Any defect: a non-`ok` validate verdict, an unreadable/unparseable bin,
/// a sha256 mismatch, replay-validation defects, an outcome cross-check
/// mismatch, an illegal metadata move, or a hash-fidelity failure.
pub fn load_shard(meta: &ShardMeta, shard_dir: &Path) -> Res<(ProofTree, String, Vec<Move>)> {
    if meta.validate != "ok" {
        return Err(format!(
            "shard {}: validate={} (only 'ok' enters a fold)",
            meta.tag, meta.validate
        ));
    }
    let path = shard_dir.join(&meta.file);
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("shard {}: cannot read {}: {e}", meta.tag, path.display()))?;
    let digest = digest_hex(&bytes);
    if let Some(expected) = &meta.sha256
        && *expected != digest
    {
        return Err(format!(
            "shard {}: bin/sha256 mismatch (sidecar or standing row says \
             {expected}, the bin digests to {digest})",
            meta.tag
        ));
    }
    let tree = ProofTree::from_bin(&mut bytes.as_slice())
        .map_err(|e| format!("shard {}: cannot parse binary tree: {e}", meta.tag))?;
    if let Err(defects) = validate_proof_tree(&tree) {
        return Err(format!(
            "shard {}: replay validation found {} defects: {}",
            meta.tag,
            defects.len(),
            defects
                .iter()
                .take(3)
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let root_outcome = tree.nodes[0]
        .outcome
        .ok_or_else(|| format!("shard {}: tree root is unrealized", meta.tag))?;
    if root_outcome != meta.outcome {
        return Err(format!(
            "shard {}: metadata outcome {} != tree root outcome {}",
            meta.tag,
            meta.outcome.as_str(),
            root_outcome.as_str()
        ));
    }
    let mut pos = Position::from_fen(Position::STARTPOS_FEN).expect("startpos parses");
    let mut moves = Vec::with_capacity(meta.moves.len());
    for uci in &meta.moves {
        let Some(mv) = uci_to_move(uci, &pos) else {
            return Err(format!(
                "shard {}: metadata move {uci:?} is not legal on the replayed path",
                meta.tag
            ));
        };
        pos.do_move(mv);
        moves.push(mv);
    }
    let meta_pos = Position::from_fen(&meta.fen)
        .map_err(|e| format!("shard {}: bad metadata fen: {e}", meta.tag))?;
    let tree_pos = Position::from_fen(&tree.root_fen)
        .map_err(|e| format!("shard {}: bad tree root fen: {e}", meta.tag))?;
    let rh = pos.hash();
    if meta_pos.hash() != rh || tree_pos.hash() != rh {
        return Err(format!(
            "shard {}: hash fidelity failed (replayed {:016x}, metadata fen {:016x}, \
             tree root {:016x})",
            meta.tag,
            rh,
            meta_pos.hash(),
            tree_pos.hash()
        ));
    }
    Ok((tree, digest, moves))
}

/// Load the standing tree from the DB's `nodes` rows: a replay-validated
/// DFS from the root with a lib `Position` (do/undo), re-deriving every
/// stored move (`uci_to_move`), cross-checking `ply`, parent-chain
/// integrity, and `move_code == move_to_bits(move)`. Restores `outcome`,
/// `provenance` (comma-split), and `exact_claims` (non-terminal rows with
/// `depth_status='exact'` claim their stored bound — the finalize
/// consistency check re-derives them, so a refined claim aborts the fold
/// exactly as a fresh overlay would).
///
/// # Errors
/// A missing/duplicate root, an unparseable/illegal stored move, a ply or
/// move-code inconsistency, an unknown outcome or status, a stored bound on
/// an open row, a duplicate child move, unreachable rows, or a non-startpos
/// `meta.root_fen`.
pub fn load_standing_tree(conn: &Connection) -> Res<StandingLoad> {
    let root_fen: String = conn
        .query_row("SELECT value FROM meta WHERE key='root_fen'", [], |r| {
            r.get(0)
        })
        .map_err(|e| format!("meta row root_fen: {e}"))?;
    if root_fen != Position::STARTPOS_FEN {
        return Err(format!("DB root_fen is not the startpos: {root_fen}"));
    }
    let raw: Vec<RawRow> = {
        let mut stmt = conn
            .prepare(
                "SELECT id, parent_id, ply, move_uci, move_code, outcome, depth_bound,
                 depth_status, provenance FROM nodes ORDER BY id",
            )
            .map_err(|e| e.to_string())?;
        stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
    };
    let shard_rows = read_shard_rows(conn)?;
    if raw.is_empty() {
        return Ok(StandingLoad {
            tree: PathTree::new(),
            state: LoadedState {
                ids: Vec::new(),
                stored: Vec::new(),
                shard_rows,
                max_id: -1,
            },
        });
    }
    let root_rows: Vec<usize> = raw
        .iter()
        .enumerate()
        .filter(|(_, r)| r.1.is_none())
        .map(|(i, _)| i)
        .collect();
    if root_rows.len() != 1 {
        return Err(format!(
            "nodes has {} parentless rows (expected exactly one root)",
            root_rows.len()
        ));
    }
    let mut children_of: HashMap<i64, Vec<usize>> = HashMap::new();
    for (i, r) in raw.iter().enumerate() {
        if let Some(p) = r.1 {
            children_of.entry(p).or_default().push(i);
        }
    }
    let mut tree = PathTree::new();
    let mut ids: Vec<i64> = Vec::with_capacity(raw.len());
    let mut stored: Vec<StoredRow> = Vec::with_capacity(raw.len());
    let mut pos = Position::from_fen(Position::STARTPOS_FEN).expect("startpos parses");
    let mut visited = 0usize;
    walk(
        &mut tree,
        &mut ids,
        &mut stored,
        &raw,
        &children_of,
        root_rows[0],
        None,
        &mut pos,
        &mut visited,
    )?;
    if visited != raw.len() {
        return Err(format!(
            "{}/{} node rows are unreachable from the root",
            raw.len() - visited,
            raw.len()
        ));
    }
    let max_id = ids.iter().copied().max().unwrap_or(-1);
    Ok(StandingLoad {
        tree,
        state: LoadedState {
            ids,
            stored,
            shard_rows,
            max_id,
        },
    })
}

/// The loader's DFS (do/undo on one `Position`): visit `row` whose parent
/// creation index is `parent_idx` (`None` = root). `pos` is the position at
/// the *parent* for a non-root row, at the row itself for the root.
#[allow(clippy::too_many_arguments)]
fn walk(
    tree: &mut PathTree,
    ids: &mut Vec<i64>,
    stored: &mut Vec<StoredRow>,
    raw: &[RawRow],
    children_of: &HashMap<i64, Vec<usize>>,
    row_idx: usize,
    parent_idx: Option<usize>,
    pos: &mut Position,
    visited: &mut usize,
) -> Res<()> {
    let (id, parent_id, ply, move_uci, move_code, outcome_s, bound, status_s, provenance_s) =
        raw[row_idx].clone();
    if parent_id.is_none() != parent_idx.is_none() {
        return Err(format!("node {id}: parent-chain inconsistency"));
    }
    // The move: replay-legality against the parent position, plus the
    // stored move-code agreement.
    let mv = match (ply > 0, move_uci.as_deref(), move_code) {
        (false, None, None) => None,
        (false, _, _) => {
            return Err(format!(
                "node {id}: the root row must have no move/move_code"
            ));
        }
        (true, None, _) => {
            return Err(format!("node {id}: non-root row without a move"));
        }
        (true, Some(uci), code) => {
            let Some(mv) = uci_to_move(uci, pos) else {
                return Err(format!(
                    "node {id}: stored move {uci:?} is not legal in the replayed parent position"
                ));
            };
            if code != Some(i64::from(move_to_bits(mv))) {
                return Err(format!(
                    "node {id}: move_code {} disagrees with move {uci:?} ({})",
                    code.unwrap_or(-1),
                    move_to_bits(mv)
                ));
            }
            Some(mv)
        }
    };
    let outcome = match outcome_s.as_deref() {
        None => None,
        Some("win") => Some(Outcome::Win),
        Some("loss") => Some(Outcome::Loss),
        Some(o) => return Err(format!("node {id}: unexpected node outcome {o:?}")),
    };
    // A stored bound/status on an open row, or a missing one on a proven
    // row, is a defective standing DB.
    if outcome.is_none() && (bound.is_some() || status_s.is_some()) {
        return Err(format!("node {id}: open row with a stored bound/status"));
    }
    if outcome.is_some() && (bound.is_none() || status_s.is_none()) {
        return Err(format!("node {id}: proven row without a bound/status"));
    }
    if let Some(s) = status_s.as_deref()
        && s != "bound"
        && s != "exact"
    {
        return Err(format!("node {id}: unknown depth_status {s:?}"));
    }
    if let Some(b) = bound
        && b < 0
    {
        return Err(format!("node {id}: negative depth_bound {b}"));
    }
    let provenance: BTreeSet<String> = provenance_s
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    // A non-terminal `exact` row: restore its claim (the finalize
    // consistency check re-derives it against the recomputed bound).
    let exact_claims: Vec<u32> = if outcome.is_some()
        && status_s.as_deref() == Some("exact")
        && children_of.contains_key(&id)
    {
        vec![bound.expect("checked above") as u32]
    } else {
        Vec::new()
    };
    let stored_row = StoredRow {
        id,
        outcome: outcome_s,
        depth_bound: bound,
        depth_status: status_s,
        provenance: provenance_s,
    };
    let node_idx = match parent_idx {
        None => {
            // The loader reuses PathTree::new's pre-created root.
            if ply != 0 {
                return Err(format!("node {id}: the root row must be at ply 0"));
            }
            tree.nodes[0].outcome = outcome;
            tree.nodes[0].provenance = provenance;
            tree.nodes[0].exact_claims = exact_claims;
            0usize
        }
        Some(p) => {
            let parent_ply = tree.nodes[p].ply;
            if ply as usize != parent_ply + 1 {
                return Err(format!(
                    "node {id}: ply {ply} but its parent is at ply {parent_ply}"
                ));
            }
            let node_idx = tree.nodes.len();
            let code = move_to_bits(mv.expect("checked above"));
            if tree.nodes[p].children.contains_key(&code) {
                return Err(format!(
                    "node {id}: duplicate child move under node {}",
                    parent_id.expect("checked above")
                ));
            }
            tree.nodes.push(PathNode {
                parent: Some(p),
                ply: ply as usize,
                mv,
                outcome,
                provenance,
                children: BTreeMap::new(),
                exact_claims,
            });
            tree.nodes[p].children.insert(code, node_idx);
            node_idx
        }
    };
    ids.push(id);
    stored.push(stored_row);
    *visited += 1;
    if let Some(m) = mv {
        pos.do_move(m);
    }
    if let Some(child_rows) = children_of.get(&id) {
        for &child in child_rows {
            walk(
                tree,
                ids,
                stored,
                raw,
                children_of,
                child,
                Some(node_idx),
                pos,
                visited,
            )?;
        }
    }
    if let Some(m) = mv {
        pos.undo_move(m);
    }
    Ok(())
}

/// The fold digest (D5): sha256 over the sorted `tag sha256` lines of the
/// post-fold `shards` table. Deterministic, honest provenance-set digest —
/// the new `meta.built_from`.
#[must_use]
pub fn fold_digest(rows: &[ShardRow]) -> String {
    let mut lines: Vec<String> = rows
        .iter()
        .map(|r| format!("{} {}\n", r.tag, r.sha256))
        .collect();
    lines.sort();
    digest_hex(lines.join("").as_bytes())
}

/// Write the fold's delta inside the caller's transaction: new node rows,
/// UPDATEs for existing rows whose recomputed values differ, one `shards`
/// insert per folded tag, and the meta refresh (`node_count`, `built_from`
/// = the fold digest, `generator`). Existing outcome fields must agree with
/// the recomputed tree and a stored bound must never *increase* (the
/// fixpoint only refines) — anything else is corruption and aborts.
/// Returns `(inserted, updated)`.
///
/// # Errors
/// Any SQLite error, an outcome flip, or a bound increase on an existing row.
pub fn write_fold_delta(
    conn: &Connection,
    tree: &PathTree,
    state: &LoadedState,
    new_shard_rows: &[ShardRow],
    digest: &str,
) -> Res<(usize, usize)> {
    let ids = RowIds::fold(tree, &state.ids, state.max_id);
    let mut inserted = 0usize;
    let mut updated = 0usize;
    // Existing rows: compare-and-update (this re-derives "stored values ==
    // finalized values" on every fold; the update itself covers refinements,
    // provenance unions, and status changes).
    for (idx, st) in state.stored.iter().enumerate() {
        let node = &tree.nodes[idx];
        let outcome_s = node.outcome.map(|o| o.as_str().to_string());
        if outcome_s != st.outcome {
            return Err(format!(
                "node id {}: stored outcome {:?} != recomputed {outcome_s:?} \
                 (corrupted standing layer)",
                st.id, st.outcome
            ));
        }
        let bound = tree.depth_bound(idx).map(i64::from);
        if bound > st.depth_bound {
            return Err(format!(
                "node id {}: stored bound {:?} < recomputed bound {bound:?} (the \
                 fixpoint only refines — corrupted standing layer)",
                st.id, st.depth_bound
            ));
        }
        let status = tree.depth_status(idx).map(|s| s.as_str().to_string());
        let prov = node
            .provenance
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        if bound != st.depth_bound || status != st.depth_status || prov != st.provenance {
            conn.execute(
                "UPDATE nodes SET outcome=?1, depth_bound=?2, depth_status=?3, \
                 provenance=?4 WHERE id=?5",
                rusqlite::params![outcome_s, bound, status, prov, st.id],
            )
            .map_err(|e| format!("cannot update node {}: {e}", st.id))?;
            updated += 1;
        }
    }
    // New rows: appended ids (D4), parents already assigned.
    let mut stmt = conn
        .prepare("INSERT INTO nodes VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")
        .map_err(|e| e.to_string())?;
    for &idx in &ids.order {
        if idx < state.ids.len() {
            continue;
        }
        let node = &tree.nodes[idx];
        let parent_row = node.parent.map(|p| ids.id_of[p]);
        let (mv_uci, mv_code) = node.mv.map_or((None, None), |mv| {
            (Some(move_to_uci(mv)), Some(i64::from(move_to_bits(mv))))
        });
        stmt.execute(rusqlite::params![
            ids.id_of[idx],
            parent_row,
            node.ply as i64,
            mv_uci,
            mv_code,
            node.outcome.map(|o| o.as_str().to_string()),
            tree.depth_bound(idx),
            tree.depth_status(idx).map(|s| s.as_str().to_string()),
            node.provenance
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(","),
        ])
        .map_err(|e| format!("cannot insert node {}: {e}", ids.id_of[idx]))?;
        inserted += 1;
    }
    let mut stmt = conn
        .prepare("INSERT INTO shards VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")
        .map_err(|e| e.to_string())?;
    for s in new_shard_rows {
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
    let node_count = tree.nodes.len().to_string();
    let mut stmt = conn
        .prepare("INSERT OR REPLACE INTO meta VALUES (?1, ?2)")
        .map_err(|e| e.to_string())?;
    for (k, v) in [
        ("node_count", node_count.as_str()),
        ("built_from", digest),
        ("generator", GENERATOR),
    ] {
        stmt.execute(rusqlite::params![k, v])
            .map_err(|e| format!("cannot refresh meta {k}: {e}"))?;
    }
    Ok((inserted, updated))
}

/// Run one fold invocation (plan22 D3): one transaction over the standing
/// DB — ensure schema, optional legacy censor import (in place, max rule),
/// load the standing tree, fold every unmerged shard, finalize, per-graft
/// skeleton re-validation, write the delta, commit. Any defect rolls the
/// whole invocation back.
///
/// # Errors
/// Any defect named in [`load_standing_tree`], [`load_shard`],
/// [`write_fold_delta`], or the overlay/skeleton machinery.
pub fn fold(
    conn: &mut Connection,
    shard_dir: &Path,
    manifest: Option<&Manifest>,
    import_censor: Option<&Path>,
) -> Res<FoldOutput> {
    let tx = conn
        .transaction()
        .map_err(|e| format!("cannot begin fold transaction: {e}"))?;
    ensure_fold_schema(&tx)?;
    let mut report = FoldReport::default();

    // The legacy censor import (D3): in place, under the per-field max rule
    // — only the imported keys are rewritten.
    if let Some(path) = import_censor {
        let mut ledger = Ledger::load_db(&tx)?;
        let legacy = Ledger::load_legacy_json(path)?;
        report.imported_censor = legacy.len();
        for (k, e) in legacy.iter() {
            let clamped = LedgerEntry {
                work_done: e.work_done.min(i64::MAX as u64),
                passes_failed: e.passes_failed,
            };
            ledger.upsert_max(k, clamped);
        }
        for (k, _) in legacy.iter() {
            let e = ledger.get(k).expect("upserted above");
            insert_censor_row(
                &tx,
                &CensorRow {
                    path: k.clone(),
                    work_done: e.work_done,
                    passes_failed: e.passes_failed,
                },
            )?;
        }
    }

    let loaded = load_standing_tree(&tx)?;
    let existing_tags: HashSet<&str> = loaded
        .state
        .shard_rows
        .iter()
        .map(|r| r.tag.as_str())
        .collect();
    let manifest_by_tag: HashMap<&str, &ShardEntry> = manifest
        .map(|m| m.entries.iter().map(|e| (e.tag.as_str(), e)).collect())
        .unwrap_or_default();
    let scan = sidecar::scan_shard_dir(shard_dir)?;

    // D1: a sidecar without its bin is a durable-layer defect.
    for tag in &scan.sidecar_tags {
        if !scan.bin_tags.iter().any(|b| b == tag) {
            return Err(format!(
                "shard {tag}: sidecar without bin — durable-layer defect \
                 (a sidecar is only written after its bin)"
            ));
        }
    }

    let mut tree = loaded.tree;
    let mut new_shard_rows: Vec<ShardRow> = Vec::new();
    let mut graft_roots: Vec<(usize, String)> = Vec::new();
    for tag in &scan.bin_tags {
        if existing_tags.contains(tag.as_str()) {
            // Idempotent skip (D3): cross-check the standing sha256 against
            // the bin on disk — same tag, different bytes is corruption or a
            // re-exported different tree; never patched.
            let bytes = std::fs::read(shard_dir.join(format!("{tag}.bin")))
                .map_err(|e| format!("shard {tag}: cannot read standing bin: {e}"))?;
            let digest = digest_hex(&bytes);
            let row = loaded
                .state
                .shard_rows
                .iter()
                .find(|r| r.tag == *tag)
                .expect("tag came from existing_tags");
            if row.sha256 != digest {
                return Err(format!(
                    "shard {tag}: standing sha256 mismatch (DB says {}, the bin \
                     digests to {digest} — same tag, different bytes is never patched)",
                    row.sha256
                ));
            }
            report.already += 1;
            continue;
        }
        let (meta, from_sidecar) = if scan.sidecar_tags.contains(tag) {
            let side = sidecar::read_sidecar(&sidecar::sidecar_path(shard_dir, tag))?;
            if side.tag != *tag {
                return Err(format!(
                    "shard {tag}: sidecar tag field is {:?} — refusing the mismatch",
                    side.tag
                ));
            }
            (ShardMeta::from_sidecar(&side), true)
        } else if let Some(entry) = manifest_by_tag.get(tag.as_str()) {
            (ShardMeta::from_manifest_entry(entry), false)
        } else {
            // D1: a bin without a sidecar is the interrupted job's orphan.
            report.orphans += 1;
            eprintln!(
                "note: orphan bin {tag}.bin without sidecar or manifest entry — \
                 skipped (the retry overwrites determinically)"
            );
            continue;
        };
        let (shard, digest, moves) = load_shard(&meta, shard_dir)?;
        report.shard_nodes_sum += shard.nodes.len();
        let graft = tree.graft_path(&moves, &meta.tag);
        tree.overlay_subtree(graft, &shard, &meta.tag)?;
        graft_roots.push((graft, meta.fen.clone()));
        new_shard_rows.push(ShardRow {
            tag: meta.tag.clone(),
            file: meta.file.clone(),
            sha256: digest.clone(),
            root_fen: shard.root_fen.clone(),
            path: meta.moves.join(" "),
            outcome: meta.outcome,
            depth_bound: shard.nodes[0].depth,
            n_nodes: shard.nodes.len(),
        });
        if !from_sidecar {
            // D2: backfill the sidecar (the manifest was its metadata
            // source; the computed digest makes it self-describing now).
            sidecar::write_sidecar(
                shard_dir,
                &SidecarEntry {
                    tag: meta.tag.clone(),
                    file: meta.file.clone(),
                    fen: meta.fen.clone(),
                    moves: meta.moves.clone(),
                    outcome: meta.outcome,
                    sha256: digest,
                },
            )?;
            report.sidecars_backfilled += 1;
        }
        report.folded += 1;
    }

    // D2: a manifest entry whose bin is missing stays a hard abort — for
    // unmerged tags without a sidecar the manifest is the only metadata
    // source. Already-folded tags and sidecar-covered tags skip (sidecar
    // wins; folded tags' bins are deletable, D8).
    if let Some(m) = manifest {
        for e in &m.entries {
            if existing_tags.contains(e.tag.as_str())
                || scan.sidecar_tags.contains(&e.tag)
                || scan.bin_tags.iter().any(|b| b == &e.tag)
            {
                continue;
            }
            return Err(format!(
                "manifest entry {}: neither bin ({}) nor sidecar exists",
                e.tag, e.file
            ));
        }
    }

    tree.finalize()?;
    for (root_id, fen) in &graft_roots {
        let skeleton = tree.reassemble_skeleton(*root_id, fen)?;
        if let Err(defects) = validate_proof_tree(&skeleton) {
            let path = tree.path_uci(*root_id).join(" ");
            return Err(format!(
                "skeleton re-validation failed at graft \"{}\": {} defects: {}",
                if path.is_empty() { "<root>" } else { &path },
                defects.len(),
                defects
                    .iter()
                    .take(3)
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        report.skeleton_revalidations += 1;
    }

    let mut all_rows = loaded.state.shard_rows.clone();
    all_rows.extend(new_shard_rows.iter().cloned());
    let digest = fold_digest(&all_rows);
    let (inserted, updated) =
        write_fold_delta(&tx, &tree, &loaded.state, &new_shard_rows, &digest)?;
    tx.commit()
        .map_err(|e| format!("cannot commit fold: {e}"))?;

    report.inserted_nodes = inserted;
    report.updated_nodes = updated;
    report.overlay_insertions = tree.overlay_insertions;
    report.ancestor_insertions = tree.ancestor_insertions;
    report.open_upgraded = tree.open_upgraded;
    report.node_count = tree.nodes.len();
    report.proven = tree.nodes.iter().filter(|n| n.outcome.is_some()).count();
    report.open = report.node_count - report.proven;
    report.fold_digest = digest;
    let ids = RowIds::fold(&tree, &loaded.state.ids, loaded.state.max_id);
    Ok(FoldOutput { tree, ids, report })
}

#[cfg(test)]
mod tests {
    use super::super::super::schema::canonical_dump_with_ids;
    use super::*;

    /// A fresh-init fold assigns ids over the whole tree in global
    /// lexicographic path order — identical to `RowIds::from_finalize` (D4's
    /// carve-out).
    #[test]
    fn fresh_init_ids_equal_finalize_order() {
        let mut t = PathTree::new();
        t.finalize().unwrap();
        let a = RowIds::from_finalize(&t);
        let b = RowIds::fold(&t, &[], -1);
        assert_eq!(a.id_of, b.id_of);
        assert_eq!(a.order, b.order);
    }

    /// `canonical_dump_with_ids` over the fold assignment equals the
    /// finalize-order dump for a fresh fold.
    #[test]
    fn fold_dump_equals_fresh_dump() {
        let mut t = PathTree::new();
        t.finalize().unwrap();
        let ids = RowIds::fold(&t, &[], -1);
        assert_eq!(
            super::super::super::schema::canonical_dump(Position::STARTPOS_FEN, "d", &t),
            canonical_dump_with_ids(Position::STARTPOS_FEN, "d", &t, &ids)
        );
    }

    /// The fold digest of an empty `shards` table is sha256 of the empty
    /// string.
    #[test]
    fn fold_digest_empty() {
        assert_eq!(
            fold_digest(&[]),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
