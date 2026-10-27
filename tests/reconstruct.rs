//! Integration tests for offline proof reconstruction (`docs/plans/proof/plan5.md`).
//!
//! Fast tier: dual-build oracle on a short mate plus the fill path with the
//! root record dropped. The slow-tier end-to-end test on a deeper decisive
//! fixture is `#[ignore]`d.

mod common;

use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofTree, ProofTreeWorkerHandle, validate_proof_tree};
use atomic_solver::reconstruct::{ReconstructConfig, reconstruct, tree_signature};
use atomic_solver::search::dfpn::Search;
use atomic_solver::tt_snapshot::{read_tt_snapshot, write_tt_snapshot};
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const MATE3_FEN: &str = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";

/// Build the live proof tree with worker events on and return it together
/// with the TT snapshot bytes and the search's child-eval count.
fn live_build(fen: &str) -> (Outcome, u64, Vec<u8>, ProofTree) {
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
    let child_evals = search.child_evaluations();
    let mut snapshot = Vec::new();
    write_tt_snapshot(search.tt(), fen, 64, &mut snapshot).unwrap();
    assert!(
        !memory_limited.load(Ordering::Acquire),
        "live worker must succeed for the dual-build oracle"
    );
    handle.finalize();
    let tree = handle.tree();
    drop(search);
    drop(handle);
    let _ = join.join();
    (outcome, child_evals, snapshot, tree)
}

fn reconstruct_from(
    snapshot: &[u8],
    root_fen: &str,
) -> atomic_solver::reconstruct::ReconstructOutput {
    let (_header, solved, _unsolved) = read_tt_snapshot(&mut Cursor::new(snapshot)).unwrap();
    reconstruct(root_fen, &solved, &ReconstructConfig::default())
}

#[test]
fn dual_build_oracle_two_rook_mate() {
    let (outcome, _c, snapshot, live_tree) = live_build(MATE3_FEN);
    assert_eq!(outcome, Outcome::Win);

    let output = reconstruct_from(&snapshot, MATE3_FEN);
    assert!(
        output.error.is_none(),
        "reconstruction failed: {:?}",
        output.error
    );
    assert_eq!(output.root_outcome, Some(Outcome::Win));
    assert!(output.stats.hit > 0, "exact snapshot hits expected");
    assert!(output.stats.anomalies == 0 && output.stats.unfillable == 0);

    let tree = output
        .tree
        .expect("successful reconstruction carries a tree");
    assert_eq!(
        tree_signature(&live_tree),
        tree_signature(&tree),
        "reconstructed tree must be isomorphic to the live event-built tree"
    );
}

#[test]
fn dual_build_oracle_with_root_record_dropped() {
    let (outcome, _c, snapshot, live_tree) = live_build(MATE3_FEN);
    assert_eq!(outcome, Outcome::Win);

    // Drop the root record: the walk must fill the root via a seeded local
    // solve and still reconstruct isomorphically.
    let (_header, mut solved, _unsolved) = read_tt_snapshot(&mut Cursor::new(&snapshot)).unwrap();
    let root_key = Position::from_fen(MATE3_FEN).unwrap().hash();
    let before = solved.len();
    solved.retain(|r| r.key != root_key);
    assert_eq!(solved.len(), before - 1);

    let output = reconstruct(MATE3_FEN, &solved, &ReconstructConfig::default());
    assert!(
        output.error.is_none(),
        "reconstruction failed: {:?}",
        output.error
    );
    assert_eq!(
        output.stats.filled, 1,
        "exactly the dropped root was filled"
    );
    let tree = output
        .tree
        .expect("successful reconstruction carries a tree");
    assert_eq!(
        tree_signature(&live_tree),
        tree_signature(&tree),
        "fill-path reconstruction must be isomorphic to the live tree"
    );
}

