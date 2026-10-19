//! Integration harness for the plan22 incremental fold, sidecar durable
//! layer, and `proofdb_verify` (R1–R8 of `plan22.md` §4). Split from
//! `tests/proofdb.rs` (report12 finding: that target kept growing); the
//! module tree is included verbatim via `#[path]` so the inline unit tests
//! run here too.
//!
//! Contracts under test:
//! - **R1** — a fresh fold of the committed fixture shard set (manifest
//!   present ⇒ sidecar backfill) is id- and value-identical to the retired
//!   full rebuild (`write_db` over the manifest), `proofdb_verify`-clean,
//!   and re-folds idempotently.
//! - **R2** — two fresh folds of the same shard set are byte-identical
//!   (the surviving determinism gate).
//! - **R3** — a delta fold preserves existing row ids and appends new ones
//!   past `max(id)`.
//! - **R4** — a conflict (same tag different bytes; stored-vs-shard
//!   outcome contradiction) aborts without writes.
//! - **R5** — crash windows: a bin without a sidecar is an ignored orphan;
//!   a sidecar without a bin and a bin/sidecar sha mismatch abort.
//! - **R6** — the unmerged-shard gate refuses harvest/flip until the fold.
//! - **R7** — censor discipline: the fold leaves censor rows in place;
//!   `--import-censor` is the max rule and idempotent.
//! - **R8** — `verify_db` is green on a folded DB and red on corrupted
//!   copies (missing child / flipped outcome / illegal move / ply break /
//!   node_count mismatch / empty provenance).

#[path = "../examples/proofdb/mod.rs"]
mod proofdb;

use std::collections::HashSet;

use rusqlite::Connection;

use atomic_movegen::types::Move;

use atomic_solver::notation::uci_to_move;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofNode, ProofTree, validate_proof_tree};

use proofdb::db::check_unmerged_shards;
use proofdb::manifest::read_manifest;
use proofdb::merge::PathTree;
use proofdb::merge::fold::{self, FoldOutput};
use proofdb::schema::{ShardRow, canonical_dump_with_ids, read_censor_rows};
use proofdb::sidecar::{SidecarEntry, write_sidecar};
use proofdb::verify::{Verdict, verify_db};

const ROOT_FEN: &str = Position::STARTPOS_FEN;
/// After 1.f3 e6 2.g4: Qd8-h4 explodes the White king (win in 1).
const TACTIC_FEN: &str = "rnbqkbnr/pppp1ppp/4p3/8/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2";
const TACTIC_PATH: &[&str] = &["f2f3", "e7e6", "g2g4"];
/// After 1.f3 e5 2.g4: the same Qd8-h4 win over a different black reply —
/// a second, disjoint graft into the `f2f3`/`g2g4` ancestor chain.
const TACTIC2_FEN: &str = "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2";
const TACTIC2_PATH: &[&str] = &["f2f3", "e7e5", "g2g4"];

// --- shared helpers ---

/// A win-in-1 fixture tree (root + terminal child) at `fen`; the mating
/// move `d8h4` is legal in both tactic positions.
fn tactic_tree_at(fen: &str) -> ProofTree {
    let mut pos = Position::from_fen(fen).unwrap();
    let mv = uci_to_move("d8h4", &pos).expect("d8h4 is legal and wins");
    pos.do_move(mv);
    let child = ProofNode {
        parent: Some(std::num::NonZeroU32::new(1).expect("nonzero")),
        first_child: None,
        next_sibling: None,
        mv,
        hash: 0,
        outcome: Some(Outcome::Loss),
        depth: 0,
    };
    let root = ProofNode {
        parent: None,
        first_child: Some(std::num::NonZeroU32::new(1).expect("nonzero")),
        next_sibling: None,
        mv: Move::NONE,
        hash: 0,
        outcome: Some(Outcome::Win),
        depth: 1,
    };
    let tree = ProofTree {
        root_fen: fen.to_string(),
        nodes: vec![root, child],
    };
    assert!(
        validate_proof_tree(&tree).is_ok(),
        "fixture must be a valid proof"
    );
    tree
}

