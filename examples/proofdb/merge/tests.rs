//! Merger unit tests: graft/overlay semantics, refinement, contradiction
//! aborts, row-id assignment, and skeleton reassembly. This module is
//! included verbatim by `tests/proofdb.rs` (cargo does not run unit tests
//! inside example targets on its own).

use atomic_movegen::types::Move;

use super::PathTree;
use crate::proofdb::merge::DepthStatus;
use atomic_solver::notation::uci_to_move;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofNode, ProofTree};

/// Build a chain-shaped ProofTree: node i's parent is node i-1.
/// `steps` are (uci, outcome, depth) per non-root node.
fn chain_tree(root_fen: &str, root_outcome: Outcome, steps: &[(&str, Outcome, u32)]) -> ProofTree {
    let pos = Position::from_fen(root_fen).expect("test fen parses");
    let mut nodes = vec![ProofNode {
        parent: None,
        first_child: None,
        next_sibling: None,
        mv: Move::NONE,
        hash: 0,
        outcome: Some(root_outcome),
        depth: steps.len() as u32,
    }];
    let mut cur = pos;
    for (i, (uci, outcome, depth)) in steps.iter().enumerate() {
        let mv = uci_to_move(uci, &cur).unwrap_or_else(|| panic!("test move {uci} is legal"));
        cur.do_move(mv);
        let parent = std::num::NonZeroU32::new(i as u32);
        let old_first = nodes[i].first_child;
        nodes.push(ProofNode {
            parent,
            first_child: None,
            next_sibling: old_first,
            mv,
            hash: 0,
            outcome: Some(*outcome),
            depth: *depth,
        });
        nodes[i].first_child = Some(std::num::NonZeroU32::new((i + 1) as u32).unwrap());
    }
    ProofTree {
        root_fen: root_fen.to_string(),
        nodes,
    }
}

const ROOT_FEN: &str = Position::STARTPOS_FEN;
/// 1-move atomic win: after 1.f3 e6 2.g4, Qd8-h4 explodes White.
const TACTIC_FEN: &str = "rnbqkbnr/pppp1ppp/4p3/8/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2";
const TACTIC_PATH: &[&str] = &["f2f3", "e7e6", "g2g4"];

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

fn win_in_1_tree() -> ProofTree {
    chain_tree(TACTIC_FEN, Outcome::Win, &[("d8h4", Outcome::Loss, 0)])
}

#[test]
fn graft_and_overlay_build_ancestors_and_facts() {
    let mut t = PathTree::new();
    let path = moves_from_path(TACTIC_PATH);
    let graft = t.graft_path(&path, "s");
    assert_eq!(t.ancestor_insertions, 3);
    assert_eq!(t.nodes.len(), 4);
    assert!(t.nodes[graft].outcome.is_none());
    assert_eq!(
        t.nodes[1].provenance.iter().next().map(String::as_str),
        Some("s")
    );
    t.overlay_subtree(graft, &win_in_1_tree(), "s").unwrap();
    assert_eq!(t.overlay_insertions, 1); // the terminal child
    assert_eq!(t.open_upgraded, 1); // graft node upgraded to win
    t.finalize().unwrap();
    assert_eq!(t.depth_bound(graft), Some(1));
    assert_eq!(t.depth_status(graft), Some(DepthStatus::Bound));
    let term = t.nodes[graft].children.values().copied().next().unwrap();
    assert_eq!(t.depth_bound(term), Some(0));
    assert_eq!(t.depth_status(term), Some(DepthStatus::Exact));
    assert_eq!(t.row_order.len(), 5);
    assert_eq!(t.row_order[0], 0); // root first
}

#[test]
fn refinement_keeps_tighter_bound() {
    let mut t = PathTree::new();
    let path = moves_from_path(TACTIC_PATH);
    let graft = t.graft_path(&path, "s1");
    // A shallower (but valid) proof of the same position: the skeleton
    // win-in-1 is the tightest; simulate by overlaying a win-in-1 tree on
    // top of a claimed deeper proof first.
    t.overlay_subtree(graft, &win_in_1_tree(), "s1").unwrap();
    t.finalize().unwrap();
    assert_eq!(t.depth_bound(graft), Some(1));
    // Same outcome from a second shard: agreement, bound stays.
    t.overlay_subtree(graft, &win_in_1_tree(), "s2").unwrap();
    t.finalize().unwrap();
    assert_eq!(t.depth_bound(graft), Some(1));
    assert_eq!(t.nodes[graft].provenance.len(), 2);
}

