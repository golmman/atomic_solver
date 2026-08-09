//! Unit tests for the proof-tree worker.
//!
//! This file is larger than 10 KiB because it exercises both the public
//! `ProofTreeWorkerHandle` and the internal worker state machine, covering
//! out-of-order events, child replacement, dummy pruning, memory limits, and
//! search-driven population.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::time::Duration;

use atomic_movegen::types::{Move, Square};

use super::{ProofResponse, ProofTreeWorker, ProofTreeWorkerHandle, ProofTreeWorkerMessage};
use crate::position::Outcome;
use crate::proof_event::{NodeProven, ProofEvent};
use crate::proof_tree::ProofTree;

fn child_by_move(tree: &ProofTree, parent: usize, mv: Move) -> Option<usize> {
    tree.nodes[parent]
        .children
        .iter()
        .copied()
        .find(|&c| tree.nodes[c].mv == mv)
}

#[test]
fn worker_handles_out_of_order_events() {
    let (handle, join) =
        ProofTreeWorkerHandle::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));

    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![
                Move::make_move(Square::E2, Square::E4),
                Move::make_move(Square::E7, Square::E5),
            ],
            0,
            Outcome::Win,
            0,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            0,
            Outcome::Loss,
            1,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![],
            0,
            Outcome::Win,
            2,
        )))
        .unwrap();

    let stats = handle.stats();
    assert_eq!(stats.nodes, 3);
    assert_eq!(stats.win_nodes, 2);
    assert_eq!(stats.loss_nodes, 1);
    assert_eq!(stats.root_depth, 2);

    let tree = handle.tree();
    assert_eq!(tree.nodes.len(), 3);
    assert_eq!(tree.nodes[0].children, vec![1]);
    assert_eq!(tree.nodes[1].children, vec![2]);

    drop(handle);
    join.join().unwrap();
}

#[test]
fn worker_replaces_win_child_with_shortest_loss() {
    let (handle, join) =
        ProofTreeWorkerHandle::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));

    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![],
            0,
            Outcome::Win,
            5,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            0,
            Outcome::Loss,
            4,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::D2, Square::D4)],
            0,
            Outcome::Loss,
            2,
        )))
        .unwrap();
    // A deeper duplicate of the selected child must be ignored, not appended.
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::D2, Square::D4)],
            0,
            Outcome::Loss,
            6,
        )))
        .unwrap();

    let tree = handle.tree();
    assert_eq!(tree.nodes[0].children.len(), 1);
    assert_eq!(
        tree.nodes[tree.nodes[0].children[0]].mv,
        Move::make_move(Square::D2, Square::D4)
    );
    assert_eq!(tree.nodes[tree.nodes[0].children[0]].depth, 2);

    drop(handle);
    join.join().unwrap();
}

#[test]
fn worker_loss_parent_keeps_all_distinct_win_children() {
    let (handle, join) =
        ProofTreeWorkerHandle::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));

    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![],
            0,
            Outcome::Loss,
            5,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            0,
            Outcome::Win,
            4,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::D2, Square::D4)],
            0,
            Outcome::Win,
            2,
        )))
        .unwrap();

    let tree = handle.tree();
    assert_eq!(tree.nodes[0].children.len(), 2);

    drop(handle);
    join.join().unwrap();
}

#[test]
fn worker_loss_parent_removes_loss_children() {
    let (handle, join) =
        ProofTreeWorkerHandle::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));

    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![],
            0,
            Outcome::Loss,
            3,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            0,
            Outcome::Loss,
            2,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::D2, Square::D4)],
            0,
            Outcome::Win,
            1,
        )))
        .unwrap();

    let tree = handle.tree();
    assert_eq!(tree.nodes[0].children.len(), 1);
    assert_eq!(
        tree.nodes[tree.nodes[0].children[0]].mv,
        Move::make_move(Square::D2, Square::D4)
    );
    assert_eq!(
        tree.nodes[tree.nodes[0].children[0]].outcome,
        Some(Outcome::Win)
    );

    drop(handle);
    join.join().unwrap();
}

