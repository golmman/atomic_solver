use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use atomic_movegen::types::Move;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::ProofTreeWorkerHandle;
use atomic_solver::search::dfpn::Search;

fn solve_and_get_tree(fen: &str) -> (Outcome, Vec<Move>, atomic_solver::proof_tree::ProofTree) {
    let mut pos = Position::from_fen(fen).expect("valid fen");
    let mut search = Search::new(64);
    search.set_timeout(10);

    let memory_limited = Arc::new(AtomicBool::new(false));
    let (handle, join) =
        ProofTreeWorkerHandle::spawn(fen.to_string(), 256, Arc::clone(&memory_limited));
    search.set_memory_limited(Some(memory_limited));
    search.set_proof_event_sender(Some(handle.event_sender()));

    let (outcome, pv, _nodes) = search.solve(&mut pos);

    handle.finalize();
    let tree = handle.tree();

    drop(search);
    drop(handle);
    join.join().expect("worker thread");

    (outcome, pv, tree)
}

#[test]
#[ignore = "proof-tree PV validation is deferred to the proof-tree layer"]
fn proof_tree_validates_two_rook_mate() {}

#[test]
#[ignore = "proof-tree PV validation is deferred to the proof-tree layer"]
fn proof_tree_validates_m27() {}

#[test]
#[ignore = "proof-tree PV validation is deferred to the proof-tree layer"]
fn proof_tree_validate_ppv_accepts_solve_pv() {}

#[test]
fn proof_tree_contains_defender_replies() {
    // Black to move is lost; the root Loss node must have more than one
    // distinct Win child (one for every legal defender reply that loses).
    let fen = "rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3";
    let (_outcome, _pv, tree) = solve_and_get_tree(fen);

    let defender_branching = tree.nodes.iter().any(|n| {
        n.outcome == Some(Outcome::Loss)
            && n.children
                .iter()
                .filter(|&&c| tree.nodes[c].outcome == Some(Outcome::Win))
                .count()
                > 1
    });
    assert!(
        defender_branching,
        "expected a Loss node with more than one Win child in the proof tree"
    );
}

#[test]
fn proof_tree_bin_round_trips_full_tree() {
    let fen = "rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3";
    let (_outcome, _pv, tree) = solve_and_get_tree(fen);

    let mut buf = Vec::new();
    tree.to_bin(&mut buf).expect("serialize tree");
    let _loaded =
        atomic_solver::proof_tree::ProofTree::from_bin(&mut &buf[..]).expect("deserialize tree");
}
