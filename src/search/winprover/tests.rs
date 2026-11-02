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
/// A small forcing line where one refuted defence's proof transfers to its
/// open sibling. With simulation off the front-end needs 1,240 evals; with it
/// on the root is proven in 1,195 (fixture and cap measured at implementation
/// time; the cap 1,200 is the pre-registered transfer fixture).
const SIMULATION_FIXTURE: &str = "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26";
const SIMULATION_FIXTURE_CAP: u64 = 1_200;

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
        if node.depth == 0 {
            assert_eq!(
                replay.outcome(),
                Some(node.outcome),
                "depth-0 event must be terminal: path={:?} hash={} outcome={}",
                node.path,
                node.hash,
                node.outcome
            );
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

#[test]
fn simulation_transfers_a_refuted_defence_proof_to_a_sibling() {
    let pos = position(SIMULATION_FIXTURE);
    let mut without = prover(SIMULATION_FIXTURE_CAP);
    without.set_simulation(false);
    assert!(without.solve(&pos).is_none());
    assert_eq!(without.simulation_attempts(), 0);

    let mut with = prover(SIMULATION_FIXTURE_CAP);
    let proof = with
        .solve(&pos)
        .expect("simulation must close the fixture inside 1,200 evals");
    assert!(with.simulation_attempts() > 0);
    assert!(with.simulation_successes() > 0);
    assert!(with.simulation_evaluations() > 0);
    assert!(proof.depth > 0);
    assert!(with.validate_win(&proof.pv, &pos));
}

#[test]
fn simulation_reduces_the_work_on_the_transfer_fixture() {
    let pos = position(SIMULATION_FIXTURE);
    let run = |simulation: bool| {
        let mut front = prover(10_000);
        front.set_simulation(simulation);
        let proof = front.solve(&pos);
        (
            proof.map(|p| p.depth),
            front.child_evaluations(),
            front.simulation_successes(),
        )
    };
    let off = run(false);
    let on = run(true);
    assert_eq!(off.0.is_some(), on.0.is_some());
    assert!(
        on.1 < off.1,
        "simulation on must save evals: {on:?} vs {off:?}"
    );
    assert_eq!(off.2, 0);
    assert!(on.2 > 0);
}

#[test]
fn repetition_on_the_search_path_refuses_a_replay() {
    let pos = position(SIMULATION_FIXTURE);
    let guide = position(MATE_IN_ONE);
    let mut front = prover(10_000);
    front.path_push(pos.repetition_key());
    let mut budget = 16;
    let mut line = Vec::new();
    let mut path = Vec::new();
    assert!(
        front
            .simulate_attacker(&pos, &guide, &mut budget, &mut line, &mut path)
            .is_none()
    );
    assert_eq!(front.simulation_evaluations(), 0);
}

#[test]
fn rule50_position_is_not_a_replay_win() {
    let pos = position("6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 100 26");
    assert_eq!(pos.outcome(), Some(Outcome::Draw));
    let guide = position(MATE_IN_ONE);
    let mut front = prover(10_000);
    let mut budget = 16;
    let mut line = Vec::new();
    let mut path = Vec::new();
    assert!(
        front
            .simulate_attacker(&pos, &guide, &mut budget, &mut line, &mut path)
            .is_none()
    );
    assert_eq!(front.simulation_evaluations(), 0);
}

#[test]
fn replay_budget_bounds_the_work() {
    let pos = position(SIMULATION_FIXTURE);
    let guide = position(MATE_IN_ONE);
    let mut front = prover(10_000);
    let mut budget = 1;
    let mut line = Vec::new();
    let mut path = Vec::new();
    assert!(
        front
            .simulate_attacker(&pos, &guide, &mut budget, &mut line, &mut path)
            .is_none()
    );
    assert_eq!(budget, 0);
    assert_eq!(front.simulation_evaluations(), 1);
    assert_eq!(front.child_evaluations(), 1);
}

#[test]
fn simulation_runs_are_deterministic() {
    let pos = position(SIMULATION_FIXTURE);
    let run = || {
        let mut front = prover(10_000);
        let proof = front.solve(&pos);
        (
            proof,
            front.child_evaluations(),
            front.nodes(),
            front.simulation_attempts(),
            front.simulation_successes(),
            front.simulation_evaluations(),
        )
    };
    assert_eq!(run(), run());
}

#[test]
fn simulated_proof_events_replay_with_correct_paths_and_hashes() {
    let pos = position(SIMULATION_FIXTURE);
    let mut front = prover(SIMULATION_FIXTURE_CAP);
    let (tx, rx) = std::sync::mpsc::channel();
    front.set_proof_event_sender(Some(tx));
    let proof = front.solve(&pos).expect("proven with simulation");
    assert!(front.simulation_successes() > 0);
    drop(front);

    let events: Vec<ProofEvent> = rx.try_iter().collect();
    assert!(!events.is_empty());
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
    assert!(saw_root);
}
