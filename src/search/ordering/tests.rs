use super::*;
use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::movegen::generate_legal;
use atomic_movegen::types::MoveList;

fn legal_moves_and_state(fen: &str) -> (Board, StateInfo, MoveList) {
    let board = Board::from_fen(fen).unwrap();
    let mut state = StateInfo::new();
    board.populate_state(&mut state);
    let mut moves = MoveList::new();
    generate_legal(&board, &mut moves);
    (board, state, moves)
}

fn find_move(moves: &MoveList, uci: &str) -> Move {
    for i in 0..moves.len() {
        if moves[i].to_uci() == uci {
            return moves[i];
        }
    }
    panic!("move {uci} not found");
}

#[test]
fn winning_capture_scores_highest() {
    // White queen on f7 captures the e7 pawn; the blast removes the lone
    // black commoner on d7.
    let (board, state, moves) =
        legal_moves_and_state("rnbq1bnr/pppkpQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR w KQ - 2 5");
    let f7e7 = find_move(&moves, "f7e7");
    let scorer = StaticAtomicScorer;
    assert_eq!(scorer.score(&board, f7e7, &state), SCORE_WINNING_CAPTURE);
}

#[test]
fn promotion_scores_above_threat_and_center() {
    let (board, state, moves) = legal_moves_and_state("4k3/1P6/8/8/8/8/8/4K3 w - - 0 1");
    let scorer = StaticAtomicScorer;
    let b7b8q = find_move(&moves, "b7b8q");
    let promotion = scorer.score(&board, b7b8q, &state);

    // A quiet king move should be scored far below a promotion.
    let e1d1 = moves
        .as_slice()
        .iter()
        .find(|m| m.to_uci() == "e1d1")
        .copied()
        .unwrap();
    let quiet = scorer.score(&board, e1d1, &state);
    assert!(
        promotion > quiet,
        "promotion should be preferred to a quiet king move"
    );
}

#[test]
fn capture_scores_above_quiet_moves() {
    // White knight on f3 can capture e5 or move to a quiet square.
    let (board, state, moves) =
        legal_moves_and_state("rnbqkbnr/pppp1ppp/8/4p3/8/5N2/PPPPPPPP/RNBQKB1R w KQkq - 0 3");
    let scorer = StaticAtomicScorer;
    let capture_move = find_move(&moves, "f3e5");
    let capture = scorer.score(&board, capture_move, &state);

    let quiet_move = find_move(&moves, "f3d4");
    let quiet = scorer.score(&board, quiet_move, &state);
    assert!(
        capture > quiet,
        "capture should score above quiet development"
    );
}

