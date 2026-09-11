//! Unit tests for the replay-based proof-tree validator.

use atomic_movegen::types::{Move, Square};

use super::{DefectKind, TreeDefect, validate_proof_tree};
use crate::notation::{move_to_uci, uci_to_move};
use crate::position::{Outcome, Position};
use crate::proof_tree::{ProofTree, ProofTreeWorkerHandle};
use crate::search::dfpn::Search;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const MATE_FEN: &str = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";

/// Hand-built mate-in-1 tree: root Win plays Qb7#, the resulting position is
/// statically terminal (Loss for the side to move).
#[test]
fn valid_mate_in_1_tree_passes() {
    let root_fen = "k7/8/1Q6/8/8/8/8/7K w - - 0 1";
    let pos = Position::from_fen(root_fen).unwrap();
    let mate = uci_to_move("b6b7", &pos).expect("b6b7 must be legal");
    let mut after = pos.clone();
    after.do_move(mate);
    assert_eq!(after.outcome(), Some(Outcome::Loss), "b6b7 must be mate");

    let mut tree = ProofTree::new(root_fen.to_string(), pos.hash(), Some(Outcome::Win), 1);
    tree.add_node(0, mate, after.hash(), Some(Outcome::Loss), 0);

    assert_eq!(validate_proof_tree(&tree), Ok(()));
}

/// Solve the two-rook mate, finalize the live worker tree, and check the
/// validator accepts it (the proof contains a real non-terminal Loss node
/// covering all legal replies).
#[test]
fn solved_two_rook_proof_passes() {
    let tree = finalized_two_rook_tree();
    assert_eq!(
        validate_proof_tree(&tree),
        Ok(()),
        "the finalized two-rook proof must validate clean"
    );
}

/// A tree loaded from the binary dump carries `hash == 0` on every node;
/// the validator must skip hash checks in that case.
#[test]
fn loaded_from_bin_tree_passes_without_hash_defects() {
    let tree = finalized_two_rook_tree();
    let mut bytes = Vec::new();
    tree.to_bin(&mut bytes).expect("dump must serialize");
    let loaded = ProofTree::from_bin(&mut &bytes[..]).expect("dump must parse");
    assert!(loaded.nodes.iter().all(|n| n.hash == 0));
    assert_eq!(
        validate_proof_tree(&loaded),
        Ok(()),
        "loaded tree (hash == 0) must validate without hash defects"
    );
}

fn child_by_move(tree: &ProofTree, parent: usize, mv: Move) -> Option<usize> {
    tree.children(parent).find(|&c| tree.nodes[c].mv == mv)
}

/// Drop the sole reply of the root's Loss node: the validator must report the
/// missing move with the correct path.
#[test]
fn loss_incomplete_reports_missing_reply_and_path() {
    let mut tree = finalized_two_rook_tree();
    let loss_id = child_by_move(&tree, 0, Move::make_move(Square::F1, Square::F7))
        .expect("root must have the f1f7 Loss child");
    let loss_path = vec![Move::make_move(Square::F1, Square::F7)];
    tree.nodes[loss_id].first_child = None;

    let defects = validate_proof_tree(&tree).expect_err("dropped reply must be a defect");
    let incomplete: Vec<&TreeDefect> = defects
        .iter()
        .filter(|d| d.kind == DefectKind::LossIncomplete)
        .collect();
    assert!(
        !incomplete.is_empty(),
        "expected a LossIncomplete defect, got: {defects:?}"
    );
    assert!(incomplete.iter().all(|d| d.path == loss_path));
    assert!(
        incomplete.iter().any(|d| d
            .detail
            .contains(&move_to_uci(Move::make_move(Square::E8, Square::D8)))),
        "the missing reply e8d8 must be named, got: {incomplete:?}"
    );
}

/// A non-terminal Win node with no children is a `WinNotSingle` defect.
#[test]
fn win_not_single_detected() {
    let mut tree = finalized_two_rook_tree();
    tree.nodes[0].first_child = None;

    let defects = validate_proof_tree(&tree).expect_err("childless win root is a defect");
    assert!(
        defects
            .iter()
            .any(|d| d.kind == DefectKind::WinNotSingle && d.path.is_empty()),
        "expected WinNotSingle at the root, got: {defects:?}"
    );
}

/// A Win node whose single child is not Loss is a `ChildNotLoss` defect.
#[test]
fn child_not_loss_detected() {
    let mut tree = finalized_two_rook_tree();
    let loss_id = child_by_move(&tree, 0, Move::make_move(Square::F1, Square::F7))
        .expect("root must have the f1f7 Loss child");
    tree.nodes[loss_id].outcome = Some(Outcome::Win);

    let defects = validate_proof_tree(&tree).expect_err("win child under win root is a defect");
    assert!(
        defects.iter().any(|d| d.kind == DefectKind::ChildNotLoss),
        "expected ChildNotLoss, got: {defects:?}"
    );
}