#[test]
fn worker_updates_existing_child_with_shorter_depth() {
    let (handle, join) =
        ProofTreeWorkerHandle::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));

    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![],
            0,
            Outcome::Loss,
            5,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            0,
            Outcome::Win,
            4,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::D2, Square::D4)],
            0,
            Outcome::Win,
            2,
        )))
        .unwrap();

    let tree = handle.tree();
    assert_eq!(tree.nodes[0].children.len(), 2);

    // A duplicate with a shorter depth updates the existing child.
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            0,
            Outcome::Win,
            1,
        )))
        .unwrap();

    let tree2 = handle.tree();
    assert_eq!(tree2.nodes[0].children.len(), 2);
    let e2e4 = Move::make_move(Square::E2, Square::E4);
    let e2e4_id = child_by_move(&tree2, 0, e2e4).expect("e2e4 child exists");
    assert_eq!(tree2.nodes[e2e4_id].depth, 1);

    drop(handle);
    join.join().unwrap();
}

#[test]
fn worker_sets_memory_limited_flag() {
    let flag = Arc::new(AtomicBool::new(false));
    let (handle, join) = ProofTreeWorkerHandle::spawn("fen".to_string(), 0, Arc::clone(&flag));
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![],
            0,
            Outcome::Win,
            0,
        )))
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        flag.load(std::sync::atomic::Ordering::Acquire),
        "memory flag should be set for zero budget"
    );
    drop(handle);
    join.join().unwrap();
}

#[test]
fn worker_new_does_not_spawn_thread() {
    let mut worker = ProofTreeWorker::new(
        "fen".to_string(),
        usize::MAX,
        Arc::new(AtomicBool::new(false)),
    );

    let (tx, rx) = channel();
    worker.handle_query(ProofTreeWorkerMessage::GetStats(tx), None);
    let stats = match rx.recv().unwrap() {
        ProofResponse::Stats(s) => s,
        _ => panic!("expected Stats response"),
    };
    assert_eq!(stats.nodes, 1);
}

#[test]
fn worker_handles_out_of_order_events_directly() {
    let mut worker = ProofTreeWorker::new(
        "fen".to_string(),
        usize::MAX,
        Arc::new(AtomicBool::new(false)),
    );

    worker.handle_event(ProofEvent::NodeProven(NodeProven::new(
        vec![
            Move::make_move(Square::E2, Square::E4),
            Move::make_move(Square::E7, Square::E5),
        ],
        0,
        Outcome::Win,
        0,
    )));
    worker.handle_event(ProofEvent::NodeProven(NodeProven::new(
        vec![Move::make_move(Square::E2, Square::E4)],
        0,
        Outcome::Loss,
        1,
    )));
    worker.handle_event(ProofEvent::NodeProven(NodeProven::new(
        vec![],
        0,
        Outcome::Win,
        2,
    )));

    let (tx, rx) = channel();
    worker.handle_query(ProofTreeWorkerMessage::GetStats(tx), None);
    let stats = match rx.recv().unwrap() {
        ProofResponse::Stats(s) => s,
        _ => panic!("expected Stats response"),
    };
    assert_eq!(stats.nodes, 3);
    assert_eq!(stats.win_nodes, 2);
    assert_eq!(stats.loss_nodes, 1);
    assert_eq!(stats.root_depth, 2);
}

#[test]
fn worker_clears_tree_directly() {
    let mut worker = ProofTreeWorker::new(
        "fen".to_string(),
        usize::MAX,
        Arc::new(AtomicBool::new(false)),
    );

    worker.handle_event(ProofEvent::NodeProven(NodeProven::new(
        vec![],
        0,
        Outcome::Win,
        2,
    )));
    worker.handle_event(ProofEvent::NodeProven(NodeProven::new(
        vec![Move::make_move(Square::E2, Square::E4)],
        0,
        Outcome::Loss,
        1,
    )));
    worker.handle_event(ProofEvent::Clear);

    let (tx, rx) = channel();
    worker.handle_query(ProofTreeWorkerMessage::GetStats(tx), None);
    let stats = match rx.recv().unwrap() {
        ProofResponse::Stats(s) => s,
        _ => panic!("expected Stats response"),
    };
    assert_eq!(stats.nodes, 1);
    assert_eq!(stats.win_nodes, 0);
    assert_eq!(stats.loss_nodes, 0);
}

#[test]
fn memory_limited_flag_triggers_at_small_budget_directly() {
    let flag = Arc::new(AtomicBool::new(false));
    let mut worker = ProofTreeWorker::new("fen".to_string(), 0, Arc::clone(&flag));

    worker.handle_event(ProofEvent::NodeProven(NodeProven::new(
        vec![],
        0,
        Outcome::Win,
        0,
    )));

    assert!(
        flag.load(std::sync::atomic::Ordering::Acquire),
        "zero budget should set memory flag"
    );
}

