mod common;

use atomic_movegen::types::{Move, Square};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use common::assert_solves_to;

/// A rook alone cannot force a win against a lone king that has a 2x2 safe area.
/// This position can produce reversible checking cycles, so the solver must not
/// claim a win from the cycle.
#[ignore = "slow: cyclic GHI regression; run with -- --include-ignored"]
#[test]
fn rook_alone_does_not_claim_win_against_safe_king() {
    assert_solves_to("8/8/8/8/2k5/8/8/4KR2 w - - 0 1", Outcome::Draw, None);
}

/// A reversible king/rook shuffle returns the same board with a different
/// rule50 counter. The repetition key must stay equal while the full hash
/// changes, and solving the repeated board must still not be declared a win.
#[ignore = "slow: cyclic GHI regression; run with -- --include-ignored"]
#[test]
fn reversible_cycle_keeps_repetition_key_and_stays_draw() {
    let mut pos = Position::from_fen("8/8/8/8/2k5/8/8/4KR2 w - - 0 1").unwrap();
    let start_rep = pos.repetition_key();
    let start_hash = pos.hash();

    // White rook shuffles f1-g1-f1, black king shuffles c4-b4-c4.
    let rg = Move::make_move(Square::F1, Square::G1);
    let kb = Move::make_move(Square::C4, Square::B4);
    let gr = Move::make_move(Square::G1, Square::F1);
    let bc = Move::make_move(Square::B4, Square::C4);

    pos.do_move(rg);
    pos.do_move(kb);
    pos.do_move(gr);
    pos.do_move(bc);

    assert_eq!(
        pos.repetition_key(),
        start_rep,
        "repetition key must ignore the halfmove clock"
    );
    assert_ne!(pos.hash(), start_hash, "full hash must include rule50");

    let mut search = Search::new(64);
    search.set_timeout(5);
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    assert_ne!(
        outcome,
        Outcome::Win,
        "repeated board should not be declared a win"
    );
}

/// Plan9 two-step soundness scenario: solve → reset → solve in one process.
/// The second `solve` routes through `begin_run`, which clears the per-search
/// repetition cache and resets the run state; the solver must re-derive the
/// same draw from scratch. Neither solve may ever yield `Outcome::Win`
/// (research_repetition_cache.md §4 residual risk: a second solve on the same
/// instance must not flip the result).
#[ignore = "slow: cyclic GHI regression; run with -- --include-ignored"]
#[test]
fn solve_reset_solve_never_claims_win() {
    let mut pos = Position::from_fen("8/8/8/8/2k5/8/8/4KR2 w - - 0 1").unwrap();
    let mut search = Search::new(64);
    search.set_timeout(5);

    let (first, _, _) = search.solve(&mut pos);
    assert_ne!(
        first,
        Outcome::Win,
        "first solve of the cyclic position must not claim a win"
    );

    let (second, _, _) = search.solve(&mut pos);
    assert_ne!(
        second,
        Outcome::Win,
        "second solve after reset must not claim a win"
    );
    assert_eq!(
        first, second,
        "both solves must agree on the outcome of the cyclic position"
    );
}
