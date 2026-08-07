//! Unit tests for the proof-tree worker.
//!
//! This file is larger than 10 KiB because it exercises the worker's direct
//! message handling, thread spawning, out-of-order event processing, and
//! integration with the search loop in one place.

use atomic_movegen::types::{Move, Square};

use super::*;
use crate::position::Outcome;
use crate::proof_tree::NodeProven;

#[test]
fn worker_handles_out_of_order_events() {
    let (tx, handle) =
        ProofTreeWorker::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root.e2e4.e7e5".to_string(),
        mv: Move::make_move(Square::E7, Square::E5),
        outcome: Outcome::Win,
        depth: 0,
    }))
    .unwrap();
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root.e2e4".to_string(),
        mv: Move::make_move(Square::E2, Square::E4),
        outcome: Outcome::Loss,
        depth: 1,
    }))
    .unwrap();
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root".to_string(),
        mv: Move::NONE,
        outcome: Outcome::Win,
        depth: 2,
    }))
    .unwrap();

    let (reply_tx, reply_rx) = channel();
    tx.send(ProofMessage::GetStats(reply_tx)).unwrap();
    let ProofResponse::Stats(stats) = reply_rx.recv().unwrap() else {
        panic!("expected Stats response");
    };
    assert_eq!(stats.nodes, 3);
    assert_eq!(stats.win_nodes, 2);
    assert_eq!(stats.loss_nodes, 1);
    assert_eq!(stats.root_depth, 2);

    let (tree_tx, tree_rx) = channel();
    tx.send(ProofMessage::GetTree(tree_tx)).unwrap();
    let ProofResponse::Tree(tree) = tree_rx.recv().unwrap() else {
        panic!("expected Tree response");
    };
    assert_eq!(tree.nodes.len(), 3);
    assert_eq!(tree.nodes[0].children, vec![1]);
    assert_eq!(tree.nodes[1].children, vec![2]);

    drop(tx);
    handle.join().unwrap();
}

#[test]
fn worker_replaces_win_child_with_shortest_loss() {
    let (tx, handle) =
        ProofTreeWorker::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root".to_string(),
        mv: Move::NONE,
        outcome: Outcome::Win,
        depth: 5,
    }))
    .unwrap();
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root.e2e4".to_string(),
        mv: Move::make_move(Square::E2, Square::E4),
        outcome: Outcome::Loss,
        depth: 4,
    }))
    .unwrap();
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root.d2d4".to_string(),
        mv: Move::make_move(Square::D2, Square::D4),
        outcome: Outcome::Loss,
        depth: 2,
    }))
    .unwrap();
    // A deeper duplicate of the selected child must be ignored, not appended.
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root.d2d4".to_string(),
        mv: Move::make_move(Square::D2, Square::D4),
        outcome: Outcome::Loss,
        depth: 6,
    }))
    .unwrap();

    let (reply_tx, reply_rx) = channel();
    tx.send(ProofMessage::GetTree(reply_tx)).unwrap();
    let ProofResponse::Tree(tree) = reply_rx.recv().unwrap() else {
        panic!("expected Tree response");
    };
    assert_eq!(tree.nodes[0].children.len(), 1);
    assert_eq!(
        tree.nodes[tree.nodes[0].children[0]].mv,
        Move::make_move(Square::D2, Square::D4)
    );
    assert_eq!(tree.nodes[tree.nodes[0].children[0]].depth, 2);

    drop(tx);
    handle.join().unwrap();
}

#[test]
fn worker_loss_parent_keeps_all_distinct_win_children() {
    let (tx, handle) =
        ProofTreeWorker::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root".to_string(),
        mv: Move::NONE,
        outcome: Outcome::Loss,
        depth: 5,
    }))
    .unwrap();
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root.e2e4".to_string(),
        mv: Move::make_move(Square::E2, Square::E4),
        outcome: Outcome::Win,
        depth: 4,
    }))
    .unwrap();
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root.d2d4".to_string(),
        mv: Move::make_move(Square::D2, Square::D4),
        outcome: Outcome::Win,
        depth: 2,
    }))
    .unwrap();

    let (reply_tx, reply_rx) = channel();
    tx.send(ProofMessage::GetTree(reply_tx)).unwrap();
    let ProofResponse::Tree(tree) = reply_rx.recv().unwrap() else {
        panic!("expected Tree response");
    };
    assert_eq!(tree.nodes[0].children.len(), 2);

    drop(tx);
    handle.join().unwrap();
}

#[test]
fn worker_updates_existing_child_with_shorter_depth() {
    let (tx, handle) =
        ProofTreeWorker::spawn("fen".to_string(), 256, Arc::new(AtomicBool::new(false)));
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root".to_string(),
        mv: Move::NONE,
        outcome: Outcome::Loss,
        depth: 5,
    }))
    .unwrap();
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root.e2e4".to_string(),
        mv: Move::make_move(Square::E2, Square::E4),
        outcome: Outcome::Win,
        depth: 4,
    }))
    .unwrap();
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root.d2d4".to_string(),
        mv: Move::make_move(Square::D2, Square::D4),
        outcome: Outcome::Win,
        depth: 2,
    }))
    .unwrap();

    let (reply_tx, reply_rx) = channel();
    tx.send(ProofMessage::GetTree(reply_tx)).unwrap();
    let ProofResponse::Tree(tree) = reply_rx.recv().unwrap() else {
        panic!("expected Tree response");
    };
    assert_eq!(tree.nodes[0].children.len(), 2);

    // A duplicate with a shorter depth updates the existing child.
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root.e2e4".to_string(),
        mv: Move::make_move(Square::E2, Square::E4),
        outcome: Outcome::Win,
        depth: 1,
    }))
    .unwrap();

    let (tree_tx, tree_rx) = channel();
    tx.send(ProofMessage::GetTree(tree_tx)).unwrap();
    let ProofResponse::Tree(tree2) = tree_rx.recv().unwrap() else {
        panic!("expected Tree response");
    };
    assert_eq!(tree2.nodes[0].children.len(), 2);
    let e2e4_id = tree2.index["root.e2e4"];
    assert_eq!(tree2.nodes[e2e4_id].depth, 1);

    drop(tx);
    handle.join().unwrap();
}