/// The win-in-1 fixture tree (root + terminal child) at `TACTIC_FEN`.
fn tactic_tree() -> ProofTree {
    tactic_tree_at(TACTIC_FEN)
}

fn moves_from_path(ucis: &[&str]) -> Vec<Move> {
    let mut pos = Position::from_fen(ROOT_FEN).unwrap();
    ucis.iter()
        .map(|u| {
            let mv = uci_to_move(u, &pos).expect("legal");
            pos.do_move(mv);
            mv
        })
        .collect()
}

/// Write a self-describing shard pair (bin + sidecar) into `shard_dir`.
fn write_shard_pair(shard_dir: &std::path::Path, tag: &str, moves: &[&str], tree: &ProofTree) {
    let mut bytes = Vec::new();
    tree.to_bin(&mut bytes).expect("tree serializes");
    std::fs::write(shard_dir.join(format!("{tag}.bin")), &bytes).unwrap();
    write_sidecar(
        shard_dir,
        &SidecarEntry {
            tag: tag.to_string(),
            file: format!("{tag}.bin"),
            fen: tree.root_fen.clone(),
            moves: moves.iter().map(|s| s.to_string()).collect(),
            outcome: tree.nodes[0].outcome.expect("proven fixture root"),
            sha256: proofdb::digest_hex(&bytes),
        },
    )
    .unwrap();
}

/// A fresh fold over an empty (new-file) DB.
fn fold_fresh(shard_dir: &std::path::Path) -> FoldOutput {
    let db = shard_dir.parent().unwrap().join("fold.db");
    std::fs::remove_file(&db).ok();
    let mut conn = Connection::open(&db).unwrap();
    fold::fold(&mut conn, shard_dir, None, None).unwrap()
}

