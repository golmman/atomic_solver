//! Unit tests for the proof-tree worker.
//!
//! This file is larger than 10 KiB because it exercises both the public
//! `ProofTreeWorkerHandle` and the internal worker state machine, covering
//! out-of-order events, child replacement, memory limits, and search-driven
//! population.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::time::Duration;

use atomic_movegen::types::{Move, Square};

use super::{ProofResponse, ProofTreeWorker, ProofTreeWorkerHandle, ProofTreeWorkerMessage};
use crate::position::Outcome;
use crate::proof_event::{NodeProven, ProofEvent};

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
            Outcome::Win,
            0,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            Outcome::Loss,
            1,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![],
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
            Outcome::Win,
            5,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            Outcome::Loss,
            4,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::D2, Square::D4)],
            Outcome::Loss,
            2,
        )))
        .unwrap();
    // A deeper duplicate of the selected child must be ignored, not appended.
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::D2, Square::D4)],
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
            Outcome::Loss,
            5,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            Outcome::Win,
            4,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::D2, Square::D4)],
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
fn worker_updates_existing_child_with_shorter_depth() {
    let (handle, join) =
        ProofTreeWorkerHandle::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));

    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![],
            Outcome::Loss,
            5,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::E2, Square::E4)],
            Outcome::Win,
            4,
        )))
        .unwrap();
    handle
        .event_sender()
        .send(ProofEvent::NodeProven(NodeProven::new(
            vec![Move::make_move(Square::D2, Square::D4)],
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
            Outcome::Win,
            1,
        )))
        .unwrap();

    let tree2 = handle.tree();
    assert_eq!(tree2.nodes[0].children.len(), 2);
    let e2e4_id = tree2.index["root.e2e4"];
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
    worker.handle_query(ProofTreeWorkerMessage::GetStats(tx));
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
        Outcome::Win,
        0,
    )));
    worker.handle_event(ProofEvent::NodeProven(NodeProven::new(
        vec![Move::make_move(Square::E2, Square::E4)],
        Outcome::Loss,
        1,
    )));
    worker.handle_event(ProofEvent::NodeProven(NodeProven::new(
        vec![],
        Outcome::Win,
        2,
    )));

    let (tx, rx) = channel();
    worker.handle_query(ProofTreeWorkerMessage::GetStats(tx));
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
        Outcome::Win,
        2,
    )));
    worker.handle_event(ProofEvent::NodeProven(NodeProven::new(
        vec![Move::make_move(Square::E2, Square::E4)],
        Outcome::Loss,
        1,
    )));
    worker.handle_event(ProofEvent::Clear);

    let (tx, rx) = channel();
    worker.handle_query(ProofTreeWorkerMessage::GetStats(tx));
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
