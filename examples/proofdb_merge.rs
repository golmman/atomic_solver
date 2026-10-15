//! `proofdb_merge` — the single-writer merger of the startpos proof-line
//! database (proofdb initiative, working layer).
//!
//! Pipeline per shard: read the binary proof tree → cross-check the manifest
//! fields against the tree root → replay-validate the tree (abort on any
//! defect) → replay the manifest path from the startpos and check hash
//! fidelity (the replayed position, the manifest FEN, and the tree root FEN
//! must be the same position) → graft open ancestors → overlay the proven
//! subtree. After all shards: the depth-refinement fixpoint
//! (`proofdb::merge::PathTree::finalize`), per-graft skeleton re-validation,
//! then the SQLite DB (`docs/spec/global_proof_store.md`, schema v2) and the
//! canonical dump.
//!
//! Censor state (plan20 D5/D6): the merger is the sole writer of
//! `nodes`/`shards`/`meta` but *preserves* the DB's `censor` rows (the
//! relocated work ledger — harvest's selection state) across its full
//! rebuild: rows are read from the existing DB before it is replaced and
//! re-inserted into the fresh one, never modified. `--import-censor
//! <work.json>` additionally merges a legacy pre-plan20 JSON ledger into
//! the table under the per-field max rule (idempotent; never decreases a
//! stored value).
//!
//! Output: a census on stdout; the DB at `--db` and the dump at `--dump`
//! when requested. Generated state defaults wholly under `data/proofdb/`
//! (plan14): the DB to `data/proofdb/proofdb.db`, the manifest to
//! `data/proofdb/shards/manifest.json`, the shards to `data/proofdb/shards/`
//! — a production merge needs no path flags. Any validation defect,
//! cross-check mismatch, or proven-outcome conflict aborts with a non-zero
//! exit and writes nothing — conflicts are never patched.
//!
//! File-size justification: 15.5 KB — the argument parsing, the shard
//! pipeline, the censor collect/preserve/import step (plan20), and the
//! census/summary emission are one self-contained CLI flow; the heavy
//! lifting is already split into `proofdb/merge/`.

use std::path::{Path, PathBuf};

use atomic_movegen::types::Move;

use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::Position;
use atomic_solver::proof_tree::{ProofTree, validate_proof_tree};

mod proofdb;

use proofdb::ledger::Ledger;
use proofdb::merge::PathTree;
use proofdb::schema::{CensorRow, ShardRow, canonical_dump, read_censor_rows, write_db};
use proofdb::{
    DEFAULT_DB, DEFAULT_MANIFEST, DEFAULT_SHARD_DIR, ShardEntry, create_parent_dir, digest_hex,
    read_manifest,
};

struct Census {
    shards_total: usize,
    shards_merged: usize,
    tree_nodes_sum: usize,
    skeleton_revalidations: usize,
}

fn fail(msg: &str) -> ! {
    eprintln!("proofdb_merge: CONFLICT/DEFECT: {msg}");
    std::process::exit(2);
}

fn usage() -> ! {
    eprintln!(
        "usage: proofdb_merge [--manifest <index.json>] [--shard-dir <dir>] \
         [--db <out.db>] [--dump <nodes.txt>] [--sample-lines <n>] \
         [--import-censor <work.json>]  \
         (defaults: --manifest data/proofdb/shards/manifest.json \
         --shard-dir data/proofdb/shards --db data/proofdb/proofdb.db)"
    );
    std::process::exit(1);
}

struct Args {
    manifest: PathBuf,
    shard_dir: PathBuf,
    db: PathBuf,
    dump: Option<PathBuf>,
    sample_lines: usize,
    import_censor: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut args = Args {
        manifest: PathBuf::from(DEFAULT_MANIFEST),
        shard_dir: PathBuf::from(DEFAULT_SHARD_DIR),
        db: PathBuf::from(DEFAULT_DB),
        dump: None,
        sample_lines: 0,
        import_censor: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut next = || it.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "--manifest" => args.manifest = PathBuf::from(next()),
            "--shard-dir" => args.shard_dir = PathBuf::from(next()),
            "--db" => args.db = PathBuf::from(next()),
            "--dump" => args.dump = Some(PathBuf::from(next())),
            "--import-censor" => args.import_censor = Some(PathBuf::from(next())),
            "--sample-lines" => {
                let v = next().parse::<usize>().unwrap_or_else(|_e| usage());
                args.sample_lines = v;
            }
            _ => usage(),
        }
    }
    args
}

