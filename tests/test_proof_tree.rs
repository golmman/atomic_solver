use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;
use atomic_solver::proof_tree::{ProofMessage, ProofResponse, ProofTreeWorker};
use atomic_solver::search::dfpn::Search;

fn solve_and_extract_ppv(fen: &str) -> (String, Vec<String>, Vec<String>) {
    let mut pos = Position::from_fen(fen).expect("valid fen");
    let mut search = Search::new(64);
    search.set_timeout(10);
    search.refine_shortest(true);

    let memory_limited = Arc::new(AtomicBool::new(false));
    let (tx, handle) = ProofTreeWorker::spawn(fen.to_string(), 256, Arc::clone(&memory_limited));
    search.set_memory_limited(Some(memory_limited));
    search.set_proof_tree_sender(Some(tx.clone()));

    let (_outcome, pv, _nodes) = search.solve(&mut pos);

    let (reply_tx, reply_rx) = channel();
    tx.send(ProofMessage::GetTree(reply_tx))
        .expect("send GetTree");
    let tree = match reply_rx.recv().expect("recv tree") {
        ProofResponse::Tree(t) => t,
        _ => panic!("expected Tree response"),
    };

    drop(search);
    drop(tx);
    handle.join().expect("worker thread");

    let tree_ppv = tree.extract_ppv();
    let expected = pv.iter().map(|&m| move_to_uci(m)).collect();
    (fen.to_string(), tree_ppv, expected)
}

#[test]
fn proof_tree_ppv_matches_two_rook_mate() {
    let fen = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";
    let (_fen, tree_ppv, expected) = solve_and_extract_ppv(fen);
    assert_eq!(tree_ppv, expected, "proof-tree PPV should match solver PV");
    assert_eq!(tree_ppv.len(), 3, "expected a 3-plies mate");
}

#[test]
fn proof_tree_ppv_matches_m27() {
    let fen = "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26";
    let (_fen, tree_ppv, expected) = solve_and_extract_ppv(fen);
    assert_eq!(tree_ppv, expected, "proof-tree PPV should match solver PV");
    assert_eq!(tree_ppv.len(), 7, "expected a 7-plies mate");
}

#[test]
fn proof_tree_validate_ppv_accepts_extracted_line() {
    let fen = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";
    let mut pos = Position::from_fen(fen).expect("valid fen");
    let mut search = Search::new(64);
    search.set_timeout(10);
    search.refine_shortest(true);

    let memory_limited = Arc::new(AtomicBool::new(false));
    let (tx, handle) = ProofTreeWorker::spawn(fen.to_string(), 256, Arc::clone(&memory_limited));
    search.set_memory_limited(Some(memory_limited));
    search.set_proof_tree_sender(Some(tx.clone()));

    let _ = search.solve(&mut pos);

    let (reply_tx, reply_rx) = channel();
    tx.send(ProofMessage::GetTree(reply_tx))
        .expect("send GetTree");
    let tree = match reply_rx.recv().expect("recv tree") {
        ProofResponse::Tree(t) => t,
        _ => panic!("expected Tree response"),
    };

    drop(search);
    drop(tx);
    handle.join().expect("worker thread");

    let ppv = tree.extract_ppv();
    assert!(
        tree.validate_ppv(&ppv),
        "extracted PPV must validate against the tree"
    );
    assert!(ppv.len() <= 3, "mate should be at most 3 plies");
}
