//! Cross-module DF-PN unit tests.
//!
//! This file collects tests for private `Search` methods that exercise several
//! submodules together (simulation, cross-path twin lookup, and the public
//! solver entry points). Splitting it would force those tests into modules that
//! do not own the code under test, so it is intentionally kept slightly above
//! the usual 10 KB source-file limit.

use crate::position::{Outcome, Position};
use crate::search::dfpn::Search;
use crate::search::dfpn::simulate::SIM_MAX_DEPTH;
use atomic_movegen::types::{Move, Square};

#[test]
fn simulate_repeated_position_is_draw_only() {
    let mut search = Search::new(64);
    let mut pos = Position::from_fen("7k/8/8/8/8/8/2q5/K7 w - - 0 1").unwrap();
    let rep_key = pos.repetition_key();
    search.path.insert(rep_key);

    let mut sim_path = search.path.clone();
    let mut sim_stack = search.path_stack.clone();
    let mut sim_nodes = 0;

    assert!(search.simulate(
        &mut pos,
        0,
        0,
        Outcome::Draw,
        Move::NONE,
        &mut sim_path,
        &mut sim_stack,
        &mut sim_nodes,
        SIM_MAX_DEPTH,
    ));
    assert!(!search.simulate(
        &mut pos,
        0,
        0,
        Outcome::Win,
        Move::NONE,
        &mut sim_path,
        &mut sim_stack,
        &mut sim_nodes,
        SIM_MAX_DEPTH,
    ));
    assert!(!search.simulate(
        &mut pos,
        0,
        0,
        Outcome::Loss,
        Move::NONE,
        &mut sim_path,
        &mut sim_stack,
        &mut sim_nodes,
        SIM_MAX_DEPTH,
    ));
}

#[test]
fn simulate_loss_branch_rejects_stalemate() {
    let search = Search::new(64);
    let mut pos = Position::from_fen("7k/8/8/8/8/8/2q5/K7 w - - 0 1").unwrap();

    let mut sim_path = search.path.clone();
    let mut sim_stack = search.path_stack.clone();
    let mut sim_nodes = 0;

    assert!(!search.simulate(
        &mut pos,
        0,
        0,
        Outcome::Loss,
        Move::NONE,
        &mut sim_path,
        &mut sim_stack,
        &mut sim_nodes,
        SIM_MAX_DEPTH,
    ));
}

#[test]
fn try_use_tt_simulation_uses_current_path() {
    let mut search = Search::new(64);
    let pos = Position::from_fen("7k/8/8/8/8/8/2q5/K7 w - - 0 1").unwrap();
    let key = pos.hash();
    let rep_key = pos.repetition_key();
    search.path.insert(rep_key);
    search.path_stack.push(rep_key);
    search.path_code = 0;

    // Store a Draw twin for a different path code.
    let twin_path_code = 0xDEAD_BEEF;
    search.tt.store_twin(
        key,
        twin_path_code,
        0,
        Outcome::Draw,
        Move::NONE,
        0,
        u32::MAX,
    );

    let entry = *search.tt.probe(key).unwrap();
    let resolved = search.try_use_tt(&pos, &entry, u32::MAX, 0, 0);
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap().outcome, Outcome::Draw);
}

#[test]
fn try_use_tt_rejects_win_twin_for_repeated_position() {
    let mut search = Search::new(64);
    let pos = Position::from_fen("7k/8/8/8/8/8/2q5/K7 w - - 0 1").unwrap();
    let key = pos.hash();
    let rep_key = pos.repetition_key();
    search.path.insert(rep_key);
    search.path_stack.push(rep_key);
    search.path_code = 0;

    // Store a Win twin for a different path code. The current search prefix
    // already contains this position, so the real outcome is Draw, not Win.
    let twin_path_code = 0xDEAD_BEEF;
    search.tt.store_twin(
        key,
        twin_path_code,
        0,
        Outcome::Win,
        Move::NONE,
        0,
        u32::MAX,
    );

    let entry = *search.tt.probe(key).unwrap();
    assert!(search.try_use_tt(&pos, &entry, u32::MAX, 0, 0).is_none());
}

#[test]
fn try_use_tt_accepts_cross_path_win_twin() {
    let mut search = Search::new(64);
    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let key = pos.hash();
    search.path_code = 0;

    // Store a Win twin from a different path; the best move e1e8 mates.
    let twin_path_code = 0x0ABC;
    search.tt.store_twin(
        key,
        twin_path_code,
        0,
        Outcome::Win,
        Move::make_move(Square::E1, Square::E8),
        1,
        u32::MAX,
    );

    let entry = *search.tt.probe(key).unwrap();
    let resolved = search.try_use_tt(&pos, &entry, u32::MAX, 0, 0);
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap().outcome, Outcome::Win);
}

#[test]
fn try_use_tt_rejects_cross_path_win_twin_without_child_proof() {
    // A Win twin from another path is only trustworthy if the stored proof
    // tree can be simulated under the current prefix. Here the twin's best
    // move leads to a non-terminal position with no matching child twin, so
    // simulation fails and the twin is rejected.
    let mut search = Search::new(64);
    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let key = pos.hash();
    search.path_code = 0;

    let twin_path_code = 0x0ABC;
    let best = Move::make_move(Square::E1, Square::D1);
    search
        .tt
        .store_twin(key, twin_path_code, 0, Outcome::Win, best, 100, u32::MAX);

    let entry = *search.tt.probe(key).unwrap();
    assert!(search.try_use_tt(&pos, &entry, u32::MAX, 0, 0).is_none());
}