/// The censor state for the fresh DB (plan20 D5/D6): the existing DB's rows
/// (preserved across the full rebuild — a bare merge-after-batch or recipe
/// R5 must never wipe selection state) plus, with `--import-censor`, a
/// legacy JSON work ledger merged in under the per-field max rule
/// (`work_done`/`passes_failed` each keep the max; a `work_done` beyond the
/// i64 storage width saturates on import). Idempotent for a repeated
/// import; never decreases a stored value.
fn collect_censor_rows(args: &Args) -> Result<Vec<CensorRow>, String> {
    let mut merged: std::collections::BTreeMap<String, proofdb::ledger::LedgerEntry> =
        std::collections::BTreeMap::new();
    if args.db.exists() {
        let conn = rusqlite::Connection::open_with_flags(
            &args.db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| format!("cannot open existing DB {}: {e}", args.db.display()))?;
        for row in read_censor_rows(&conn)? {
            merged.insert(
                row.path,
                proofdb::ledger::LedgerEntry {
                    work_done: row.work_done,
                    passes_failed: row.passes_failed,
                },
            );
        }
    }
    let preserved = merged.len();
    let imported = match &args.import_censor {
        None => 0,
        Some(path) => {
            let legacy = Ledger::load_legacy_json(path)?;
            let n = legacy.len();
            for (k, e) in legacy.iter() {
                let e = proofdb::ledger::LedgerEntry {
                    work_done: e.work_done.min(i64::MAX as u64),
                    passes_failed: e.passes_failed,
                };
                match merged.get_mut(k.as_str()) {
                    Some(cur) => {
                        cur.work_done = cur.work_done.max(e.work_done);
                        cur.passes_failed = cur.passes_failed.max(e.passes_failed);
                    }
                    None => {
                        merged.insert(k.clone(), e);
                    }
                }
            }
            n
        }
    };
    if preserved > 0 || imported > 0 {
        println!(
            "censor: {preserved} censor rows preserved + {imported} imported records -> {} rows",
            merged.len()
        );
    }
    Ok(merged
        .into_iter()
        .map(|(path, e)| CensorRow {
            path,
            work_done: e.work_done,
            passes_failed: e.passes_failed,
        })
        .collect())
}

