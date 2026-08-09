//! Move ordering for the atomic-chess solver.
//!
//! This file is larger than 10 KiB because the `MoveScorer` trait, the static
//! scorer implementation, distance heuristics, and the unit-test matrix for
//! captures/promotions/quiet moves are kept together to avoid exposing the
//! scorer internals through extra modules.

use atomic_movegen::attacks;
use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::types::{Color, Move, NO_PIECE, PieceType, Square};

pub trait MoveScorer {
    fn score(&self, board: &Board, m: Move, state: &StateInfo) -> i32;
}

pub struct StaticAtomicScorer;

const SCORE_WINNING_CAPTURE: i32 = 100_000_000;
const SCORE_PROMOTION: i32 = 1_000_000;
const SCORE_CAPTURE: i32 = 5_000;
const SCORE_THREAT_LAST: i32 = 10_000;
const SCORE_THREAT: i32 = 1_000;
const SCORE_KAMIKAZE_LAST: i32 = 9_000;
const SCORE_KAMIKAZE: i32 = 3_000;
const CAPTURE_NET_SCALE: i32 = 10;
const SCORE_APPROACH: i32 = 100;
const SCORE_CENTER: i32 = 50;

fn piece_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Pawn => 100,
        PieceType::Knight => 320,
        PieceType::Bishop => 330,
        PieceType::Rook => 500,
        PieceType::Queen => 900,
        PieceType::Commoner => 20_000,
        _ => 0,
    }
}

/// Compute the net material destroyed by an atomic capture blast.
///
/// The capturing piece is always lost. Non-pawn pieces in the surrounding
/// 3x3 blast zone are also lost; pawns are immune to the surrounding blast.
/// The victim at ground zero is always lost. The origin square is excluded
/// because the moving piece is leaving it.
fn capture_net_value(board: &Board, m: Move) -> i32 {
    let from = m.from_sq();
    let to = m.to_sq();
    let moving_piece = board.piece_on(from);
    let moving_value = piece_value(moving_piece.type_of().unwrap());

    let victim_value = if m.is_en_passant() {
        piece_value(PieceType::Pawn)
    } else {
        piece_value(board.piece_on(to).type_of().unwrap())
    };

    let mut own_destroyed = moving_value;
    let mut enemy_destroyed = victim_value;

    let blast = attacks::king_attacks(to) & !board.pieces_pt(PieceType::Pawn);

    let mut b = blast;
    while !b.is_empty() {
        let sq = b.pop_lsb();
        if sq == from {
            continue;
        }
        let p = board.piece_on(sq);
        if p == NO_PIECE {
            continue;
        }
        let value = piece_value(p.type_of().unwrap());
        if p.color().unwrap() == board.side_to_move() {
            own_destroyed += value;
        } else {
            enemy_destroyed += value;
        }
    }

    enemy_destroyed - own_destroyed
}

fn file_rank_of(sq: Square) -> (i8, i8) {
    use atomic_movegen::types::{file_of, rank_of};
    (file_of(sq) as u8 as i8, rank_of(sq) as u8 as i8)
}

fn chebyshev(a: Square, b: Square) -> i8 {
    let (af, ar) = file_rank_of(a);
    let (bf, br) = file_rank_of(b);
    (af - bf).abs().max((ar - br).abs())
}

/// Precompute the nearest enemy commoner distance for every square.
///
/// If the opponent has no commoners, every entry is set to `i8::MAX`.
pub fn nearest_commoner_map(board: &Board, them: Color) -> [i8; 64] {
    let mut map = [i8::MAX; 64];
    let mut commoners = board.commoners(them);
    if commoners.is_empty() {
        return map;
    }
    while !commoners.is_empty() {
        let c = commoners.pop_lsb();
        for sq in 0..64 {
            let d = chebyshev(Square::from_u8(sq), c);
            if d < map[sq as usize] {
                map[sq as usize] = d;
            }
        }
    }
    map
}

