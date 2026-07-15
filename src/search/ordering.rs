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

fn nearest_commoner_dist(board: &Board, them: Color, sq: Square) -> Option<i8> {
    let mut commoners = board.commoners(them);
    if commoners.is_empty() {
        return None;
    }
    let mut best = i8::MAX;
    while !commoners.is_empty() {
        let c = commoners.pop_lsb();
        let d = chebyshev(sq, c);
        if d < best {
            best = d;
        }
    }
    Some(best)
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

impl MoveScorer for StaticAtomicScorer {
    fn score(&self, board: &Board, m: Move, state: &StateInfo) -> i32 {
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
        if let Some(from_dist) = nearest_commoner_dist(board, them, from)
            && let Some(to_dist) = nearest_commoner_dist(board, them, to)
            && to_dist < from_dist
        {
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
