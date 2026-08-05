use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;

use atomic_movegen::types::Move;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofMessage, ProofResponse, ProofTreeWorker};
use atomic_solver::search::dfpn::Search;

fn solve_and_get_tree(fen: &str) -> (Outcome, Vec<Move>, atomic_solver::proof_tree::ProofTree) {
    let mut pos = Position::from_fen(fen).expect("valid fen");
    let mut search = Search::new(64);
    search.set_timeout(10);

    let memory_limited = Arc::new(AtomicBool::new(false));
    let (tx, handle) = ProofTreeWorker::spawn(fen.to_string(), 256, Arc::clone(&memory_limited));
    search.set_memory_limited(Some(memory_limited));
    search.set_proof_tree_sender(Some(tx.clone()));

    let (outcome, pv, _nodes) = search.solve(&mut pos);

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

    (outcome, pv, tree)
}

#[test]
fn proof_tree_validates_two_rook_mate() {
    let fen = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";
    let (outcome, pv, tree) = solve_and_get_tree(fen);
    assert_eq!(outcome, Outcome::Win);
    assert!(!pv.is_empty(), "expected a non-empty PV");
    assert_eq!(pv.len(), 3, "expected a 3-plies mate");

    let pos = Position::from_fen(fen).expect("valid fen");
    assert!(
        tree.validate_ppv(&pv),
        "proof tree must validate the returned PV"
    );
    assert!(Search::validate_pv(&pv, &pos, Outcome::Win, None));
}

#[test]
fn proof_tree_validates_m27() {
    let fen = "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26";
    let (outcome, pv, tree) = solve_and_get_tree(fen);
    assert_eq!(outcome, Outcome::Win, "expected White to win");
    assert_eq!(pv.len(), 7, "expected a 7-plies mate");

    let pos = Position::from_fen(fen).expect("valid fen");
    assert!(
        tree.validate_ppv(&pv),
        "proof tree must validate the returned PV"
    );
    assert!(Search::validate_pv(&pv, &pos, Outcome::Win, None));
}

#[test]
fn proof_tree_validate_ppv_accepts_solve_pv() {
    let fen = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";
    let (outcome, pv, tree) = solve_and_get_tree(fen);
    assert_eq!(outcome, Outcome::Win);
    assert!(
        tree.validate_ppv(&pv),
        "solve PV must validate against the tree"
    );
    assert!(pv.len() <= 3, "mate should be at most 3 plies");
}

#[test]
fn proof_tree_contains_defender_replies() {
    // Black to move is lost; the root Loss node must have more than one
    // distinct Win child (one for every legal defender reply that loses).
    let fen = "rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3";
    let (_outcome, _pv, tree) = solve_and_get_tree(fen);

    let defender_branching = tree.nodes.iter().any(|n| {
        n.outcome == Outcome::Loss
            && n.children
                .iter()
                .filter(|&&c| tree.nodes[c].outcome == Outcome::Win)
                .count()
                > 1
    });
    assert!(
        defender_branching,
        "expected a Loss node with more than one Win child in the proof tree"
    );

    let (outcome, pv, _) = solve_and_get_tree(fen);
    assert_eq!(outcome, Outcome::Loss);
    let pos = Position::from_fen(fen).expect("valid fen");
    assert!(Search::validate_pv(&pv, &pos, Outcome::Loss, None));
    assert!(tree.validate_ppv(&pv));
}

#[test]
fn proof_tree_bin_round_trips_full_tree() {
    let fen = "rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3";
    let (outcome, pv, tree) = solve_and_get_tree(fen);
    assert_eq!(outcome, Outcome::Loss);
    assert!(
        tree.validate_ppv(&pv),
        "solve PV must validate before round-trip"
    );

    let mut buf = Vec::new();
    tree.to_bin(&mut buf).expect("serialize tree");
    let loaded =
        atomic_solver::proof_tree::ProofTree::from_bin(&mut &buf[..]).expect("deserialize tree");

    let pos = Position::from_fen(fen).expect("valid fen");
    assert!(
        loaded.validate_ppv(&pv),
        "round-tripped tree must validate the solve PV"
    );
    assert!(Search::validate_pv(&pv, &pos, Outcome::Loss, None));
}