fn attacks_from(
    pt: PieceType,
    color: Color,
    sq: Square,
    occupied: atomic_movegen::types::Bitboard,
) -> atomic_movegen::types::Bitboard {
    match pt {
        PieceType::Pawn => attacks::pawn_attacks(color, sq),
        PieceType::Knight => attacks::knight_attacks(sq),
        PieceType::Bishop => attacks::bishop_attacks(sq, occupied),
        PieceType::Rook => attacks::rook_attacks(sq, occupied),
        PieceType::Queen => attacks::queen_attacks(sq, occupied),
        PieceType::Commoner => attacks::king_attacks(sq),
        _ => atomic_movegen::types::Bitboard::EMPTY,
    }
}

impl StaticAtomicScorer {
    /// Score a move using a precomputed nearest-commoner distance map.
    ///
    /// This is the same logic as [`MoveScorer::score`] but avoids recomputing
    /// the enemy commoner distance for every `from`/`to` pair.
    pub fn score_with_map(
        &self,
        board: &Board,
        m: Move,
        state: &StateInfo,
        nearest: &[i8; 64],
    ) -> i32 {
        let from = m.from_sq();
        let to = m.to_sq();
        let from_piece = board.piece_on(from);
        if from_piece == NO_PIECE {
            return 0;
        }
        let from_pt = from_piece.type_of().unwrap();
        let us = board.side_to_move();
        let them = us.flip();

        let is_capture = board.is_capture(m);

        // 1. Winning capture: blast removes the opponent's last commoner.
        if is_capture {
            let blast_zone =
                attacks::king_attacks(to) | atomic_movegen::types::Bitboard::square_bb(to);
            let them_commoners = board.commoners(them);
            if them_commoners.count() == 1
                && (them_commoners & blast_zone) != atomic_movegen::types::Bitboard::EMPTY
            {
                return SCORE_WINNING_CAPTURE;
            }
        }

        // 2. Promotion (non-captures only; capture-promotions are evaluated by aSEE).
        if m.is_promotion() && !is_capture {
            return SCORE_PROMOTION + piece_value(m.promotion_type());
        }

        // 3. Capture by atomic static exchange evaluation (aSEE).
        if is_capture {
            let net = capture_net_value(board, m);
            return SCORE_CAPTURE + net * CAPTURE_NET_SCALE;
        }

        let mut score = 0;

        // 4. Direct commoner threat: after moving, the piece attacks an opponent commoner.
        {
            let from_bb = atomic_movegen::types::Bitboard::square_bb(from);
            let to_bb = atomic_movegen::types::Bitboard::square_bb(to);
            let new_occupied = (board.occupied() & !from_bb) | to_bb;
            let attack_bb = attacks_from(from_pt, us, to, new_occupied);
            if (attack_bb & board.commoners(them)) != atomic_movegen::types::Bitboard::EMPTY {
                if state.them_commoners_count == 1 {
                    score += SCORE_THREAT_LAST;
                } else {
                    score += SCORE_THREAT;
                }
            }
        }

        // 5. Kamikaze: landing adjacent to an enemy commoner creates a real blast threat.
        if (attacks::king_attacks(to) & board.commoners(them))
            != atomic_movegen::types::Bitboard::EMPTY
        {
            if state.them_commoners_count == 1 {
                score += SCORE_KAMIKAZE_LAST;
            } else {
                score += SCORE_KAMIKAZE;
            }
        }

        // 6. Centralizing / attacking moves.
        let from_dist = nearest[from as usize];
        let to_dist = nearest[to as usize];
        if from_dist < i8::MAX && to_dist < i8::MAX && to_dist < from_dist {
            score += SCORE_APPROACH + i32::from(from_dist - to_dist) * 10;
        }

        let (f, r) = file_rank_of(to);
        let center = 3 - (f - 3).abs().max(r - 3).abs();
        if center > 0 {
            score += SCORE_CENTER + i32::from(center) * 10;
        }

        score
    }
}

impl MoveScorer for StaticAtomicScorer {
    fn score(&self, board: &Board, m: Move, state: &StateInfo) -> i32 {
        let from = m.from_sq();
        let from_piece = board.piece_on(from);
        if from_piece == NO_PIECE {
            return 0;
        }
        let us = board.side_to_move();
        let them = us.flip();
        let nearest = nearest_commoner_map(board, them);
        self.score_with_map(board, m, state, &nearest)
    }
}

#[cfg(test)]
mod tests {
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
}
