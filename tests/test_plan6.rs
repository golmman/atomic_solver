mod common;

use std::process::Command;

use atomic_movegen::types::{Move, MoveList, Square};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use common::{cli_bin, solve_refined_moves};

#[test]
fn black_root_report6_fen() {
    let (outcome, pv, _nodes) =
        solve_refined_moves("6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27");
    assert_eq!(outcome, Outcome::Loss, "expected black to lose");
    let first = pv.first().copied().unwrap();
    assert_eq!(first, Move::make_move(Square::F7, Square::E6));
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m19_white_wins() {
    assert_eq!(
        solve_refined_moves("4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19").0,
        Outcome::Win
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m20_white_wins() {
    assert_eq!(
        solve_refined_moves("4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20").0,
        Outcome::Win
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m20_black_loses() {
    assert_eq!(
        solve_refined_moves("4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/1R4RK b - - 5 20").0,
        Outcome::Loss
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m21_white_wins() {
    assert_eq!(
        solve_refined_moves("4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21").0,
        Outcome::Win
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m21_black_loses() {
    assert_eq!(
        solve_refined_moves("4r2k/3p4/2pB2p1/p4p1p/6PP/2N1PP2/P1PP4/1R4RK b - - 0 21").0,
        Outcome::Loss
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m22_white_wins() {
    assert_eq!(
        solve_refined_moves("4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22").0,
        Outcome::Win
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m22_black_loses() {
    assert_eq!(
        solve_refined_moves("4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK b - - 0 22").0,
        Outcome::Loss
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m23_white_wins() {
    assert_eq!(
        solve_refined_moves("4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23").0,
        Outcome::Win
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m23_black_loses() {
    assert_eq!(
        solve_refined_moves("4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K b - - 2 23").0,
        Outcome::Loss
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m24_white_wins() {
    assert_eq!(
        solve_refined_moves("4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24").0,
        Outcome::Win
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m24_black_loses() {
    assert_eq!(
        solve_refined_moves("4r1k1/3p4/2pB2p1/p6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K b - - 0 24").0,
        Outcome::Loss
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m25a_white_wins() {
    assert_eq!(
        solve_refined_moves("4r1k1/3p4/2pB2p1/p6Pp/p6P/2N2P2/P1PP4/1R2R2K w - - 0 25").0,
        Outcome::Win
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m25a_black_loses() {
    assert_eq!(
        solve_refined_moves("4r1k1/3p4/2pB2p1/p6Pp/7P/p1N2P2/P1PP4/1R2R2K b - - 0 25").0,
        Outcome::Loss
    );
}

#[test]
fn m25b_white_wins() {
    assert_eq!(
        solve_refined_moves("6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 25").0,
        Outcome::Win
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m25b_black_loses() {
    assert_eq!(
        solve_refined_moves("6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25").0,
        Outcome::Loss
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m26_black_loses() {
    assert_eq!(
        solve_refined_moves("1R4k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 1 26").0,
        Outcome::Loss
    );
}

#[test]
fn m27_white_wins() {
    assert_eq!(
        solve_refined_moves("1R6/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 2 27").0,
        Outcome::Win
    );
}

#[test]
fn m27_shortest_pv() {
    let (outcome, pv, _nodes) =
        solve_refined_moves("6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26");
    assert_eq!(outcome, Outcome::Win);
    assert_eq!(pv.len(), 7, "expected a 7-plies PV, got {pv:?}");
    assert_eq!(pv[0], Move::make_move(Square::B1, Square::B8));

    let mut pos = Position::from_fen("6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26")
        .expect("valid fen");
    pos.do_move(pv[0]);
    let mut legal = MoveList::new();
    pos.legal_moves(&mut legal);
    assert!(
        (0..legal.len()).any(|i| legal[i] == pv[1]),
        "second move should be a legal defender reply, got {pv:?}"
    );
}

#[test]
#[ignore = "invalid FEN contains non-standard pieces; needs FEN correction"]
fn m27_kh7_fast_win() {
    let (outcome, pv, _nodes) =
        solve_refined_moves("1R6/3p3c/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7C w - - 2 27");
    assert_eq!(outcome, Outcome::Win);
    assert_eq!(
        pv,
        vec![
            Move::make_move(Square::B8, Square::G8),
            Move::make_move(Square::C5, Square::C4),
            Move::make_move(Square::G8, Square::G6),
        ]
    );
}

#[test]
fn m27_black_loses() {
    assert_eq!(
        solve_refined_moves("6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27").0,
        Outcome::Loss
    );
}

#[test]
fn m28_white_wins() {
    assert_eq!(
        solve_refined_moves("6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28").0,
        Outcome::Win
    );
}

#[test]
fn m28_black_loses() {
    assert_eq!(
        solve_refined_moves("5R2/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 5 28").0,
        Outcome::Loss
    );
}

#[test]
fn m29_white_wins() {
    assert_eq!(
        solve_refined_moves("5R2/3p4/3Bk1p1/6Pp/2p4P/p1N2P2/P1PP4/7K w - - 0 29").0,
        Outcome::Win
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m29_black_loses() {
    assert_eq!(
        solve_refined_moves("8/3p4/3BkRp1/6Pp/2p4P/p1N2P2/P1PP4/7K b - - 1 29").0,
        Outcome::Loss
    );
}

#[test]
#[ignore = "60 second search"]
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