/// Load, cross-check, replay-validate, and hash-verify one shard. Returns the
/// tree, its file digest, and the manifest path as resolved `Move`s (the
/// startpos replay doubles as the hash-fidelity check).
///
/// Aborts (non-zero exit) on any defect: conflicts are never patched.
fn load_shard(entry: &ShardEntry, shard_dir: &Path) -> (ProofTree, String, Vec<Move>) {
    if entry.validate != "ok" {
        fail(&format!(
            "shard {}: manifest validate={} (only 'ok' enters a merge)",
            entry.tag, entry.validate
        ));
    }
    let path = shard_dir.join(&entry.file);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        fail(&format!(
            "shard {}: cannot read {}: {e}",
            entry.tag,
            path.display()
        ))
    });
    let digest = digest_hex(&bytes);
    let tree = ProofTree::from_bin(&mut bytes.as_slice()).unwrap_or_else(|e| {
        fail(&format!(
            "shard {}: cannot parse binary tree: {e}",
            entry.tag
        ))
    });
    if let Err(defects) = validate_proof_tree(&tree) {
        fail(&format!(
            "shard {}: replay validation found {} defects: {}",
            entry.tag,
            defects.len(),
            defects
                .iter()
                .take(3)
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    // Cross-check: manifest outcome vs tree root outcome.
    let root_outcome = tree.nodes[0]
        .outcome
        .unwrap_or_else(|| fail(&format!("shard {}: tree root is unrealized", entry.tag)));
    if root_outcome != entry.outcome {
        fail(&format!(
            "shard {}: manifest outcome {} != tree root outcome {}",
            entry.tag,
            entry.outcome.as_str(),
            root_outcome.as_str()
        ));
    }
    // Hash fidelity: the replayed path, the manifest FEN, and the tree root
    // FEN must all be the same position (subsumes FEN normalization and the
    // halfmove clock).
    let mut pos = Position::from_fen(Position::STARTPOS_FEN).expect("startpos parses");
    let mut moves = Vec::with_capacity(entry.moves.len());
    for uci in &entry.moves {
        let Some(mv) = uci_to_move(uci, &pos) else {
            fail(&format!(
                "shard {}: manifest move {uci:?} is not legal on the replayed path",
                entry.tag
            ));
        };
        pos.do_move(mv);
        moves.push(mv);
    }
    let manifest_pos = Position::from_fen(&entry.fen)
        .unwrap_or_else(|e| fail(&format!("shard {}: bad manifest fen: {e}", entry.tag)));
    let tree_pos = Position::from_fen(&tree.root_fen)
        .unwrap_or_else(|e| fail(&format!("shard {}: bad tree root fen: {e}", entry.tag)));
    let rh = pos.hash();
    if manifest_pos.hash() != rh || tree_pos.hash() != rh {
        fail(&format!(
            "shard {}: hash fidelity failed (replayed {:016x}, manifest fen {:016x}, \
             tree root {:016x})",
            entry.tag,
            rh,
            manifest_pos.hash(),
            tree_pos.hash()
        ));
    }
    (tree, digest, moves)
}

/// Deterministic splitmix64 RNG for the `--sample-lines` spot-checks.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

fn sample_lines(tree: &PathTree, n: usize) {
    let mut rng = Rng(0x2026_0928);
    for k in 1..=n {
        let mut cur = 0usize;
        let mut uci: Vec<String> = Vec::new();
        while !tree.nodes[cur].children.is_empty() {
            let kids: Vec<usize> = tree.nodes[cur].children.values().copied().collect();
            cur = kids[(rng.next() % kids.len() as u64) as usize];
            uci.push(move_to_uci(
                tree.nodes[cur].mv.expect("non-root has a move"),
            ));
        }
        let end = match tree.nodes[cur].outcome {
            None => "open".to_string(),
            Some(o) => format!(
                "{} bound {} ({})",
                o.as_str(),
                tree.depth_bound(cur).expect("proven node has a depth"),
                tree.depth_status(cur)
                    .expect("proven node has a status")
                    .as_str(),
            ),
        };
        println!(
            "sample {k}: {} ({} plies, end: {end})",
            uci.join(" "),
            uci.len()
        );
    }
}

fn main() {
    let args = parse_args();
    // The censor state (plan20 D5/D6) must be read before write_db replaces
    // the DB file.
    let censor_rows = collect_censor_rows(&args).unwrap_or_else(|e| fail(&e));
    let manifest = read_manifest(&args.manifest).unwrap_or_else(|e| fail(&e));
    let mut census = Census {
        shards_total: manifest.entries.len(),
        shards_merged: 0,
        tree_nodes_sum: 0,
        skeleton_revalidations: 0,
    };
    let mut tree = PathTree::new();
    let mut shard_rows: Vec<ShardRow> = Vec::new();
    let mut graft_roots: Vec<(usize, String)> = Vec::new(); // (node idx, shard root fen)

    for entry in &manifest.entries {
        let (shard, digest, moves) = load_shard(entry, &args.shard_dir);
        census.tree_nodes_sum += shard.nodes.len();
        let graft = tree.graft_path(&moves, &entry.tag);
        if let Err(e) = tree.overlay_subtree(graft, &shard, &entry.tag) {
            fail(&format!("shard {}: {e}", entry.tag));
        }
        graft_roots.push((graft, entry.fen.clone()));
        shard_rows.push(ShardRow {
            tag: entry.tag.clone(),
            file: entry.file.clone(),
            sha256: digest,
            root_fen: shard.root_fen.clone(),
            path: entry.moves.join(" "),
            outcome: entry.outcome,
            depth_bound: shard.nodes[0].depth,
            n_nodes: shard.nodes.len(),
        });
        census.shards_merged += 1;
    }

    if let Err(e) = tree.finalize() {
        fail(&e);
    }
    // Gate: re-assemble the proof skeleton at every graft root and
    // replay-validate it (the "always again at finalization" leg).
    for (root_id, fen) in &graft_roots {
        let skeleton = tree
            .reassemble_skeleton(*root_id, fen)
            .unwrap_or_else(|e| fail(&e));
        if let Err(defects) = validate_proof_tree(&skeleton) {
            let path = tree.path_uci(*root_id).join(" ");
            fail(&format!(
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
        census.skeleton_revalidations += 1;
    }

    // The default DB path lives under data/proofdb/ — create it on first
    // write (reading the manifest/shards never creates anything).
    create_parent_dir(&args.db).unwrap_or_else(|e| fail(&e));
    if let Err(e) = write_db(
        &args.db,
        Position::STARTPOS_FEN,
        &manifest.sha256_hex,
        &shard_rows,
        &tree,
        &censor_rows,
    ) {
        fail(&e);
    }
    if let Some(dump_path) = &args.dump {
        let dump = canonical_dump(Position::STARTPOS_FEN, &manifest.sha256_hex, &tree);
        std::fs::write(dump_path, dump)
            .unwrap_or_else(|e| fail(&format!("cannot write dump: {e}")));
    }
    if args.sample_lines > 0 {
        sample_lines(&tree, args.sample_lines);
    }

    let proven = tree.nodes.iter().filter(|n| n.outcome.is_some()).count();
    let open = tree.nodes.len() - proven;
    let dedupes = census
        .tree_nodes_sum
        .checked_sub(tree.overlay_insertions)
        .expect("overlay insertions cannot exceed shard nodes");
    println!(
        "census: shards merged {}/{}; shard tree nodes {}; overlay new {}; \
         dedupes {}; open-ancestor insertions {}; open nodes upgraded {}; \
         merged nodes {} (proven {proven}, open {open}); skeleton re-validations {}",
        census.shards_merged,
        census.shards_total,
        census.tree_nodes_sum,
        tree.overlay_insertions,
        dedupes,
        tree.ancestor_insertions,
        tree.open_upgraded,
        tree.nodes.len(),
        census.skeleton_revalidations,
    );
    println!(
        "node arithmetic: {} = 1 root + {} overlay + {} ancestors; shard nodes {} = {} new + {} deduped",
        tree.nodes.len(),
        tree.overlay_insertions,
        tree.ancestor_insertions,
        census.tree_nodes_sum,
        tree.overlay_insertions,
        dedupes,
    );
    println!(
        "db: {} (nodes {}), manifest built_from {}",
        args.db.display(),
        tree.row_order.len(),
        &manifest.sha256_hex[..16],
    );
}
