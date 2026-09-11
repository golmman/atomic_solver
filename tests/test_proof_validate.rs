//! Integration tests for plan6 (`docs/plans/proof/plan6.md`): the
//! twin-preference fix in `finalize()` and the replay-based Loss-completeness
//! validator.
//!
//! This file does not reuse the plan number in its name because
//! `tests/test_plan6.rs` is an unrelated, pre-existing m2x regression file.
//!
//! Fast tier: the two-rook mate fixture validated end-to-end on both build
//! paths (live worker events and snapshot reconstruction). The slow-tier
//! dec02 end-to-end test is `#[ignore]`d.

use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofTree, ProofTreeWorkerHandle, validate_proof_tree};
use atomic_solver::reconstruct::{ReconstructConfig, reconstruct};
use atomic_solver::search::dfpn::Search;
use atomic_solver::tt_snapshot::{read_tt_snapshot, write_tt_snapshot};

const MATE_FEN: &str = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";

/// Solve `fen` with worker events on, finalize, and return the outcome, the
/// TT snapshot bytes, and the finalized live tree.
fn live_build(fen: &str) -> (Outcome, Vec<u8>, ProofTree) {
    let memory_limited = Arc::new(AtomicBool::new(false));
    let (handle, join) =
        ProofTreeWorkerHandle::spawn(fen.to_string(), 256, Arc::clone(&memory_limited));
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(60);
    search.set_first_outcome_only(true);
    search.set_memory_limited(Some(Arc::clone(&memory_limited)));
    search.set_proof_event_sender(Some(handle.event_sender()));
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_ne!(outcome, Outcome::Draw, "fixture must solve");
    let mut snapshot = Vec::new();
    write_tt_snapshot(search.tt(), fen, 64, &mut snapshot).unwrap();
    assert!(
        !memory_limited.load(Ordering::Acquire),
        "live worker must succeed"
    );
    handle.finalize();
    let stats = handle.stats();
    assert_eq!(
        stats.validation_errors, 0,
        "the finalized live tree must validate clean"
    );
    let tree = handle.tree();
    drop(search);
    drop(handle);
    let _ = join.join();
    (outcome, snapshot, tree)
}

#[test]
fn finalized_live_tree_validates_clean_two_rook_mate() {
    let (_outcome, _snapshot, tree) = live_build(MATE_FEN);
    assert!(
        validate_proof_tree(&tree).is_ok(),
        "the finalized live tree must pass the replay-based validator"
    );
}

#[test]
fn reconstructed_tree_validates_clean_two_rook_mate() {
    let (outcome, snapshot, _live) = live_build(MATE_FEN);

    let (_header, solved, _unsolved) = read_tt_snapshot(&mut Cursor::new(&snapshot)).unwrap();
    let output = reconstruct(MATE_FEN, &solved, &ReconstructConfig::default());
    assert!(
        output.error.is_none(),
        "reconstruction failed: {:?}",
        output.error
    );
    assert_eq!(output.root_outcome, Some(outcome));
    let tree = output
        .tree
        .expect("successful reconstruction carries a tree");
    assert!(
        validate_proof_tree(&tree).is_ok(),
        "the reconstructed tree must pass the replay-based validator"
    );
}

#[test]
#[ignore = "slow: end-to-end solve + finalize + validate + reconstruct + validate on dec02 (30-ply decisive fixture)"]
fn dec02_end_to_end_validates_clean() {
    // dec02 from the decisive suite: a 30-ply Loss, solved well within the
    // timeout (see tests/fixtures/decisive_positions.txt).
    let fen = "8/1k1p4/1P2p3/p2P1P2/P7/6p1/6K1/8 b - - 0 27";
    let (outcome, snapshot, live_tree) = live_build(fen);
    assert!(
        validate_proof_tree(&live_tree).is_ok(),
        "the finalized live dec02 tree must validate clean"
    );

    let (_header, solved, _unsolved) = read_tt_snapshot(&mut Cursor::new(&snapshot)).unwrap();
    let output = reconstruct(fen, &solved, &ReconstructConfig::default());
    assert!(
        output.error.is_none(),
        "reconstruction failed: {:?}",
        output.error
    );
    assert_eq!(output.root_outcome, Some(outcome));
    let tree = output
        .tree
        .expect("successful reconstruction carries a tree");
    assert!(
        validate_proof_tree(&tree).is_ok(),
        "the reconstructed dec02 tree must validate clean"
    );
}
