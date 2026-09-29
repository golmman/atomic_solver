//! Harvest engine unit tests (plan2): frontier extraction/deepest-first
//! order, stale-DB digest abort, censored classification, tag determinism,
//! manifest-entry shape, and the replay-prefix contract. Included by
//! `harvest.rs`; also runs via `tests/proofdb.rs`.

use super::*;
use crate::proofdb::merge::PathTree;
use crate::proofdb::schema::{ShardRow, write_db};
use atomic_movegen::types::Move;
use atomic_solver::proof_tree::{ProofNode, ProofTree, validate_proof_tree};

/// After 1.f3 e6 2.g4: Qd8-h4 explodes the White king (win in 1).
const TACTIC_FEN: &str = "rnbqkbnr/pppp1ppp/4p3/8/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2";

/// Build a fixture DB: root (open) + two grafts sharing the same
/// win-in-1 subtree at `f2f3 e7e6 g2g4` and `e2e4 f7f6 g2g4`. Open:
/// root, `f2f3`, `e7e6`, `e2e4`, `f7f6` (two ply-1/ply-2 tie pairs); the
/// four `g2g4`/`d8h4` chain nodes are proven. Returns the DB path and
/// its manifest digest.
fn fixture_db(dir: &std::path::Path) -> (std::path::PathBuf, String) {
    let mut pos = Position::from_fen(TACTIC_FEN).unwrap();
    let mv = uci_to_move("d8h4", &pos).unwrap();
    pos.do_move(mv);
    let child = ProofNode {
        parent: Some(std::num::NonZeroU32::new(1).unwrap()),
        first_child: None,
        next_sibling: None,
        mv,
        hash: 0,
        outcome: Some(Outcome::Loss),
        depth: 0,
    };
    let root = ProofNode {
        parent: None,
        first_child: Some(std::num::NonZeroU32::new(1).unwrap()),
        next_sibling: None,
        mv: Move::NONE,
        hash: 0,
        outcome: Some(Outcome::Win),
        depth: 1,
    };
    let tree = ProofTree {
        root_fen: TACTIC_FEN.to_string(),
        nodes: vec![root, child],
    };
    assert!(validate_proof_tree(&tree).is_ok());

    let mut pt = PathTree::new();
    let grafts: Vec<Vec<String>> = vec![
        vec!["f2f3".into(), "e7e6".into(), "g2g4".into()],
        vec!["e2e4".into(), "f7f6".into(), "g2g4".into()],
    ];
    let mut rows = Vec::new();
    for (k, moves) in grafts.iter().enumerate() {
        let tag = format!("fixture{k}");
        let mut rp = Position::from_fen(Position::STARTPOS_FEN).unwrap();
        let mvs: Vec<_> = moves
            .iter()
            .map(|u| {
                let m = uci_to_move(u, &rp).unwrap();
                rp.do_move(m);
                m
            })
            .collect();
        let graft = pt.graft_path(&mvs, &tag);
        pt.overlay_subtree(graft, &tree, &tag).unwrap();
        rows.push(ShardRow {
            file: format!("{tag}.bin"),
            tag,
            root_fen: TACTIC_FEN.into(),
            path: moves.join(" "),
            outcome: Outcome::Win,
            depth_bound: 1,
            n_nodes: 2,
            sha256: "0".repeat(64),
        });
    }
    pt.finalize().unwrap();

    let manifest_bytes = b"{\"entries\":[]}".to_vec();
    let digest = crate::proofdb::digest_hex(&manifest_bytes);
    std::fs::write(dir.join("manifest.json"), &manifest_bytes).unwrap();
    let db = dir.join("fixture.db");
    write_db(&db, Position::STARTPOS_FEN, &digest, &rows, &pt).unwrap();
    (db, digest)
}

