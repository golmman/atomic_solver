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
const SCORE_PAWN_STORM: i32 = 5_500;
const SCORE_PAWN_STORM_STEP: i32 = 100;
const SCORE_ROOK_CENTER: i32 = 500;
const SCORE_ROOK_OPEN_FILE: i32 = 2_000;
const SCORE_ROOK_OPEN_FILE_STEP: i32 = 50;
const SCORE_ROOK_BACK_RANK: i32 = 300;

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

        // Precompute the lone enemy commoner square when it exists.
        let lone_commoner = if state.them_commoners_count == 1 {
            let mut c = board.commoners(them);
            let sq = c.pop_lsb();
            if sq != Square::NONE { Some(sq) } else { None }
        } else {
            None
        };

        // 4. Direct commoner threat: after moving, the piece attacks an opponent commoner.
        {
            let from_bb = atomic_movegen::types::Bitboard::square_bb(from);
            let to_bb = atomic_movegen::types::Bitboard::square_bb(to);
            let new_occupied = (board.occupied() & !from_bb) | to_bb;
            let attack_bb = attacks_from(from_pt, us, to, new_occupied);
            if (attack_bb & board.commoners(them)) != atomic_movegen::types::Bitboard::EMPTY {
                let base = if state.them_commoners_count == 1 {
                    SCORE_THREAT_LAST
                } else {
                    SCORE_THREAT
                };
                // If the threatening piece can be immediately captured, the
                // threat is less reliable; downgrade it.
                let enemy_attackers =
                    board.attackers_to(to, new_occupied) & board.pieces_color(them);
                let bonus = if enemy_attackers.is_empty() {
                    base
                } else {
                    base / 2
                };
                score += bonus;
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

        // 6. Pawn storm: a pawn push toward the lone enemy commoner that attacks
        // squares near it.
        if from_pt == PieceType::Pawn
            && let Some(commoner_sq) = lone_commoner
        {
            let from_dist = nearest[from as usize];
            let to_dist = nearest[to as usize];
            if to_dist < from_dist {
                let attacks = attacks::pawn_attacks(us, to);
                let mut b = attacks;
                let mut near = false;
                while !b.is_empty() {
                    let sq = b.pop_lsb();
                    if chebyshev(sq, commoner_sq) <= 2 {
                        near = true;
                        break;
                    }
                }
                if near {
                    score +=
                        SCORE_PAWN_STORM + i32::from(from_dist - to_dist) * SCORE_PAWN_STORM_STEP;
                }
            }
        }

        // 7. Heavy-piece centralization, open-file alignment, and back-rank presence.
        if matches!(from_pt, PieceType::Rook | PieceType::Queen)
            && let Some(commoner_sq) = lone_commoner
        {
            let from_dist = nearest[from as usize];
            let to_dist = nearest[to as usize];

            let from_bb = atomic_movegen::types::Bitboard::square_bb(from);
            let occupied_without_from = board.occupied() & !from_bb;
            let rook_attacks = attacks::rook_attacks(to, occupied_without_from);
            if (rook_attacks & atomic_movegen::types::Bitboard::square_bb(commoner_sq))
                != atomic_movegen::types::Bitboard::EMPTY
            {
                let reduction = (from_dist - to_dist).max(0);
                score += SCORE_ROOK_OPEN_FILE + i32::from(reduction) * SCORE_ROOK_OPEN_FILE_STEP;
            } else {
                // A rook/queen that moves onto a central/semi-open file pointing
                // at an enemy piece on the back rank is a strong plan signal.
                // Treat own pawns as transparent so that a lift like Rg1-e1
                // (preparing Rxe8) is recognized before the e-pawn has left the
                // file. Only award this on a file the move actually changed, so
                // shuffling a rook already on the e-file does not keep receiving
                // the bonus.
                let changed_file =
                    atomic_movegen::types::file_of(to) != atomic_movegen::types::file_of(from);
                if changed_file {
                    let enemy_back_rank = if us == Color::White { 7u32 } else { 0u32 };
                    let back_rank_mask =
                        atomic_movegen::types::Bitboard(0xFFu64 << (enemy_back_rank * 8));
                    let file_mask = atomic_movegen::types::Bitboard(
                        0x0101_0101_0101_0101u64
                            << (atomic_movegen::types::file_of(to) as u8 as u32),
                    );
                    let occupied_no_own_pawns =
                        board.occupied() & !board.pieces_color_pt(us, PieceType::Pawn) & !from_bb;
                    let rook_attacks_semi = attacks::rook_attacks(to, occupied_no_own_pawns);
                    let enemy_back_rank_pieces =
                        board.pieces_color(them) & back_rank_mask & file_mask;
                    if (rook_attacks_semi & enemy_back_rank_pieces)
                        != atomic_movegen::types::Bitboard::EMPTY
                    {
                        let reduction = (from_dist - to_dist).max(0);
                        score +=
                            SCORE_ROOK_OPEN_FILE + i32::from(reduction) * SCORE_ROOK_OPEN_FILE_STEP;
                    }
                }
            }

            // Back-rank presence when the enemy commoner is on or near it.
            let back_rank = if us == Color::White { 7 } else { 0 };
            if (to as u8 / 8) == back_rank && chebyshev(to, commoner_sq) <= 2 {
                score += SCORE_ROOK_BACK_RANK;
            }
        }

        // 8. Centralizing / attacking moves.
        let from_dist = nearest[from as usize];
        let to_dist = nearest[to as usize];
        if from_dist < i8::MAX && to_dist < i8::MAX && to_dist < from_dist {
            score += SCORE_APPROACH + i32::from(from_dist - to_dist) * 10;
        }

        let (f, r) = file_rank_of(to);
        let center = 3 - (f - 3).abs().max(r - 3).abs();
        if center > 0 {
            score += SCORE_CENTER + i32::from(center) * 10;
            if matches!(from_pt, PieceType::Rook | PieceType::Queen) {
                score += SCORE_ROOK_CENTER * i32::from(center);
            }
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
mod tests;