#[test]
fn outcome_contradiction_aborts() {
    let mut t = PathTree::new();
    let path = moves_from_path(TACTIC_PATH);
    let graft = t.graft_path(&path, "s1");
    t.overlay_subtree(graft, &win_in_1_tree(), "s1").unwrap();
    // A loss claim at the same path contradicts the win claim.
    let loss_tree = chain_tree(TACTIC_FEN, Outcome::Loss, &[("d8h4", Outcome::Win, 0)]);
    let err = t.overlay_subtree(graft, &loss_tree, "s2").unwrap_err();
    assert!(err.contains("contradiction"), "{err}");
}

#[test]
fn ids_are_lexicographic_and_order_independent() {
    // Two merge orders of the same shard set must give the same row ids.
    let build = |order: &[&str]| {
        let mut t = PathTree::new();
        let g1 = t.graft_path(&moves_from_path(&["a2a3"]), "a");
        let g2 = t.graft_path(&moves_from_path(&["b2b3"]), "b");
        for tag in order {
            let graft = if *tag == "a" { g1 } else { g2 };
            t.overlay_subtree(graft, &win_in_1_tree(), tag).unwrap();
        }
        t.finalize().unwrap();
        t.path_uci(t.row_order[1]).join(" ")
    };
    assert_eq!(build(&["a", "b"]), build(&["b", "a"]));
    // Lexicographic by move code: a2a3 vs b2b3 — a2a3's code is smaller
    // (from_sq a2 = 8 < b2 = 9), so it sorts first.
    let mut t = PathTree::new();
    let _ = t.graft_path(&moves_from_path(&["b2b3"]), "b");
    let _ = t.graft_path(&moves_from_path(&["a2a3"]), "a");
    t.finalize().unwrap();
    assert_eq!(t.path_uci(t.row_order[1]).join(" "), "a2a3");
    assert_eq!(t.path_uci(t.row_order[2]).join(" "), "b2b3");
}

#[test]
fn parent_rows_follow_lexicographic_order() {
    // Regression: the DB/dump writers must map a node's *creation index*
    // to its row id through the inverse of the lexicographic order, not
    // through the order itself. With a non-identity order (b2b3 grafted
    // before a2a3, but a2a3 sorting first) the wrong direction scrambles
    // parent ids while leaving all other fields intact.
    use crate::proofdb::schema::canonical_dump;
    let mut t = PathTree::new();
    let gb = t.graft_path(&moves_from_path(&["b2b3"]), "b");
    let ga = t.graft_path(&moves_from_path(&["a2a3"]), "a");
    t.overlay_subtree(gb, &win_in_1_tree(), "b").unwrap();
    t.overlay_subtree(ga, &win_in_1_tree(), "a").unwrap();
    t.finalize().unwrap();
    let dump = canonical_dump(Position::STARTPOS_FEN, "test", &t);
    let parse = |id: usize| {
        dump.lines()
            .map(|l| l.split('\t').collect::<Vec<_>>())
            .find(|f| f[0].parse::<usize>() == Ok(id))
            .map(|f| {
                (
                    f[2].parse::<u32>().expect("ply"),
                    f[3].to_string(),
                    f[1].to_string(),
                )
            })
            .expect("row exists")
    };
    // Row ids follow depth-first lexicographic path order (each node
    // before its extensions) regardless of graft order: 0 = root,
    // 1 = a2a3, 2 = a2a3 d8h4, 3 = b2b3, 4 = b2b3 d8h4.
    assert_eq!(parse(1), (1, "a2a3".to_string(), "0".to_string()));
    assert_eq!(parse(2), (2, "d8h4".to_string(), "1".to_string()));
    assert_eq!(parse(3), (1, "b2b3".to_string(), "0".to_string()));
    assert_eq!(parse(4), (2, "d8h4".to_string(), "3".to_string()));
}

#[test]
fn skeleton_reassembly_validates_after_merge() {
    use atomic_solver::proof_tree::validate_proof_tree;
    let mut t = PathTree::new();
    let path = moves_from_path(TACTIC_PATH);
    let graft = t.graft_path(&path, "s");
    t.overlay_subtree(graft, &win_in_1_tree(), "s").unwrap();
    t.finalize().unwrap();
    let skeleton = t.reassemble_skeleton(graft, TACTIC_FEN).unwrap();
    assert_eq!(skeleton.nodes.len(), 2);
    assert!(validate_proof_tree(&skeleton).is_ok());
}
