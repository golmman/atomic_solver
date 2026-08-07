mod common;

use std::process::Command;

use atomic_movegen::types::{Move, MoveList, Square};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use common::{assert_solves_to, assert_solves_to_timeout, cli_bin, solve_refined_moves};

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
    let (outcome, pv, _nodes) =
        solve_refined_moves("1R6/3p3c/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7C w - - 2 27");
    assert_eq!(outcome, Outcome::Win, "expected a quick win with commoners");
    assert_eq!(
        pv,
        vec![
            Move::make_move(Square::B8, Square::G8),
            Move::make_move(Square::C5, Square::C4),
            Move::make_move(Square::G8, Square::G6),
        ],
        "expected the documented fast-win PV"
    );
}

#[test]
fn m27_shortest_pv() {
    let (outcome, pv, _nodes) =
        solve_refined_moves("6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26");
    assert_eq!(outcome, Outcome::Win, "expected white to win at m27");
    assert_eq!(pv.len(), 7, "expected a 7-plies PV, got {pv:?}");
    assert_eq!(pv[0], Move::make_move(Square::B1, Square::B8));

    let mut pos =
        Position::from_fen("6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26").unwrap();
    pos.do_move(pv[0]);
    let mut legal = MoveList::new();
    pos.legal_moves(&mut legal);
    assert!(
        (0..legal.len()).any(|i| legal[i] == pv[1]),
        "second move should be a legal defender reply, got {pv:?}"
    );
}

#[test]
fn m27_streaming_output() {
    let fen = "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26";
    let mut pos = Position::from_fen(fen).unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);

    let mut progress_count = 0;
    let (outcome, pv, _nodes) = search.solve_with_progress(&mut pos, |_outcome, _line| {
        progress_count += 1;
    });

    assert_eq!(outcome, Outcome::Win, "expected a winning outcome");
    assert_eq!(pv.len(), 7, "expected a 7-plies PV, got {pv:?}");
    assert_eq!(pv[0], Move::make_move(Square::B1, Square::B8));

    pos.do_move(pv[0]);
    let mut legal = MoveList::new();
    pos.legal_moves(&mut legal);
    assert!(
        (0..legal.len()).any(|i| legal[i] == pv[1]),
        "second move should be a legal defender reply, got {pv:?}"
    );
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

    let lines: Vec<&str> = stdout.lines().collect();
    let outcome_line = lines
        .iter()
        .find(|l| l.starts_with("outcome: "))
        .expect("expected an outcome line");
    assert!(
        outcome_line.starts_with("outcome: win"),
        "expected winning outcome, got:\n{stdout}"
    );

    // With --first-outcome we expect the outcome line, one PV line, and the
    // pre-exit summary lines (no iterative refinement progress lines).
    assert!(
        lines.len() >= 2,
        "expected outcome + at least pv and pre_exit lines, got:\n{stdout}"
    );
    let pv_line = lines
        .iter()
        .find(|l| l.starts_with("pv: "))
        .expect("expected a pv line");
    assert!(
        pv_line.starts_with("pv: b1b8"),
        "expected PV to start with b1b8, got: {pv_line}"
    );
    let pre_exit = lines
        .iter()
        .find(|l| l.starts_with("pre_exit:"))
        .expect("expected a pre_exit summary line");
    assert!(
        pre_exit.contains("Complete"),
        "expected complete pre_exit, got: {pre_exit}"
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

    let (outcome, pv, _nodes) = search.solve(&mut pos);
    assert_eq!(outcome, Outcome::Win, "expected white to win at m24");
    assert!(
        pv.len() >= 2,
        "expected a PV with at least one attacker move and a defender reply, got {pv:?}"
    );

    let mut current = Position::from_fen(fen).unwrap();
    current.do_move(pv[0]);
    let mut legal = MoveList::new();
    current.legal_moves(&mut legal);
    assert!(
        (0..legal.len()).any(|i| legal[i] == pv[1]),
        "second move of m24 PV should be a legal defender reply, got {pv:?}"
    );
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
