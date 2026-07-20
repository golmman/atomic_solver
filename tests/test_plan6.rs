mod common;

use atomic_movegen::types::{Move, Square};
use atomic_solver::position::Outcome;
use common::solve_refined_moves;

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
        solve_refined_moves("4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K b - - 0 24").0,
        Outcome::Loss
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m25a_white_wins() {
    assert_eq!(
        solve_refined_moves("4r1k1/3p4/2pB2p1/6Pp/p6P/2N2P2/P1PP4/1R2R2K w - - 0 25").0,
        Outcome::Win
    );
}

#[test]
#[ignore = "exceeds 5s search limit"]
fn m25a_black_loses() {
    assert_eq!(
        solve_refined_moves("4r1k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R2R2K b - - 0 25").0,
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
    assert_eq!(pv[1], Move::make_move(Square::G8, Square::F7));
}

#[test]
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