#[test]
fn frontier_is_open_nodes_deepest_first() {
    let dir = std::env::temp_dir().join(format!("proofdb_hv_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (db, digest) = fixture_db(&dir);
    let input = extract_jobs(&db, &digest).unwrap();
    assert_eq!(input.root_fen, Position::STARTPOS_FEN);
    assert_eq!(input.n_nodes, 9);
    assert_eq!(input.jobs.len(), 5, "root + 4 graft ancestors are open");
    // Deepest-first: both ply-2 nodes before both ply-1 nodes (ties
    // lexicographic ascending), root last.
    let paths: Vec<Vec<String>> = input.jobs.iter().map(|j| j.path.clone()).collect();
    assert_eq!(paths[0], vec!["e2e4".to_string(), "f7f6".to_string()]);
    assert_eq!(paths[1], vec!["f2f3".to_string(), "e7e6".to_string()]);
    assert_eq!(paths[2], vec!["e2e4".to_string()]);
    assert_eq!(paths[3], vec!["f2f3".to_string()]);
    assert!(paths[4].is_empty(), "the root is the last job");
    // The proven paths are recorded for the disjointness assert.
    assert!(input.proven_paths.contains("f2f3 e7e6 g2g4"));
    assert!(input.proven_paths.contains("e2e4 f7f6 g2g4"));
    assert_eq!(input.proven_paths.len(), 4, "the two d8h4 terminals too");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn stale_db_digest_aborts() {
    let dir = std::env::temp_dir().join(format!("proofdb_hv2_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (db, digest) = fixture_db(&dir);
    let stale = format!("{:0>64}", "0");
    assert!(stale != digest);
    let err = extract_jobs(&db, &stale).unwrap_err();
    assert!(err.contains("built_from"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn draw_is_censored_and_decisive_classifies() {
    assert_eq!(classify(Outcome::Draw), None);
    assert_eq!(classify(Outcome::Win), Some(Outcome::Win));
    assert_eq!(classify(Outcome::Loss), Some(Outcome::Loss));
}

#[test]
fn tag_is_deterministic_hex() {
    let a = vec!["e2e4".to_string(), "e7e5".to_string()];
    let b = vec!["e2e4".to_string(), "e7e5".to_string()];
    let t = tag_for_path(&a);
    assert_eq!(t, tag_for_path(&b), "deterministic");
    assert_eq!(t.len(), 18, "h_ + 16 hex chars");
    assert!(t.starts_with("h_"));
    assert!(t[2..].chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(tag_for_path(&[]), tag_for_path(&a));
}

#[test]
fn entry_carries_path_fen_outcome() {
    let job = Job {
        path: vec!["e2e4".into()],
        ply: 1,
    };
    let e = make_entry(&job, "FEN", Outcome::Loss);
    assert_eq!(e.tag, tag_for_path(&job.path));
    assert_eq!(e.file, format!("{}.bin", e.tag));
    assert_eq!(e.moves, job.path);
    assert_eq!(e.fen, "FEN");
    assert_eq!(e.outcome, Outcome::Loss);
    assert_eq!(e.validate, "ok");
}

#[test]
fn replay_prefix_excludes_the_node_itself() {
    // Empty path (root job): no prefix at all — the startpos must not sit
    // twice on the search's own path (false repetition, see fn docs).
    let (pos, prefix) = replay_job_path(&[]).unwrap();
    assert!(prefix.is_empty());
    assert_eq!(pos.fen(), Position::STARTPOS_FEN);

    let (pos, prefix) = replay_job_path(&["e2e4".into(), "e7e5".into()]).unwrap();
    assert_eq!(prefix.len(), 2, "startpos key + key after e2e4");
    assert_ne!(prefix[0], prefix[1]);
    assert_eq!(
        pos.fen(),
        "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2"
    );
    // The node's own key is not in its prefix (the search pushes it).
    assert!(!prefix.contains(&pos.repetition_key()));
}

#[test]
fn replay_rejects_illegal_db_move() {
    assert!(replay_job_path(&["e2e5".into()]).is_err());
}