#[test]
fn solve_populates_proof_tree_with_nodes() {
    use crate::position::Position;
    use crate::search::dfpn::Search;

    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let (handle, join) =
        ProofTreeWorkerHandle::spawn(pos.fen(), 64, Arc::new(AtomicBool::new(false)));
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_proof_event_sender(Some(handle.event_sender()));
    let (outcome, pv, _nodes) = search.solve(&mut pos);

    handle.finalize();
    let stats = handle.stats();
    let tree = handle.tree();
    assert_eq!(outcome, Outcome::Win);
    assert!(
        stats.nodes > 0,
        "proof tree should contain at least the root node and proven children"
    );
    assert!(tree.validate_ppv(&pv), "proof tree should validate the PV");

    drop(search);
    drop(handle);
    join.join().unwrap();
}

#[test]
fn finalize_copies_expanded_twin_to_unexpanded_sibling() {
    let (handle, join) =
        ProofTreeWorkerHandle::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));

    // Loss root with two Win children that are the same position (hash 10).
    // Only the first is expanded; the second should inherit its subtree.
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![],
            0,
            Outcome::Loss,
            1,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            10,
            Outcome::Win,
            1,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![
                Move::make_move(Square::E2, Square::E4),
                Move::make_move(Square::E7, Square::E5),
            ],
            20,
            Outcome::Loss,
            0,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::D2, Square::D4)],
            10,
            Outcome::Win,
            1,
        )))
        .unwrap();

    handle.finalize();
    let tree = handle.tree();
    assert_eq!(tree.nodes[0].children.len(), 2);
    for &c in &tree.nodes[0].children {
        assert_eq!(tree.nodes[c].children.len(), 1);
        let leaf = tree.nodes[c].children[0];
        assert_eq!(tree.nodes[leaf].outcome, Some(Outcome::Loss));
        assert_eq!(tree.nodes[leaf].depth, 0);
    }

    drop(handle);
    join.join().unwrap();
}

#[test]
fn finalize_prefers_shorter_consistent_twin() {
    let (handle, join) =
        ProofTreeWorkerHandle::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));

    // Loss root with two Win twins. One has a stale stored depth of 4 with a
    // terminal child; the other is consistent with depth 1 and a terminal child.
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![],
            0,
            Outcome::Loss,
            5,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            10,
            Outcome::Win,
            4,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![
                Move::make_move(Square::E2, Square::E4),
                Move::make_move(Square::E7, Square::E5),
            ],
            20,
            Outcome::Loss,
            0,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::D2, Square::D4)],
            10,
            Outcome::Win,
            1,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![
                Move::make_move(Square::D2, Square::D4),
                Move::make_move(Square::E7, Square::E5),
            ],
            20,
            Outcome::Loss,
            0,
        )))
        .unwrap();

    handle.finalize();
    let tree = handle.tree();
    assert_eq!(tree.nodes[0].children.len(), 2);
    for &c in &tree.nodes[0].children {
        assert_eq!(tree.nodes[c].children.len(), 1);
        assert_eq!(tree.nodes[c].depth, 1);
    }
    // The stale depth is replaced by the consistent, shorter depth.
    assert_eq!(tree.nodes[0].depth, 2);

    drop(handle);
    join.join().unwrap();
}

#[test]
fn finalize_prunes_dummy_subtree() {
    let (handle, join) =
        ProofTreeWorkerHandle::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));

    let e2e4 = Move::make_move(Square::E2, Square::E4);
    let e7e5 = Move::make_move(Square::E7, Square::E5);
    let d2d4 = Move::make_move(Square::D2, Square::D4);
    let d7d5 = Move::make_move(Square::D7, Square::D5);

    // Root is Win via e2e4. A second branch starting with d2d4 is created as
    // a dummy parent for a deeper event, but d2d4 itself is never realized.
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![],
            100,
            Outcome::Win,
            2,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![e2e4],
            200,
            Outcome::Loss,
            1,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![e2e4, e7e5],
            300,
            Outcome::Win,
            0,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![d2d4, d7d5],
            400,
            Outcome::Win,
            0,
        )))
        .unwrap();

    handle.finalize();
    let tree = handle.tree();
    assert_eq!(tree.nodes.len(), 3);
    assert_eq!(tree.nodes[0].children.len(), 1);
    let e2e4_id = child_by_move(&tree, 0, e2e4).expect("e2e4 child");
    assert_eq!(tree.nodes[e2e4_id].children.len(), 1);
    assert!(child_by_move(&tree, 0, d2d4).is_none());
    assert!(!tree.nodes.iter().any(|n| n.mv == d7d5));

    drop(handle);
    join.join().unwrap();
}
