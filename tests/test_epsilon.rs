mod common;

use std::process::Command;

use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use common::{cli_bin, pv_strings};

fn solve_with_epsilon_full(fen: &str, epsilon: f64) -> (Outcome, Vec<String>, u64) {
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    search.set_epsilon(epsilon);
    let (outcome, pv, nodes) = search.solve(&mut pos);
    (outcome, pv_strings(&pv), nodes)
}

fn solve_with_epsilon(fen: &str, epsilon: f64) -> Outcome {
    solve_with_epsilon_full(fen, epsilon).0
}

#[test]
fn different_epsilon_values_solve_simple_mate() {
    let fen = "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1";
    for epsilon in [0.0, 0.01, 0.25, 0.5, 0.99, 1.0] {
        let (outcome, _pv, _nodes) = solve_with_epsilon_full(fen, epsilon);
        assert_eq!(
            outcome,
            Outcome::Win,
            "epsilon {epsilon} should solve the rook mate"
        );
    }
}

#[test]
fn cli_epsilon_solves_simple_position() {
    let output = Command::new(cli_bin())
        .args([
            "--epsilon",
            "0.5",
            "--fen",
            "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1",
        ])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "binary failed: {stderr}");
    assert!(
        stdout.contains("outcome: win"),
        "expected win output, got:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("pv:"),
        "expected a PV in output, got:\n{stdout}"
    );
}

#[test]
fn cli_rejects_out_of_range_epsilon() {
    let output = Command::new(cli_bin())
        .args([
            "--epsilon",
            "1.1",
            "--fen",
            "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1",
        ])
        .output()
        .expect("failed to run CLI binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected exit failure for epsilon=1.1"
    );
    assert!(
        stderr.contains("epsilon must be in [0.0, 1.0]"),
        "expected clear epsilon error, got: {stderr}"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow in debug builds")]
fn epsilon_zero_solves_mate_in_two() {
    let fen = "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3";
    let (outcome, _pv, _nodes) = solve_with_epsilon_full(fen, 0.0);
    assert_eq!(outcome, Outcome::Win, "epsilon 0 should win from the start");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow in debug builds")]
fn epsilon_thresholds_do_not_claim_win_in_cyclic_position() {
    let fen = "8/8/8/8/2k5/8/8/4KR2 w - - 0 1";
    for epsilon in [0.0, 0.25, 0.5] {
        let outcome = solve_with_epsilon(fen, epsilon);
        assert_ne!(
            outcome,
            Outcome::Win,
            "epsilon {epsilon} should not claim a win in a drawn cyclic position"
        );
    }
}