#[test]
fn worker_sets_memory_limited_flag() {
    let flag = Arc::new(AtomicBool::new(false));
    let (tx, handle) = ProofTreeWorker::spawn("fen".to_string(), 0, Arc::clone(&flag));
    tx.send(ProofMessage::NodeProven(NodeProven {
        path: "root".to_string(),
        mv: Move::NONE,
        outcome: Outcome::Win,
        depth: 0,
    }))
    .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(
        flag.load(Ordering::Acquire),
        "memory flag should be set for zero budget"
    );
    drop(tx);
    handle.join().unwrap();
}

#[test]
fn worker_new_does_not_spawn_thread() {
    let mut worker = ProofTreeWorker::new(
        "fen".to_string(),
        usize::MAX,
        Arc::new(AtomicBool::new(false)),
    );

    let (tx, rx) = channel();
    let response = worker.handle_message(ProofMessage::GetStats(tx));
    let stats = match rx.recv().unwrap() {
        ProofResponse::Stats(s) => s,
        _ => panic!("expected Stats response"),
    };
    assert!(matches!(response, Some(ProofResponse::Stats(_))));
    assert_eq!(stats.nodes, 1);
}

#[test]
fn handle_message_processes_out_of_order_events() {
    let mut worker = ProofTreeWorker::new(
        "fen".to_string(),
        usize::MAX,
        Arc::new(AtomicBool::new(false)),
    );

    worker.handle_message(ProofMessage::NodeProven(NodeProven {
        path: "root.e2e4.e7e5".to_string(),
        mv: Move::make_move(Square::E7, Square::E5),
        outcome: Outcome::Win,
        depth: 0,
    }));
    worker.handle_message(ProofMessage::NodeProven(NodeProven {
        path: "root.e2e4".to_string(),
        mv: Move::make_move(Square::E2, Square::E4),
        outcome: Outcome::Loss,
        depth: 1,
    }));
    worker.handle_message(ProofMessage::NodeProven(NodeProven {
        path: "root".to_string(),
        mv: Move::NONE,
        outcome: Outcome::Win,
        depth: 2,
    }));

    let (tx, rx) = channel();
    worker.handle_message(ProofMessage::GetStats(tx));
    let ProofResponse::Stats(stats) = rx.recv().unwrap() else {
        panic!("expected Stats response");
    };
    assert_eq!(stats.nodes, 3);
    assert_eq!(stats.win_nodes, 2);
    assert_eq!(stats.loss_nodes, 1);
}

#[test]
fn handle_message_clears_tree() {
    let mut worker = ProofTreeWorker::new(
        "fen".to_string(),
        usize::MAX,
        Arc::new(AtomicBool::new(false)),
    );

    worker.handle_message(ProofMessage::NodeProven(NodeProven {
        path: "root".to_string(),
        mv: Move::NONE,
        outcome: Outcome::Win,
        depth: 2,
    }));
    worker.handle_message(ProofMessage::NodeProven(NodeProven {
        path: "root.e2e4".to_string(),
        mv: Move::make_move(Square::E2, Square::E4),
        outcome: Outcome::Loss,
        depth: 1,
    }));
    worker.handle_message(ProofMessage::Clear);

    let (tx, rx) = channel();
    worker.handle_message(ProofMessage::GetStats(tx));
    let ProofResponse::Stats(stats) = rx.recv().unwrap() else {
        panic!("expected Stats response");
    };
    assert_eq!(stats.nodes, 1);
    assert_eq!(stats.win_nodes, 0);
    assert_eq!(stats.loss_nodes, 0);
}

#[test]
fn memory_limited_flag_triggers_at_small_budget() {
    let flag = Arc::new(AtomicBool::new(false));
    let mut worker = ProofTreeWorker::new("fen".to_string(), 0, Arc::clone(&flag));

    worker.handle_message(ProofMessage::NodeProven(NodeProven {
        path: "root".to_string(),
        mv: Move::NONE,
        outcome: Outcome::Win,
        depth: 0,
    }));

    assert!(
        flag.load(Ordering::Acquire),
        "zero budget should set memory flag"
    );
}

#[test]
fn solve_populates_proof_tree_with_nodes() {
    use crate::position::Position;
    use crate::search::dfpn::Search;

    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let (tx, handle) = ProofTreeWorker::spawn(pos.fen(), 64, Arc::new(AtomicBool::new(false)));
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_proof_tree_sender(Some(tx.clone()));
    let (outcome, _pv, _nodes) = search.solve(&mut pos);

    let (reply_tx, reply_rx) = channel();
    tx.send(ProofMessage::GetStats(reply_tx)).unwrap();
    let ProofResponse::Stats(stats) = reply_rx.recv().unwrap() else {
        panic!("expected Stats response");
    };

    assert_eq!(outcome, Outcome::Win);
    assert!(
        stats.nodes > 0,
        "proof tree should contain at least the root node and proven children"
    );

    // Drop the search (and the sender clone it holds) so the worker channel
    // closes and the worker can exit.
    drop(search);
    drop(tx);
    handle.join().unwrap();
}