/// A depth-0 node whose outcome contradicts the statically replayed position
/// is a `TerminalMismatch` defect.
#[test]
fn terminal_mismatch_detected() {
    let mut tree = finalized_two_rook_tree();
    let loss_id = child_by_move(&tree, 0, Move::make_move(Square::F1, Square::F7)).unwrap();
    let win_id = child_by_move(&tree, loss_id, Move::make_move(Square::E8, Square::D8)).unwrap();
    let leaf_id = child_by_move(&tree, win_id, Move::make_move(Square::G1, Square::G8)).unwrap();
    assert_eq!(tree.nodes[leaf_id].depth, 0);
    tree.nodes[leaf_id].outcome = Some(Outcome::Win);

    let defects = validate_proof_tree(&tree).expect_err("flipped leaf outcome is a defect");
    assert!(
        defects
            .iter()
            .any(|d| d.kind == DefectKind::TerminalMismatch
                && d.path
                    == vec![
                        Move::make_move(Square::F1, Square::F7),
                        Move::make_move(Square::E8, Square::D8),
                        Move::make_move(Square::G1, Square::G8),
                    ]),
        "expected TerminalMismatch at the mate leaf, got: {defects:?}"
    );
}

/// A stored depth that contradicts the bottom-up recomputation is a
/// `DepthInconsistent` defect.
#[test]
fn depth_inconsistent_detected() {
    let mut tree = finalized_two_rook_tree();
    tree.nodes[0].depth = 99;

    let defects = validate_proof_tree(&tree).expect_err("corrupted root depth is a defect");
    assert!(
        defects
            .iter()
            .any(|d| d.kind == DefectKind::DepthInconsistent && d.path.is_empty()),
        "expected DepthInconsistent at the root, got: {defects:?}"
    );
}

/// A non-zero node hash that disagrees with the replayed Zobrist key is a
/// `HashMismatch` defect.
#[test]
fn hash_mismatch_detected() {
    let mut tree = finalized_two_rook_tree();
    tree.nodes[0].hash = 42;

    let defects = validate_proof_tree(&tree).expect_err("corrupted root hash is a defect");
    assert!(
        defects
            .iter()
            .any(|d| d.kind == DefectKind::HashMismatch && d.path.is_empty()),
        "expected HashMismatch at the root, got: {defects:?}"
    );
}

/// Draw roots and mid-tree Draw nodes are `NotDecisive` defects (the search
/// never emits draw events, so a finalized decisive proof has none).
#[test]
fn draw_nodes_are_not_decisive() {
    let mut tree = finalized_two_rook_tree();
    tree.nodes[0].outcome = Some(Outcome::Draw);
    let defects = validate_proof_tree(&tree).expect_err("draw root is a defect");
    assert!(
        defects
            .iter()
            .any(|d| d.kind == DefectKind::NotDecisive && d.path.is_empty()),
        "expected NotDecisive at the root, got: {defects:?}"
    );

    let mut tree = finalized_two_rook_tree();
    let loss_id = child_by_move(&tree, 0, Move::make_move(Square::F1, Square::F7)).unwrap();
    tree.nodes[loss_id].outcome = Some(Outcome::Draw);
    let defects = validate_proof_tree(&tree).expect_err("mid-tree draw node is a defect");
    assert!(
        defects.iter().any(|d| d.kind == DefectKind::NotDecisive
            && d.path == vec![Move::make_move(Square::F1, Square::F7)]),
        "expected NotDecisive at root.f1f7, got: {defects:?}"
    );
}

/// An unparseable root FEN yields a single `NotDecisive` defect (the root
/// position cannot be established).
#[test]
fn unparseable_root_fen_is_not_decisive() {
    let tree = ProofTree::new("fen".to_string(), 0, Some(Outcome::Win), 0);
    let defects = validate_proof_tree(&tree).expect_err("bad root FEN is a defect");
    assert_eq!(defects.len(), 1);
    assert_eq!(defects[0].kind, DefectKind::NotDecisive);
    assert!(defects[0].path.is_empty());
}

/// Build the finalized live proof tree of the two-rook mate fixture.
fn finalized_two_rook_tree() -> ProofTree {
    let memory_limited = Arc::new(AtomicBool::new(false));
    let (handle, join) =
        ProofTreeWorkerHandle::spawn(MATE_FEN.to_string(), 256, Arc::clone(&memory_limited));
    let mut pos = Position::from_fen(MATE_FEN).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(60);
    search.set_first_outcome_only(true);
    search.set_memory_limited(Some(Arc::clone(&memory_limited)));
    search.set_proof_event_sender(Some(handle.event_sender()));
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win, "the two-rook fixture must solve");
    handle.finalize();
    let tree = handle.tree();
    drop(search);
    drop(handle);
    join.join().unwrap();
    tree
}