/// The full `nodes` table as sorted `id|values` strings (the R4
/// without-writes snapshot).
fn db_rows(db: &std::path::Path) -> Vec<String> {
    let conn = Connection::open(db).unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT id, parent_id, ply, move_uci, move_code, outcome, depth_bound, \
             depth_status, provenance FROM nodes ORDER BY id",
        )
        .unwrap();
    let nodes: Vec<String> = stmt
        .query_map([], |r| {
            Ok(format!(
                "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, String>(8)?
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let shards: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT tag, sha256 FROM shards ORDER BY tag")
            .unwrap();
        stmt.query_map([], |r| {
            Ok(format!(
                "{:?}|{:?}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    };
    let mut out = nodes;
    out.extend(shards);
    out
}

fn shard_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("proofdb_fold_{name}_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("shards")).unwrap();
    dir
}

// --- R1: migration equivalence (the strongest check) ---

/// A fresh fold of the committed fixture shard set (262 bins + manifest —
/// the manifest is the legacy metadata source and the merge backfills the
/// sidecars) produces exactly the retired full rebuild's `nodes` table:
/// same row ids (the fresh-init carve-out assigns global lexicographic
/// path order, the old `finalize` assignment), same depths, statuses,
/// provenance — the canonical dumps agree byte-for-byte (the `built_from`
/// line now carries the fold digest on both sides). Node census: the
/// plan12-audit pins (55,703 nodes, 55,628 proven). Then `verify_db` is
/// green, and a re-fold is a no-op (`folded 0`, identical dump).
#[test]
fn r1_fresh_fold_matches_the_rebuild_and_re_folds_idempotently() {
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = crate_root.join("docs/plans/proofdb/shards");
    let dir = shard_dir("r1");
    // Copy the committed fixture set (bins + manifest) into the temp
    // shard dir — a test must never write back into the checkout.
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        std::fs::copy(e.path(), dir.join("shards").join(e.file_name())).unwrap();
    }
    let manifest = read_manifest(&dir.join("shards/manifest.json")).unwrap();
    assert_eq!(manifest.entries.len(), 262, "standing manifest entry count");

    // (a) The fresh fold (manifest present ⇒ sidecar backfill).
    let db = dir.join("fold.db");
    let mut conn = Connection::open(&db).unwrap();
    let out = fold::fold(&mut conn, &dir.join("shards"), Some(&manifest), None).unwrap();
    let r = &out.report;
    assert_eq!(r.folded, 262);
    assert_eq!(r.already, 0);
    assert_eq!(r.sidecars_backfilled, 262, "every manifest tag backfilled");
    assert_eq!(r.node_count, 55_703, "merged node count");
    assert_eq!(r.proven, 55_628, "proven node count");
    assert_eq!(r.fold_digest.len(), 64);
    // The fold digest pin (sha256 over the sorted `tag sha256` lines of
    // the post-fold shards table — deterministic over the committed bins).
    assert_eq!(
        r.fold_digest, "14fd07530996eb9eddaa2b5d3e112d51d57339a892f7f8d0bb166205180a04e2",
        "the fixture shard set's fold digest"
    );
    // The meta row carries the fold digest and node count.
    let built_from: String = conn
        .query_row("SELECT value FROM meta WHERE key='built_from'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let node_count: String = conn
        .query_row("SELECT value FROM meta WHERE key='node_count'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(built_from, r.fold_digest);
    assert_eq!(node_count, "55703");

    // (b) The retired full rebuild over the same manifest, with the fold
    // digest as `built_from`: the canonical dumps must be byte-identical
    // (same ids — the fresh-init carve-out — same values).
    let mut tree = PathTree::new();
    let mut rows: Vec<ShardRow> = Vec::new();
    for entry in &manifest.entries {
        let bytes = std::fs::read(dir.join("shards").join(&entry.file)).unwrap();
        let mut shard = ProofTree::from_bin(&mut bytes.as_slice()).unwrap();
        validate_proof_tree(&shard).unwrap();
        assert_eq!(shard.nodes[0].outcome, Some(entry.outcome));
        let moves = moves_from_path(&entry.moves.iter().map(String::as_str).collect::<Vec<_>>());
        let graft = tree.graft_path(&moves, &entry.tag);
        tree.overlay_subtree(graft, &shard, &entry.tag).unwrap();
        rows.push(ShardRow {
            tag: entry.tag.clone(),
            file: entry.file.clone(),
            sha256: proofdb::digest_hex(&bytes),
            root_fen: std::mem::take(&mut shard.root_fen),
            path: entry.moves.join(" "),
            outcome: entry.outcome,
            depth_bound: shard.nodes[0].depth,
            n_nodes: shard.nodes.len(),
        });
    }
    tree.finalize().unwrap();
    let digest = fold::fold_digest(&rows);
    assert_eq!(digest, r.fold_digest, "the fold digest is manifest-derived");
    let rebuild_dump = canonical_dump_with_ids(
        ROOT_FEN,
        &digest,
        &tree,
        &proofdb::schema::RowIds::from_finalize(&tree),
    );
    let fold_dump = canonical_dump_with_ids(ROOT_FEN, &r.fold_digest, &out.tree, &out.ids);
    assert_eq!(fold_dump, rebuild_dump, "fold == rebuild, byte-for-byte");

    // (c) `proofdb_verify` is green on the folded DB.
    let verdict = verify_db(&conn, 10).unwrap();
    let Verdict::Ok { nodes, proven, .. } = &verdict else {
        panic!("verify must be ok: {verdict:?}");
    };
    assert_eq!(*nodes, 55_703);
    assert_eq!(*proven, 55_628);

    // (d) A re-fold is a no-op: every tag already merged, sha-checked,
    // dump byte-identical.
    let out2 = fold::fold(&mut conn, &dir.join("shards"), Some(&manifest), None).unwrap();
    assert_eq!(out2.report.folded, 0);
    assert_eq!(out2.report.already, 262);
    assert_eq!(out2.report.node_count, 55_703);
    let dump2 = canonical_dump_with_ids(ROOT_FEN, &out2.report.fold_digest, &out2.tree, &out2.ids);
    assert_eq!(dump2, fold_dump, "re-fold leaves the tree unchanged");

    std::fs::remove_dir_all(&dir).ok();
}

// --- R2: deterministic fold ---

/// Two fresh folds of the same sidecar shard set are byte-identical (the
/// surviving determinism gate; the byte-identical *rebuild* contract is
/// retired — the replacement is this pin plus `proofdb_verify`).
#[test]
fn r2_deterministic_fold_is_byte_identical() {
    let dir = shard_dir("r2");
    let tree = tactic_tree();
    write_shard_pair(&dir.join("shards"), "h_one", TACTIC_PATH, &tree);
    let a = fold_fresh(&dir.join("shards"));
    let b = fold_fresh(&dir.join("shards"));
    let dump_a = canonical_dump_with_ids(ROOT_FEN, &a.report.fold_digest, &a.tree, &a.ids);
    let dump_b = canonical_dump_with_ids(ROOT_FEN, &b.report.fold_digest, &b.tree, &b.ids);
    assert_eq!(dump_a, dump_b, "byte-identical folds");
    assert_eq!(a.report.fold_digest, b.report.fold_digest);
    std::fs::remove_dir_all(&dir).ok();
}

// --- R3: delta fold appends, never renumbers ---

/// Fold batch 1, then a second shard sharing the graft path (dedupe +
/// agreement exercised): existing ids are unchanged, new ids are past
/// `max(id)`, the node arithmetic line is consistent, and `built_from`
/// tracks the fold digest.
#[test]
fn r3_delta_fold_preserves_ids_and_appends() {
    let dir = shard_dir("r3");
    let tree = tactic_tree();
    write_shard_pair(&dir.join("shards"), "h_b1", TACTIC_PATH, &tree);
    let out1 = fold_fresh(&dir.join("shards"));
    assert_eq!(out1.report.folded, 1);
    let dump1 = canonical_dump_with_ids(ROOT_FEN, &out1.report.fold_digest, &out1.tree, &out1.ids);
    let max_id1 = *out1.ids.id_of.iter().max().unwrap();

    // Batch 2: a second shard reaching the same win-in-1 subtree through a
    // different legal move order (same TACTIC tree at the same FEN — the
    // dedupe path; agreement, not contradiction).
    write_shard_pair(
        &dir.join("shards"),
        "h_b2",
        TACTIC2_PATH,
        &tactic_tree_at(TACTIC2_FEN),
    );
    let db = dir.join("shards").parent().unwrap().join("fold.db");
    let mut conn = Connection::open(&db).unwrap();
    let out2 = fold::fold(&mut conn, &dir.join("shards"), None, None).unwrap();
    let r2 = &out2.report;
    assert_eq!(r2.folded, 1);
    assert_eq!(r2.already, 1, "the batch-1 tag is idempotently skipped");
    assert_eq!(r2.node_count, out1.report.node_count + r2.inserted_nodes);
    let dump2 = canonical_dump_with_ids(ROOT_FEN, &r2.fold_digest, &out2.tree, &out2.ids);

    // Dump rows: every batch-1 row keeps its id and its value columns;
    // only provenance may widen (the new tag joins the annotation set —
    // D3's provenance unions). Every new row's id is past max_id1.
    let rows1: Vec<&str> = dump1
        .lines()
        .skip_while(|l| l.starts_with('#'))
        .skip(1)
        .collect();
    let rows2: Vec<&str> = dump2
        .lines()
        .skip_while(|l| l.starts_with('#'))
        .skip(1)
        .collect();
    assert!(rows2.len() > rows1.len(), "batch 2 appended rows");
    for (r1, r2) in rows1.iter().zip(rows2.iter()) {
        let f1: Vec<&str> = r1.split('\t').collect();
        let f2: Vec<&str> = r2.split('\t').collect();
        assert_eq!(f1[0], f2[0], "row id unchanged");
        assert_eq!(&f1[..f1.len() - 1], &f2[..f2.len() - 1], "values unchanged");
        assert!(f2[f2.len() - 1].contains("h_b1"), "old provenance retained");
    }
    for line in &rows2[rows1.len()..] {
        let id: i64 = line.split('\t').next().unwrap().parse().unwrap();
        assert!(id > max_id1, "appended id {id} must be past {max_id1}");
    }
    // built_from tracks the fold digest.
    let built_from: String = conn
        .query_row("SELECT value FROM meta WHERE key='built_from'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(built_from, r2.fold_digest);
    std::fs::remove_dir_all(&dir).ok();
}

// --- R4: conflicts abort without writes ---

/// A same-tag/different-bytes bin (corruption or a re-exported different
/// tree) aborts the fold, and a stored-vs-shard outcome contradiction
/// aborts the overlay — in both cases the DB is byte-value-identical to
/// the pre-invocation state (the transaction rolled back).
#[test]
fn r4_conflicts_abort_without_writes() {
    let dir = shard_dir("r4");
    let tree = tactic_tree();
    write_shard_pair(&dir.join("shards"), "h_c1", TACTIC_PATH, &tree);
    fold_fresh(&dir.join("shards"));
    let db = dir.join("shards").parent().unwrap().join("fold.db");
    let before = db_rows(&db);

    // (a) Same tag, different bytes: overwrite the already-folded bin.
    let mut bytes = Vec::new();
    tactic_tree().to_bin(&mut bytes).unwrap();
    bytes[10] ^= 0xff; // corrupt one byte (still parseable or not — the
    // sha check fires first either way)
    std::fs::write(dir.join("shards/h_c1.bin"), &bytes).unwrap();
    let mut conn = Connection::open(&db).unwrap();
    let err = fold::fold(&mut conn, &dir.join("shards"), None, None)
        .err()
        .unwrap();
    assert!(err.contains("sha256 mismatch"), "unexpected: {err}");
    assert_eq!(db_rows(&db), before, "no writes after the abort");
    // Restore the bin (the corrupt bytes are phase (a)'s input only).
    let mut bytes = Vec::new();
    tactic_tree().to_bin(&mut bytes).unwrap();
    std::fs::write(dir.join("shards/h_c1.bin"), &bytes).unwrap();

    // (b) A stored-vs-shard contradiction: corrupt the standing row at the
    // graft path root to `loss`, then fold a second tag at the same path
    // (whose tree says `win`) — the overlay refuses; nothing is written.
    conn.execute(
        "UPDATE nodes SET outcome='loss' WHERE ply=3 AND move_uci='g2g4'",
        [],
    )
    .unwrap();
    let corrupted = db_rows(&db);
    write_shard_pair(&dir.join("shards"), "h_c2", TACTIC_PATH, &tree);
    let err = fold::fold(&mut conn, &dir.join("shards"), None, None)
        .err()
        .unwrap();
    assert!(err.contains("contradiction"), "unexpected: {err}");
    assert_eq!(db_rows(&db), corrupted, "no writes after the abort");
    std::fs::remove_dir_all(&dir).ok();
}

// --- R5: crash windows ---

/// (i) A bin without a sidecar is the interrupted job's orphan: skipped
/// with a census note, the fold still commits; (ii) a sidecar without a
/// bin is a durable-layer defect (abort); (iii) a bin/sidecar sha mismatch
/// aborts.
#[test]
fn r5_crash_windows() {
    let dir = shard_dir("r5");
    let tree = tactic_tree();
    write_shard_pair(&dir.join("shards"), "h_w1", TACTIC_PATH, &tree);

    // (i) The orphan: strip the sidecar — the bin folds as nothing, the
    // census counts it.
    std::fs::remove_file(dir.join("shards/h_w1.meta.json")).unwrap();
    let out = fold_fresh(&dir.join("shards"));
    assert_eq!(out.report.folded, 0);
    assert_eq!(out.report.orphans, 1, "the bin-only orphan is counted");
    assert_eq!(out.report.node_count, 1, "only the standing root");

    // (ii) A sidecar without its bin: hard defect.
    write_shard_pair(&dir.join("shards"), "h_w2", &["a2a3"], &tree);
    std::fs::remove_file(dir.join("shards/h_w2.bin")).unwrap();
    let db = dir.join("shards").parent().unwrap().join("fold.db");
    let mut conn = Connection::open(&db).unwrap();
    let err = fold::fold(&mut conn, &dir.join("shards"), None, None)
        .err()
        .unwrap();
    assert!(err.contains("sidecar without bin"), "unexpected: {err}");
    std::fs::remove_file(dir.join("shards/h_w2.meta.json")).unwrap();

    // (iii) A sha mismatch between bin and sidecar: refuse the shard.
    write_shard_pair(&dir.join("shards"), "h_w3", &["b1c3"], &tree);
    let bytes = std::fs::read(dir.join("shards/h_w3.bin")).unwrap();
    std::fs::write(dir.join("shards/h_w3.bin"), &bytes[..bytes.len() - 1]).unwrap();
    let err = fold::fold(&mut conn, &dir.join("shards"), None, None)
        .err()
        .unwrap();
    assert!(err.contains("sha256"), "unexpected: {err}");
    std::fs::remove_dir_all(&dir).ok();
}

// --- R6: the unmerged-shard gate ---

/// A sidecar whose tag is absent from `shards` refuses (the message names
/// the tag); after the fold it passes; a bin without a sidecar is the
/// ignored orphan; a sidecar without a bin refuses as a durable-layer
/// defect.
#[test]
fn r6_unmerged_gate() {
    let dir = shard_dir("r6");
    let tree = tactic_tree();
    write_shard_pair(&dir.join("shards"), "h_g1", TACTIC_PATH, &tree);

    let mut conn = Connection::open(dir.join("gate.db")).unwrap();
    fold::fold(&mut conn, &dir.join("shards"), None, None).unwrap();
    let rows = proofdb::schema::read_shard_rows(&conn).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(check_unmerged_shards(&dir.join("shards"), &rows).is_ok());

    // A bin without a sidecar: ignored (the interrupted job's orphan).
    let mut bytes = Vec::new();
    tree.to_bin(&mut bytes).unwrap();
    std::fs::write(dir.join("shards/h_orphan.bin"), &bytes).unwrap();
    assert!(check_unmerged_shards(&dir.join("shards"), &rows).is_ok());
    std::fs::remove_file(dir.join("shards/h_orphan.bin")).unwrap();

    // A second sidecar without a row: refuse, naming the tag.
    write_shard_pair(&dir.join("shards"), "h_g2", TACTIC_PATH, &tree);
    let err = check_unmerged_shards(&dir.join("shards"), &rows)
        .err()
        .unwrap();
    assert!(
        err.contains("h_g2") && err.contains("unmerged"),
        "unexpected: {err}"
    );

    // A sidecar without a bin: durable-layer defect.
    std::fs::remove_file(dir.join("shards/h_g2.bin")).unwrap();
    let err = check_unmerged_shards(&dir.join("shards"), &rows)
        .err()
        .unwrap();
    assert!(
        err.contains("h_g2") && err.contains("sidecar without bin"),
        "unexpected: {err}"
    );

    // After folding h_g2 (bin restored), its sidecar stops being unmerged.
    write_shard_pair(&dir.join("shards"), "h_g2", TACTIC_PATH, &tree);
    fold::fold(&mut conn, &dir.join("shards"), None, None).unwrap();
    let rows2 = proofdb::schema::read_shard_rows(&conn).unwrap();
    assert_eq!(rows2.len(), 2);
    assert!(check_unmerged_shards(&dir.join("shards"), &rows2).is_ok());
    std::fs::remove_dir_all(&dir).ok();
}

// --- R7: censor discipline ---

/// The fold never touches the DB's censor rows (they survive byte-equal
/// through a fold that changes the tree), and `--import-censor` merges a
/// legacy work.json in place under the max rule, idempotently.
#[test]
fn r7_censor_rows_survive_the_fold_and_import_is_max_rule() {
    use proofdb::schema::insert_censor_row;

    let dir = shard_dir("r7");
    let tree = tactic_tree();
    write_shard_pair(&dir.join("shards"), "h_k1", TACTIC_PATH, &tree);
    let db = dir.join("shards").parent().unwrap().join("fold.db");
    let mut conn = Connection::open(&db).unwrap();
    fold::fold(&mut conn, &dir.join("shards"), None, None).unwrap();
    // (row order as `read_censor_rows` returns it: the root first)
    let seeded = vec![
        proofdb::schema::CensorRow {
            path: String::new(),
            work_done: 4,
            passes_failed: 1,
        },
        proofdb::schema::CensorRow {
            path: "a2a3".to_string(),
            work_done: 4_000_000,
            passes_failed: 2,
        },
    ];
    for row in &seeded {
        insert_censor_row(&conn, row).unwrap();
    }

    // A tree-changing fold (new shard at a fresh path) leaves the rows.
    write_shard_pair(
        &dir.join("shards"),
        "h_k2",
        TACTIC2_PATH,
        &tactic_tree_at(TACTIC2_FEN),
    );
    fold::fold(&mut conn, &dir.join("shards"), None, None).unwrap();
    assert_eq!(
        read_censor_rows(&conn).unwrap(),
        seeded,
        "censor rows untouched"
    );

    // The legacy import (the same primitives --import-censor drives): max
    // rule, new keys inserted, idempotent on a repeat.
    let legacy = dir.join("work.json");
    std::fs::write(
        &legacy,
        r#"{"records":[
            {"path":["a2a3"],"work_done":100,"passes_failed":1},
            {"path":["b1c3"],"work_done":9000000,"passes_failed":3}]}"#,
    )
    .unwrap();
    let out = fold::fold(&mut conn, &dir.join("shards"), None, Some(&legacy)).unwrap();
    assert_eq!(out.report.imported_censor, 2);
    let after = read_censor_rows(&conn).unwrap();
    assert!(after.contains(&proofdb::schema::CensorRow {
        path: "a2a3".to_string(),
        work_done: 4_000_000, // the stored (newer) value survived the max rule
        passes_failed: 2,
    }));
    assert!(after.contains(&proofdb::schema::CensorRow {
        path: "b1c3".to_string(),
        work_done: 9_000_000,
        passes_failed: 3,
    }));
    let out2 = fold::fold(&mut conn, &dir.join("shards"), None, Some(&legacy)).unwrap();
    assert_eq!(out2.report.imported_censor, 2, "idempotent re-import");
    assert_eq!(
        read_censor_rows(&conn).unwrap(),
        after,
        "import changed nothing"
    );
    std::fs::remove_dir_all(&dir).ok();
}

// --- R8: proofdb_verify ---

/// `verify_db` is green on a folded DB, and red on corrupted copies:
/// missing child, flipped outcome, illegal stored move, ply break,
/// `node_count` mismatch, empty provenance — each with at least one defect
/// line reported.
#[test]
fn r8_verify_green_and_red_on_corruptions() {
    let dir = shard_dir("r8");
    let tree = tactic_tree();
    write_shard_pair(&dir.join("shards"), "h_v1", TACTIC_PATH, &tree);
    let db_path = dir.join("fold.db");
    let mut conn = Connection::open(&db_path).unwrap();
    let out = fold::fold(&mut conn, &dir.join("shards"), None, None).unwrap();
    assert_eq!(out.report.node_count, 5); // root, f2f3, e7e6, g2g4, d8h4
    let verdict = verify_db(&conn, 10).unwrap();
    let Verdict::Ok {
        nodes,
        proven,
        open,
    } = &verdict
    else {
        panic!("verify must be ok: {verdict:?}");
    };
    assert_eq!((*nodes, *proven, *open), (5, 2, 3));

    // Each corruption on its own copy of the DB.
    let corruptions: &[(&str, &str)] = &[
        // The OR node g2g4 loses its only proven-loss child (the census
        // row is fixed up so the meta gate does not fire first).
        (
            "missing child",
            "DELETE FROM nodes WHERE move_uci='d8h4';
             UPDATE meta SET value='4' WHERE key='node_count';",
        ),
        // A flipped outcome: a non-terminal `loss` must have the full
        // legal-reply child set.
        (
            "flipped outcome",
            "UPDATE nodes SET outcome='loss' WHERE ply=3 AND move_uci='g2g4'",
        ),
        // An illegal stored move: not legal in the replayed position.
        (
            "illegal move",
            "UPDATE nodes SET move_uci='e7e5' WHERE move_uci='d8h4'",
        ),
        // A ply break: the child's ply no longer equals parent + 1.
        ("ply break", "UPDATE nodes SET ply=9 WHERE move_uci='d8h4'"),
        // The meta census disagrees with the table (the CLI's meta gate).
        (
            "node_count mismatch",
            "UPDATE meta SET value='999' WHERE key='node_count'",
        ),
        // Every row needs non-empty provenance.
        (
            "empty provenance",
            "UPDATE nodes SET provenance='' WHERE move_uci='d8h4'",
        ),
    ];
    for (name, sql) in corruptions {
        let bad = dir.join("bad.db");
        std::fs::copy(&db_path, &bad).unwrap();
        let c = Connection::open(&bad).unwrap();
        c.execute_batch(sql).unwrap();
        // The meta census is the CLI's gate; the tree checks are verify_db.
        let meta_err = proofdb::verify::check_meta(&c);
        let verdict = verify_db(&c, 10);
        if name.starts_with("node_count") {
            assert!(meta_err.is_err(), "corruption {name:?}: meta gate passed");
        } else {
            assert!(meta_err.is_ok(), "corruption {name:?}: {meta_err:?}");
            let verdict = verdict.unwrap();
            let Verdict::Failed {
                total, reported, ..
            } = &verdict
            else {
                panic!("corruption {name:?} must fail verify: {verdict:?}");
            };
            assert!(*total >= 1, "corruption {name:?}: no defects counted");
            assert!(
                !reported.is_empty() && reported.len() <= 10,
                "corruption {name:?}: defect lines {reported:?}"
            );
        }
        std::fs::remove_file(&bad).ok();
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// The session collision seeds derived from a shard set (plan22 D6): the
/// tags are the hash-of-path derivations and the standing paths are the
/// space-joined UCI paths — a second session cannot collide on either.
#[test]
fn shard_rows_seed_distinct_tag_and_path_sets() {
    let dir = shard_dir("seeds");
    let tree = tactic_tree();
    write_shard_pair(&dir.join("shards"), "h_s1", TACTIC_PATH, &tree);
    write_shard_pair(
        &dir.join("shards"),
        "h_s2",
        TACTIC2_PATH,
        &tactic_tree_at(TACTIC2_FEN),
    );
    let db = dir.join("shards").parent().unwrap().join("fold.db");
    let mut conn = Connection::open(&db).unwrap();
    fold::fold(&mut conn, &dir.join("shards"), None, None).unwrap();
    let rows = proofdb::schema::read_shard_rows(&conn).unwrap();
    let tags: HashSet<&str> = rows.iter().map(|r| r.tag.as_str()).collect();
    let paths: HashSet<&str> = rows.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(tags.len(), 2);
    assert_eq!(paths.len(), 2);
    assert!(paths.contains("f2f3 e7e6 g2g4"));
    assert!(paths.contains("f2f3 e7e5 g2g4"));
    std::fs::remove_dir_all(&dir).ok();
}
