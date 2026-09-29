//! Shared fixture DB for the proofdb tool tests (merger-built, physical:
//! every row replays legally). Included only under `#[cfg(test)]` from
//! `mod.rs`; used by `frontier/tests.rs` and `policy/tests.rs`.

use crate::proofdb::merge::PathTree;
use crate::proofdb::schema::{ShardRow, write_db};
use atomic_movegen::types::Move;
use atomic_solver::notation::uci_to_move;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofNode, ProofTree, validate_proof_tree};

/// After 1.f3 e6 2.g4: Qd8-h4 explodes the White king (win in 1).
pub(crate) const TACTIC_FEN: &str =
    "rnbqkbnr/pppp1ppp/4p3/8/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2";

/// Build a fixture DB: root (open) + two grafts reaching the same win-in-1
/// subtree via two different legal move orders (`f2f3 e7e6 g2g4` and
/// `g2g4 e7e6 f2f3`), so every row replays and the path-tree dedupe is
/// exercised. Open: root, `f2f3`, `g2g4`, `f2f3 e7e6`, `g2g4 e7e6`; the
/// `… f2f3`/`… g2g4` (win, bound 1) and `d8h4` (loss terminal) chain nodes
/// are proven. Returns the DB path and its manifest digest.
pub(crate) fn fixture_db(dir: &std::path::Path) -> (std::path::PathBuf, String) {
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
        vec!["g2g4".into(), "e7e6".into(), "f2f3".into()],
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
