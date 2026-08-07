//! Move ordering for the atomic-chess solver.

use atomic_movegen::attacks;
use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::types::{Color, Move, NO_PIECE, PieceType, Square};

pub trait MoveScorer {
    fn score(&self, board: &Board, m: Move, state: &StateInfo) -> i32;
}

pub struct StaticAtomicScorer;

const SCORE_WINNING_CAPTURE: i32 = 100_000_000;
const SCORE_PROMOTION: i32 = 1_000_000;
const SCORE_CAPTURE: i32 = 100_000;
const SCORE_THREAT_LAST: i32 = 10_000;
const SCORE_ATOMIC_CHECK: i32 = 9_000;
const SCORE_THREAT: i32 = 1_000;
const SCORE_BLAST: i32 = 500;
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
        let to_piece = board.piece_on(to);
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

        // 2. Promotion.
        if m.is_promotion() {
            return SCORE_PROMOTION + piece_value(m.promotion_type());
        }

        // 3. Capture by MVV-LVA.
        if is_capture {
            let victim = if m.is_en_passant() {
                PieceType::Pawn
            } else {
                to_piece.type_of().unwrap()
            };
            return SCORE_CAPTURE + piece_value(victim) * 10 - piece_value(from_pt);
        }

        let mut score = 0;

        // 4. Check-like threat: after moving, the piece attacks an opponent commoner.
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
            } else if state.them_commoners_count == 1 {
                let mut them_commoners = board.commoners(them);
                let enemy_king_sq = them_commoners.pop_lsb();
                let near_king = attacks::king_attacks(enemy_king_sq);
                if (attack_bb & near_king) != atomic_movegen::types::Bitboard::EMPTY {
                    score += SCORE_ATOMIC_CHECK;
                }
            }
        }

        // 5. Blast-threaten capture: capture blast zone is near an enemy commoner.
        {
            let blast_zone =
                attacks::king_attacks(to) | atomic_movegen::types::Bitboard::square_bb(to);
            let mut near = blast_zone;
            let mut b = blast_zone;
            while !b.is_empty() {
                let sq = b.pop_lsb();
                near = near | attacks::king_attacks(sq);
            }
            if (board.commoners(them) & near) != atomic_movegen::types::Bitboard::EMPTY {
                score += SCORE_BLAST;
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
        let f3e5 = find_move(&moves, "f3e5");
        let capture = scorer.score(&board, f3e5, &state);

        let f3d4 = find_move(&moves, "f3d4");
        let quiet = scorer.score(&board, f3d4, &state);
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
}
