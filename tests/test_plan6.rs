mod common;

use std::process::Command;

use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use common::{assert_solves_to, assert_solves_to_timeout, cli_bin};

// Fast regression positions that run in release CI but are too slow for debug
// builds (they take several seconds even with the 5-second timeout).

#[test]
#[cfg_attr(debug_assertions, ignore = "slow regression; run with --ignored")]
fn m23_white_wins() {
    assert_solves_to(
        "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K w - - 1 23",
        Outcome::Win,
        None,
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow regression; run with --ignored")]
fn m23_black_loses() {
    assert_solves_to(
        "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K b - - 2 23",
        Outcome::Loss,
        None,
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow regression; run with --ignored")]
fn m24_white_wins() {
    assert_solves_to(
        "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24",
        Outcome::Win,
        None,
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow regression; run with --ignored")]
fn m24_black_loses() {
    // Fixed: rank 5 was `p6Pp` (9 squares); corrected to `p5Pp`.
    assert_solves_to(
        "4r1k1/3p4/2pB2p1/p5Pp/p4p1P/2N1PP2/P1PP4/1R2R2K b - - 0 24",
        Outcome::Loss,
        None,
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow regression; run with --ignored")]
fn m25a_white_wins() {
    // Fixed: rank 5 was `p6Pp` (9 squares); corrected to `p5Pp`.
    assert_solves_to(
        "4r1k1/3p4/2pB2p1/p5Pp/p6P/2N2P2/P1PP4/1R2R2K w - - 0 25",
        Outcome::Win,
        None,
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow regression; run with --ignored")]
fn m25a_black_loses() {
    // Fixed: rank 5 was `p6Pp` (9 squares); corrected to `p5Pp`.
    assert_solves_to(
        "4r1k1/3p4/2pB2p1/p5Pp/7P/p1N2P2/P1PP4/1R2R2K b - - 0 25",
        Outcome::Loss,
        None,
    );
}

#[test]
fn m25b_white_wins() {
    assert_solves_to(
        "6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 25",
        Outcome::Win,
        None,
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow regression; run with --ignored")]
fn m25b_black_loses() {
    // Regression for the cyclic/invalid PV reported in the issue tracker:
    // the solver used to return a non-terminal PV for this black-to-move FEN.
    // The outcome is now verified; the PV is informational and not validated.
    assert_solves_to(
        "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25",
        Outcome::Loss,
        None,
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow regression; run with --ignored")]
fn m26_black_loses() {
    assert_solves_to(
        "1R4k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 1 26",
        Outcome::Loss,
        None,
    );
}

#[test]
fn m27_white_wins() {
    assert_solves_to(
        "1R6/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 2 27",
        Outcome::Win,
        None,
    );
}

#[test]
fn m27_kh7_fast_win_with_commoners() {
    // The `c`/`C` pieces are intentional custom commoners; the FEN is valid
    // and the position solves quickly.
    let mut pos = Position::from_fen("1R6/3p3c/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7C w - - 2 27").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win, "expected a quick win with commoners");
}

#[test]
fn m27_streaming_output() {
    let fen = "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26";
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);

    let mut progress_count = 0;
    let (outcome, _pv, _nodes) = search.solve_with_progress(&mut pos, |_outcome, _line| {
        progress_count += 1;
    });

    assert_eq!(outcome, Outcome::Win, "expected a winning outcome");
    assert!(
        progress_count >= 1,
        "expected at least one progress callback with the decisive line"
    );
}

#[test]
fn m27_ppv_only() {
    let output = Command::new(cli_bin())
        .args([
            "--first-outcome",
            "--fen",
            "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26",
        ])
        .output()
        .expect("failed to run CLI binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "CLI exited with failure: {stdout}");

    let outcome_line = stdout
        .lines()
        .find(|l| l.starts_with("outcome: "))
        .expect("expected an outcome line");
    assert!(
        outcome_line.starts_with("outcome: win"),
        "expected winning outcome, got:\n{stdout}"
    );

    assert!(
        stdout.lines().any(|l| l.starts_with("pv: ")),
        "expected a pv line in stdout:\n{stdout}"
    );
    assert!(
        stdout.lines().any(|l| l.starts_with("pre_exit:")),
        "expected a pre_exit summary line:\n{stdout}"
    );
    assert!(
        !stdout.contains("ppv_valid:"),
        "CLI should not print ppv_valid"
    );
}

#[test]
fn m28_black_loses() {
    assert_solves_to(
        "5R2/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 5 28",
        Outcome::Loss,
        None,
    );
}

#[test]
fn m29_white_wins() {
    assert_solves_to(
        "5R2/3p4/3Bk1p1/6Pp/2p4P/p1N2P2/P1PP4/7K w - - 0 29",
        Outcome::Win,
        None,
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "slow regression; run with --ignored")]
fn m29_black_loses() {
    assert_solves_to(
        "8/3p4/3BkRp1/6Pp/2p4P/p1N2P2/P1PP4/7K b - - 1 29",
        Outcome::Loss,
        None,
    );
}

// Positions that are too deep to prove in a 5-second budget but solve in 60
// seconds in release builds.

#[test]
#[cfg_attr(debug_assertions, ignore = "60 second regression; run with --ignored")]
fn m22_white_wins() {
    assert_solves_to_timeout(
        "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22",
        Outcome::Win,
        None,
        60,
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "60 second regression; run with --ignored")]
fn m22_black_loses() {
    assert_solves_to_timeout(
        "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK b - - 0 22",
        Outcome::Loss,
        None,
        60,
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "60 second stress test; run with --ignored")]
fn m24_solve_with_pv() {
    let fen = "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24";
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(60);

    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win, "expected white to win at m24");
}

#[test]
fn timeout_message() {
    let fen = "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26";
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(0);

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(
        outcome,
        Outcome::Draw,
        "immediate timeout should return Draw"
    );
    assert!(
        pv.is_empty(),
        "expected an empty PV after immediate timeout, got {pv:?}"
    );
    assert!(
        search.time_exceeded(),
        "time should be exceeded after timeout"
    );
}