#[test]
fn score_is_deterministic() {
    let (board, state, moves) =
        legal_moves_and_state("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let scorer = StaticAtomicScorer;
    for i in 0..moves.len() {
        let a = scorer.score(&board, moves[i], &state);
        let b = scorer.score(&board, moves[i], &state);
        assert_eq!(a, b, "score should be deterministic");
    }
}

#[test]
fn score_with_no_commoners_is_max_distance() {
    let board = Board::from_fen("8/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let map = nearest_commoner_map(&board, Color::Black);
    assert!(map.iter().all(|&d| d == i8::MAX));
}

#[test]
fn kamikaze_landing_adjacent_to_lone_commoner() {
    // White knight c2 -> e3 lands next to the black commoner on e4 but does
    // not attack it. A non-kamikaze knight jump should score lower.
    let (board, state, moves) = legal_moves_and_state("8/8/8/8/4k3/8/2N5/4K3 w - - 0 1");
    let scorer = StaticAtomicScorer;

    let kamikaze = scorer.score(&board, find_move(&moves, "c2e3"), &state);
    let other = scorer.score(&board, find_move(&moves, "c2a3"), &state);
    assert!(
        kamikaze > other,
        "kamikaze move c2e3 should score above a non-kamikaze jump"
    );
}

#[test]
fn losing_capture_scores_below_direct_commoner_threat() {
    // White queen capturing the e5 pawn loses the queen for a pawn. A quiet
    // bishop move to c6 attacks the black commoner on e8 and should score higher.
    let (board, state, moves) = legal_moves_and_state("4k3/8/8/1B2p3/8/8/4Q3/4K3 w - - 0 1");
    let scorer = StaticAtomicScorer;

    let capture = scorer.score(&board, find_move(&moves, "e2e5"), &state);
    let threat = scorer.score(&board, find_move(&moves, "b5c6"), &state);
    assert!(
        threat > capture,
        "direct commoner threat should score above a losing capture"
    );
}

#[test]
fn capture_with_blasted_rook_scores_higher() {
    // A queen capture on e5 that also destroys the f5 rook should score
    // higher than a capture that only takes a pawn.
    let (board, state, moves) = legal_moves_and_state("4k3/8/8/2p1pr2/3Q4/8/8/4K3 w - - 0 1");
    let scorer = StaticAtomicScorer;

    let rook_blast = scorer.score(&board, find_move(&moves, "d4e5"), &state);
    let pawn_only = scorer.score(&board, find_move(&moves, "d4c5"), &state);
    assert!(
        rook_blast > pawn_only,
        "capture that also blasts a rook should score higher"
    );
}

#[test]
fn capture_promotion_is_not_scored_as_promotion() {
    // Pawn a7xb8 with promotion should be evaluated by aSEE, not by the
    // promotion bonus, because the promoted piece is destroyed in the blast.
    let (board, state, moves) = legal_moves_and_state("1n2k3/P7/8/8/8/8/8/4K3 w - - 0 1");
    let scorer = StaticAtomicScorer;

    let capture_promo = find_move(&moves, "a7b8q");
    let non_capture_promo = find_move(&moves, "a7a8q");

    let capture_score = scorer.score(&board, capture_promo, &state);
    let promo_score = scorer.score(&board, non_capture_promo, &state);

    assert!(
        capture_score < SCORE_PROMOTION,
        "capture-promotion should not receive the promotion bonus"
    );
    assert!(
        promo_score > capture_score,
        "non-capture promotion should score above capture-promotion here"
    );
}

#[test]
fn m22_g4g5_scores_above_d6e5() {
    // The key pawn-storm push should outrank the unsupported bishop probe.
    let (board, state, moves) =
        legal_moves_and_state("4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22");
    let scorer = StaticAtomicScorer;

    let pawn_storm = scorer.score(&board, find_move(&moves, "g4g5"), &state);
    let bishop_probe = scorer.score(&board, find_move(&moves, "d6e5"), &state);
    assert!(
        pawn_storm > bishop_probe,
        "g4g5 pawn storm should score above d6e5 bishop probe"
    );
}

#[test]
fn m22_rg1e1_scores_above_quiet_pawn_moves() {
    // The rook lift to the central e-file should be preferred to quiet
    // pawn shuffling on the queenside.
    let (board, state, moves) =
        legal_moves_and_state("4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22");
    let scorer = StaticAtomicScorer;

    let rook_lift = scorer.score(&board, find_move(&moves, "g1e1"), &state);
    let quiet_pawn = scorer.score(&board, find_move(&moves, "a2a3"), &state);
    assert!(
        rook_lift > quiet_pawn,
        "Rg1e1 rook lift should score above quiet a-pawn push"
    );
}

#[test]
fn m22_pawn_storm_does_not_overvalue_distant_pawn_pushes() {
    // Quiet a-pawn pushes should not receive the pawn-storm bonus because
    // their attacks are far from the lone enemy commoner on h8.
    let (board, state, moves) =
        legal_moves_and_state("4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22");
    let scorer = StaticAtomicScorer;

    let a2a3 = scorer.score(&board, find_move(&moves, "a2a3"), &state);
    let a2a4 = scorer.score(&board, find_move(&moves, "a2a4"), &state);
    assert_eq!(a2a3, 0, "a2a3 should not get a pawn-storm bonus");
    assert_eq!(a2a4, 0, "a2a4 should not get a pawn-storm bonus");
}

#[test]
fn pawn_storm_is_lower_at_and_node() {
    // The key pawn-storm push is valued less highly from the defender's
    // perspective (AND node) than from the attacker's (OR node).
    let (board, state, moves) =
        legal_moves_and_state("4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22");
    let nearest = nearest_commoner_map(&board, board.side_to_move().flip());

    let g4g5 = find_move(&moves, "g4g5");
    let or = StaticAtomicScorer.score_with_map(&board, g4g5, &state, &nearest, true);
    let and = StaticAtomicScorer.score_with_map(&board, g4g5, &state, &nearest, false);
    assert!(
        and < or,
        "AND-node pawn storm should score below OR-node pawn storm"
    );
}

#[test]
fn direct_commoner_threat_stays_high_at_and_node() {
    // A direct attack on the enemy commoner is a genuine counter-threat even
    // for the defender, so the AND profile should not reduce it.
    let (board, state, moves) =
        legal_moves_and_state("4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22");
    let nearest = nearest_commoner_map(&board, board.side_to_move().flip());

    let d6e5 = find_move(&moves, "d6e5");
    let or = StaticAtomicScorer.score_with_map(&board, d6e5, &state, &nearest, true);
    let and = StaticAtomicScorer.score_with_map(&board, d6e5, &state, &nearest, false);
    assert!(
        and > 0,
        "direct commoner threat should stay positive at an AND node"
    );
    assert!(
        and >= or / 2,
        "direct commoner threat should not collapse at an AND node"
    );
}

#[test]
fn and_profile_shrinks_gap_between_pawn_storm_and_quiet_move() {
    // At an AND node the speculative pawn storm bonus is reduced, so the
    // distance between a pawn-storm push and a quiet centralizing move shrinks
    // even though the pawn storm can still be useful.
    let (board, state, moves) =
        legal_moves_and_state("4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22");
    let nearest = nearest_commoner_map(&board, board.side_to_move().flip());

    let g4g5 = find_move(&moves, "g4g5");
    let e3e4 = find_move(&moves, "e3e4");
    let storm_or = StaticAtomicScorer.score_with_map(&board, g4g5, &state, &nearest, true);
    let quiet_or = StaticAtomicScorer.score_with_map(&board, e3e4, &state, &nearest, true);
    let storm_and = StaticAtomicScorer.score_with_map(&board, g4g5, &state, &nearest, false);
    let quiet_and = StaticAtomicScorer.score_with_map(&board, e3e4, &state, &nearest, false);
    let gap_or = storm_or - quiet_or;
    let gap_and = storm_and - quiet_and;
    assert!(
        gap_and < gap_or,
        "AND profile should reduce the gap between pawn storm and quiet central move"
    );
}
