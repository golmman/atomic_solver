use super::*;
use crate::position::Position;
use atomic_movegen::types::{Move, Square};

#[test]
fn add_node_builds_path() {
    let mut tree = ProofTree::new("fen".to_string(), 0, Some(Outcome::Win), 2);
    let child = tree.add_node(
        0,
        Move::make_move(Square::E2, Square::E4),
        0,
        Some(Outcome::Loss),
        1,
        0,
    );
    let grandchild = tree.add_node(
        child,
        Move::make_move(Square::E7, Square::E5),
        0,
        Some(Outcome::Win),
        0,
        0,
    );
    assert_eq!(tree.children(0).collect::<Vec<_>>(), vec![child]);
    assert_eq!(tree.children(child).collect::<Vec<_>>(), vec![grandchild]);
}

#[test]
fn to_bin_round_trips_small_tree() {
    let mut tree = ProofTree::new(Position::STARTPOS_FEN.to_string(), 0, Some(Outcome::Win), 2);
    tree.add_node(
        0,
        Move::make_move(Square::E2, Square::E4),
        0,
        Some(Outcome::Loss),
        1,
        7,
    );
    tree.add_node(
        1,
        Move::make_move(Square::E7, Square::E5),
        0,
        Some(Outcome::Win),
        0,
        3,
    );

    let mut buf = Vec::new();
    tree.to_bin(&mut buf).unwrap();
    let loaded = ProofTree::from_bin(&mut &buf[..]).unwrap();

    assert_eq!(loaded.nodes.len(), tree.nodes.len());
    assert_eq!(loaded.root_fen, tree.root_fen);
    for i in 0..loaded.nodes.len() {
        let a = &loaded.nodes[i];
        let b = &tree.nodes[i];
        assert_eq!(a.mv, b.mv);
        assert_eq!(a.outcome, b.outcome);
        assert_eq!(a.depth, b.depth);
        assert_eq!(a.work, b.work);
        assert_eq!(
            loaded.children(i).collect::<Vec<_>>(),
            tree.children(i).collect::<Vec<_>>()
        );
    }
}

#[test]
fn extract_ppv_returns_empty_for_drawn_root() {
    let tree = ProofTree::new("fen".to_string(), 0, Some(Outcome::Draw), 0);
    assert!(tree.extract_ppv().is_empty());
}

#[test]
fn validate_ppv_rejects_wrong_path() {
    let mut tree = ProofTree::new("fen".to_string(), 0, Some(Outcome::Win), 2);
    let child = tree.add_node(
        0,
        Move::make_move(Square::E2, Square::E4),
        0,
        Some(Outcome::Loss),
        1,
        0,
    );
    tree.add_node(
        child,
        Move::make_move(Square::E7, Square::E5),
        0,
        Some(Outcome::Win),
        0,
        0,
    );

    let wrong = vec![Move::make_move(Square::D2, Square::D4)];
    assert!(!tree.validate_ppv(&wrong));
}

#[test]
fn validate_ppv_rejects_premature_termination() {
    let mut tree = ProofTree::new("fen".to_string(), 0, Some(Outcome::Win), 2);
    let _ = tree.add_node(
        0,
        Move::make_move(Square::E2, Square::E4),
        0,
        Some(Outcome::Loss),
        1,
        0,
    );
    // The child node at depth 1 is not terminal, so a one-move PV is invalid.
    assert!(!tree.validate_ppv(&[Move::make_move(Square::E2, Square::E4)]));
}