/// The harvest abort of 2026-10-27 (plan25): the job position was decisive at
/// ~64 M child evals, and a mate-in-1 record below the deep `e7e5` child had
/// been evicted from the session's retained TT. The walker's pre-plan25 fill
/// loop seeded the hole's bound from the parent's proven depth (46) and then
/// refused to search it at all ("unfillable hole … fill depth cap 32
/// exhausted", `fill_evals == 0`) — every such hole killed a harvest session.
///
/// D1 makes the parent-derived bound dominate the cap, so the hole is filled
/// at its true bound with the default config.
#[test]
#[ignore = "slow: deep-proof child hole (reported harvest abort, ~64M child evals)"]
fn deep_child_hole_exports_with_the_default_fill_cap() {
    // The aborted job: 9 plies from the startpos (clock 0), black to move.
    let job_fen = "rnbqkbnr/pp2p1pp/2pp4/8/5PP1/2N5/PPPPP2P/R1BQKB1R b KQkq f3 0 5";
    let mut pos = Position::from_fen(job_fen).unwrap();
    let mut search = Search::new(128);
    search.set_timeout(3600);
    search.set_first_outcome_only(true);
    // The production harvest budget (`proofdb_cycle_run`): the cold run peaks
    // at 63,954,107 child evals.
    search.set_child_eval_budget(100_000_000);
    let (outcome, _depth, _nodes) = search.search_depth_with_prefix(&mut pos, u32::MAX, &[]);
    assert_eq!(outcome, Outcome::Win, "the job must be decisive");

    let mut snapshot = Vec::new();
    write_tt_snapshot(search.tt(), job_fen, 128, &mut snapshot).unwrap();
    let (_header, solved, _unsolved) = read_tt_snapshot(&mut Cursor::new(&snapshot)).unwrap();
    let root_key = Position::from_fen(job_fen).unwrap().hash();
    let root = *solved
        .iter()
        .find(|r| r.key == root_key)
        .expect("the root record exists");
    assert_eq!(root.outcome, Outcome::Win);

    // The evicted hole: the root's best-move child (`e7e5`, the incident's
    // `root.e7e5.a2a3` hole's parent).
    let mut child_pos = Position::from_fen(job_fen).unwrap();
    child_pos.do_move(root.best_move);
    let child_key = child_pos.hash();
    let holes: Vec<_> = solved
        .iter()
        .copied()
        .filter(|r| r.key != child_key)
        .collect();
    assert_eq!(holes.len(), solved.len() - 1, "the child record is dropped");
    // The regression precondition: the parent's proven depth must exceed the
    // default cap, otherwise this test silently stops exercising D1.
    let cap = ReconstructConfig::default().fill_depth_cap;
    assert!(
        root.depth > cap,
        "regression precondition: root depth {} must exceed the fill cap {cap}",
        root.depth
    );

    let output = reconstruct(job_fen, &holes, &ReconstructConfig::default());
    assert!(
        output.error.is_none(),
        "reconstruction failed: {:?}",
        output.error
    );
    assert!(output.fill_evals > 0, "the hole is searched, not rejected");
    let tree = output
        .tree
        .expect("successful reconstruction carries a tree");
    assert!(
        validate_proof_tree(&tree).is_ok(),
        "the exported tree must replay-validate"
    );
}

#[test]
#[ignore = "slow: end-to-end reconstruction of a deeper decisive fixture (dec02, 30 plies)"]
fn dual_build_oracle_deep_fixture_dec02() {
    // dec02 from the decisive suite: a 30-ply Loss, solved within the
    // default 5-second budget (see tests/fixtures/decisive_positions.txt).
    let fen = "8/1k1p4/1P2p3/p2P1P2/P7/6p1/6K1/8 b - - 0 27";
    let (outcome, _c, snapshot, live_tree) = live_build(fen);
    assert_ne!(
        outcome,
        Outcome::Draw,
        "dec02 must solve within the timeout"
    );

    let output = reconstruct_from(&snapshot, fen);
    assert!(
        output.error.is_none(),
        "reconstruction failed: {:?}",
        output.error
    );
    assert_eq!(output.root_outcome, Some(outcome));
    let tree = output
        .tree
        .expect("successful reconstruction carries a tree");
    assert_eq!(
        tree_signature(&live_tree),
        tree_signature(&tree),
        "reconstructed tree must be isomorphic to the live event-built tree"
    );
}
