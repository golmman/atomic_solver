//! Unit tests for the opt-in win-only front-end.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use atomic_movegen::types::Move;

use crate::position::{Outcome, Position};
use crate::proof_event::ProofEvent;
use crate::search::dfpn::{PvStatus, Search};
use crate::zobrist::INF;

use super::WinProver;

/// White to move wins in one move: `Re8` captures the black commoner.
const MATE_IN_ONE: &str = "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1";
/// A busy non-terminal position (the start position) used for cap tests.
const STARTPOS: &str = Position::STARTPOS_FEN;

fn prover(budget: u64) -> WinProver {
    WinProver::new(
        crate::search::ordering::StaticAtomicScorer::default(),
        budget,
        std::time::Instant::now() + std::time::Duration::from_secs(30),
        None,
    )
}

fn position(fen: &str) -> Position {
    Position::from_fen(fen).unwrap()
}

#[test]
fn proves_mate_in_one_with_replayable_pv() {
    let pos = position(MATE_IN_ONE);
    let mut front = prover(10_000);
    let proof = front.solve(&pos).expect("one-move win must be proven");
    assert_eq!(proof.depth, 1);
    assert_eq!(proof.pv.len(), 1);

    let mut replay = pos.clone();
    for mv in &proof.pv {
        let mut moves = atomic_movegen::types::MoveList::new();
        replay.legal_moves(&mut moves);
        assert!(moves.as_slice().contains(mv), "PV move must be legal");
        replay.do_move(*mv);
    }
    assert_eq!(replay.outcome(), Some(Outcome::Loss));
    assert!(front.child_evaluations() > 0);
    assert!(front.nodes() > 0);
}

#[test]
fn terminal_root_is_not_a_front_end_claim() {
    // Two bare commoners: a terminal Draw.
    let pos = position("7k/8/8/8/8/8/8/7K w - - 0 1");
    let mut front = prover(10_000);
    assert!(front.solve(&pos).is_none());
    assert_eq!(front.child_evaluations(), 0);
}

#[test]
fn repetition_is_a_draw_not_a_win() {
    let pos = position(MATE_IN_ONE);
    let mut front = prover(10_000);
    // Pretend the root is already on the current path.
    front.path_push(pos.repetition_key());
    let mut root = pos.clone();
    let outcome = front.dfpn(&mut root, INF, INF, true);
    assert_eq!(outcome, Outcome::Draw);
}

#[test]
fn rule50_root_is_not_a_win() {
    let pos = position("4k3/8/8/8/8/8/8/4R1K1 w - - 100 1");
    assert_eq!(pos.outcome(), Some(Outcome::Draw));
    let mut front = prover(10_000);
    assert!(front.solve(&pos).is_none());
}

#[test]
fn zero_budget_is_a_no_op() {
    let pos = position(MATE_IN_ONE);
    let mut front = prover(0);
    assert!(front.solve(&pos).is_none());
    assert_eq!(front.child_evaluations(), 0);
    assert_eq!(front.nodes(), 0);
}

#[test]
fn cap_bounds_the_work_and_reports_it() {
    let pos = position(STARTPOS);
    let mut front = prover(100);
    assert!(
        front.solve(&pos).is_none(),
        "startpos is not a 100-eval win"
    );
    // The cap is checked at frame-loop heads, so one child sweep may cross it.
    assert!(
        front.child_evaluations() >= 100,
        "the cap must be reached, got {}",
        front.child_evaluations()
    );
    assert!(front.nodes() > 0);
}

#[test]
fn run_is_deterministic() {
    let pos = position(STARTPOS);
    let first = {
        let mut front = prover(500);
        let proof = front.solve(&pos);
        (proof, front.child_evaluations(), front.nodes())
    };
    let second = {
        let mut front = prover(500);
        let proof = front.solve(&pos);
        (proof, front.child_evaluations(), front.nodes())
    };
    assert_eq!(first, second);
}

#[test]
fn proof_events_replay_to_the_claimed_outcomes() {
    let pos = position(MATE_IN_ONE);
    let mut front = prover(10_000);
    let (tx, rx) = std::sync::mpsc::channel();
    front.set_proof_event_sender(Some(tx));
    let proof = front.solve(&pos).expect("proven");
    drop(front);

    let events: Vec<ProofEvent> = rx.try_iter().collect();
    assert!(!events.is_empty(), "a proven win must emit events");

    let mut saw_root = false;
    for event in events {
        let ProofEvent::NodeProven(node) = event;
        let mut replay = pos.clone();
        for mv in &node.path {
            let mut moves = atomic_movegen::types::MoveList::new();
            replay.legal_moves(&mut moves);
            assert!(moves.as_slice().contains(mv), "event path must be legal");
            replay.do_move(*mv);
        }
        assert_eq!(replay.hash(), node.hash, "event hash must match replay");
        if let Some(terminal) = replay.outcome() {
            assert_eq!(terminal, node.outcome, "terminal event outcome drift");
        }
        if node.path.is_empty() {
            saw_root = true;
            assert_eq!(node.outcome, Outcome::Win);
            assert_eq!(node.depth, proof.depth);
        }
    }
    assert!(saw_root, "the root must be among the emitted events");
}

#[test]
fn front_end_ignores_general_search_heuristics() {
    let pos = position(MATE_IN_ONE);
    let solve = |levers: bool| {
        let mut search = Search::new(16);
        search.set_first_outcome_only(true);
        search.set_win_front_end(true);
        search.set_win_front_end_budget(10_000);
        if levers {
            search.set_move_class_pn(true);
            search.set_quiet_grouping(true);
            search.set_eval_pn(crate::search::dfpn::EvalPnMode::Or);
        }
        let mut root = pos.clone();
        let (outcome, pv, _) = search.solve(&mut root);
        (
            outcome,
            pv,
            search.pv_status(),
            search.win_front_end_evaluations(),
            search.win_front_end_nodes(),
        )
    };
    let plain = solve(false);
    let levered = solve(true);
    assert_eq!(plain.0, Outcome::Win);
    assert_eq!(plain.2, PvStatus::WinFrontEnd);
    assert_eq!(plain, levered);
    assert_eq!(plain.1.len(), 1);
}

#[test]
fn pv_extraction_stops_on_an_illegal_first_move() {
    // Defensive: an empty PV cannot validate as a win.
    let pos = position(MATE_IN_ONE);
    let front = prover(100);
    assert!(!front.validate_win(&[], &pos));
    assert!(front.validate_win(
        &[Move::make_move(
            atomic_movegen::types::Square::E1,
            atomic_movegen::types::Square::E8,
        )],
        &pos
    ));
}

#[test]
fn stop_flag_cuts_the_run() {
    let flag = Arc::new(AtomicBool::new(true));
    let mut front = WinProver::new(
        crate::search::ordering::StaticAtomicScorer::default(),
        10_000,
        std::time::Instant::now() + std::time::Duration::from_secs(30),
        Some(flag),
    );
    let pos = position(MATE_IN_ONE);
    assert!(front.solve(&pos).is_none());
}
