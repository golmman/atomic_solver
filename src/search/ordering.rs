//! Move ordering for the atomic-chess solver.
//!
//! This file is larger than 10 KiB because the `MoveScorer` trait, the static
//! scorer implementation, distance heuristics, and the unit-test matrix for
//! captures/promotions/quiet moves are kept together to avoid exposing the
//! scorer internals through extra modules. The configurable scorer parameters
//! live in `params.rs` to keep this file focused on scoring logic.

use atomic_movegen::attacks;
use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::types::{Color, Move, NO_PIECE, PieceType, Square};

mod context;
mod params;
pub(crate) use context::ScoreContext;
pub use context::nearest_commoner_map;
use context::{chebyshev, queen_rays};
pub use params::{PieceValues, ScorerParams, ScorerParamsError};

pub trait MoveScorer {
    fn score(&self, board: &Board, m: Move, state: &StateInfo) -> i32;
}

#[derive(Clone)]
pub struct StaticAtomicScorer {
    params: ScorerParams,
}

impl StaticAtomicScorer {
    /// Create a scorer with the compiled-in default parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            params: ScorerParams::default(),
        }
    }

    /// Create a scorer from an externally loaded parameter set.
    #[must_use]
    pub fn from_params(params: ScorerParams) -> Self {
        Self { params }
    }

    /// Borrow the current parameters.
    #[must_use]
    pub fn params(&self) -> &ScorerParams {
        &self.params
    }
}

impl Default for StaticAtomicScorer {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the net material destroyed by an atomic capture blast.
///
/// The capturing piece is always lost. Non-pawn pieces in the surrounding
/// 3x3 blast zone are also lost; pawns are immune to the surrounding blast.
/// The victim at ground zero is always lost. The origin square is excluded
/// because the moving piece is leaving it.
///
/// Bitboard implementation: the blast zone is intersected with the per-type
/// piece bitboards and the value sum accumulated by popcount instead of a
/// per-square `piece_on` walk. Identical scores, verified by the exact-score
/// pins and the drift gate (the per-square additions are commutative, so the
/// `i32` totals match exactly).
fn capture_net_value(scorer: &StaticAtomicScorer, board: &Board, m: Move) -> i32 {
    let params = &scorer.params;
    let from = m.from_sq();
    let to = m.to_sq();
    let moving_piece = board.piece_on(from);
    let moving_value = params.piece_value(moving_piece.type_of().unwrap());

    let victim_value = if m.is_en_passant() {
        params.piece_value(PieceType::Pawn)
    } else {
        params.piece_value(board.piece_on(to).type_of().unwrap())
    };

    let mut own_destroyed = moving_value;
    let mut enemy_destroyed = victim_value;

    // Blast zone minus pawns and the origin square (the mover left `from`,
    // and pawns are immune to the surrounding blast).
    let blast = attacks::king_attacks(to)
        & !board.pieces_pt(PieceType::Pawn)
        & !atomic_movegen::types::Bitboard::square_bb(from);
    let us = board.side_to_move();

    for pt in [
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
        PieceType::Commoner,
    ] {
        let value = params.piece_value(pt);
        own_destroyed += (blast & board.pieces_color_pt(us, pt)).count() as i32 * value;
        enemy_destroyed += (blast & board.pieces_color_pt(us.flip(), pt)).count() as i32 * value;
    }

    enemy_destroyed - own_destroyed
}

fn file_rank_of(sq: Square) -> (i8, i8) {
    use atomic_movegen::types::{file_of, rank_of};
    (file_of(sq) as u8 as i8, rank_of(sq) as u8 as i8)
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
    ///
    /// `is_or_node` selects the scoring profile: OR nodes (attacker) use the
    /// full static bonuses, while AND nodes (defender) scale down speculative
    /// attacker-only bonuses such as pawn storms and rook lifts.
    #[must_use]
    pub fn score_with_map(
        &self,
        board: &Board,
        m: Move,
        state: &StateInfo,
        nearest: &[i8; 64],
        is_or_node: bool,
    ) -> i32 {
        let ctx = ScoreContext::build(board, state, *nearest);
        self.score_with_context(board, m, &ctx, is_or_node)
    }

    /// Score a move against an already-built [`ScoreContext`].
    ///
    /// Identical to [`StaticAtomicScorer::score_with_map`] but the per-node
    /// invariants (enemy commoners, lone-commoner square, back-rank mask,
    /// nearest map) are read from the context instead of being recomputed
    /// per move. Integer arithmetic and evaluation order per move are
    /// unchanged.
    pub(crate) fn score_with_context(
        &self,
        board: &Board,
        m: Move,
        ctx: &ScoreContext,
        is_or_node: bool,
    ) -> i32 {
        let p = &self.params;
        let from = m.from_sq();
        let to = m.to_sq();
        let from_piece = board.piece_on(from);
        if from_piece == NO_PIECE {
            return 0;
        }
        let from_pt = from_piece.type_of().unwrap();
        let us = ctx.us;
        let them = ctx.them;

        let is_capture = board.is_capture(m);

        // 1. Winning capture: blast removes the opponent's last commoner.
        if is_capture {
            let blast_zone =
                attacks::king_attacks(to) | atomic_movegen::types::Bitboard::square_bb(to);
            if ctx.them_commoners_count == 1
                && (ctx.them_commoners & blast_zone) != atomic_movegen::types::Bitboard::EMPTY
            {
                return p.score_winning_capture;
            }
        }

        // 2. Promotion (non-captures only; capture-promotions are evaluated by aSEE).
        if m.is_promotion() && !is_capture {
            return p.score_promotion + p.piece_value(m.promotion_type());
        }

        // 3. Capture by atomic static exchange evaluation (aSEE).
        if is_capture {
            let net = capture_net_value(self, board, m);
            return p.score_capture + net * p.capture_net_scale;
        }

        let mut score = 0;

        // Node-type-aware scaling. OR nodes (attacker) keep the full bonuses;
        // AND nodes (defender) scale down speculative attacker-only bonuses.
        let (pawn_storm, pawn_storm_step) = if is_or_node {
            (p.score_pawn_storm, p.score_pawn_storm_step)
        } else {
            (
                p.score_pawn_storm * p.and_pawn_storm_scale / 100,
                p.score_pawn_storm_step * p.and_pawn_storm_scale / 100,
            )
        };
        let (rook_open_file, rook_open_file_step, rook_back_rank) = if is_or_node {
            (
                p.score_rook_open_file,
                p.score_rook_open_file_step,
                p.score_rook_back_rank,
            )
        } else {
            (
                p.score_rook_open_file * p.and_rook_attack_scale / 100,
                p.score_rook_open_file_step * p.and_rook_attack_scale / 100,
                p.score_rook_back_rank * p.and_rook_attack_scale / 100,
            )
        };
        let (approach, approach_step, center, center_step, rook_center) = if is_or_node {
            (
                p.score_approach,
                p.score_approach_step,
                p.score_center,
                p.score_center_step,
                p.score_rook_center,
            )
        } else {
            (
                p.score_approach * p.and_approach_scale / 100,
                p.score_approach_step * p.and_approach_scale / 100,
                p.score_center * p.and_approach_scale / 100,
                p.score_center_step * p.and_approach_scale / 100,
                p.score_rook_center * p.and_approach_scale / 100,
            )
        };

        // The lone enemy commoner square was precomputed in the context.
        let lone_commoner = ctx.lone_commoner;

        // 4. Direct commoner threat: after moving, the piece attacks an opponent commoner.
        {
            // Occupancy-independent movers (pawn/knight/commoner) attack from
            // static tables, so the occupancy rewrite and dispatch are skipped;
            // sliders keep the occupied-board query. For sliders the attack
            // set is a subset of the queen rays through `to`, so when no enemy
            // commoner is aligned the scan is provably skippable (exact
            // filter, not a heuristic). Identical scores, verified by the
            // exact-score pins and the drift gate.
            let attack_bb = match from_pt {
                PieceType::Pawn => attacks::pawn_attacks(us, to),
                PieceType::Knight => attacks::knight_attacks(to),
                PieceType::Commoner => attacks::king_attacks(to),
                _ => {
                    if (queen_rays(to) & ctx.them_commoners)
                        == atomic_movegen::types::Bitboard::EMPTY
                    {
                        atomic_movegen::types::Bitboard::EMPTY
                    } else {
                        let from_bb = atomic_movegen::types::Bitboard::square_bb(from);
                        let to_bb = atomic_movegen::types::Bitboard::square_bb(to);
                        let new_occupied = (board.occupied() & !from_bb) | to_bb;
                        attacks_from(from_pt, us, to, new_occupied)
                    }
                }
            };
            if (attack_bb & ctx.them_commoners) != atomic_movegen::types::Bitboard::EMPTY {
                let base = if ctx.them_commoners_count == 1 {
                    p.score_threat_last
                } else {
                    p.score_threat
                };
                // If the threatening piece can be immediately captured, the
                // threat is less reliable; downgrade it.
                let from_bb = atomic_movegen::types::Bitboard::square_bb(from);
                let to_bb = atomic_movegen::types::Bitboard::square_bb(to);
                let new_occupied = (board.occupied() & !from_bb) | to_bb;
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
        if (attacks::king_attacks(to) & ctx.them_commoners)
            != atomic_movegen::types::Bitboard::EMPTY
        {
            if ctx.them_commoners_count == 1 {
                score += p.score_kamikaze_last;
            } else {
                score += p.score_kamikaze;
            }
        }

        // 6. Pawn storm: a pawn push toward the lone enemy commoner that attacks
        // squares near it.
        if from_pt == PieceType::Pawn
            && let Some(commoner_sq) = lone_commoner
        {
            let nearest = &ctx.nearest;
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
                    score += pawn_storm + i32::from(from_dist - to_dist) * pawn_storm_step;
                }
            }
        }

        // 7. Heavy-piece centralization, open-file alignment, and back-rank presence.
        if matches!(from_pt, PieceType::Rook | PieceType::Queen)
            && let Some(commoner_sq) = lone_commoner
        {
            let nearest = &ctx.nearest;
            let from_dist = nearest[from as usize];
            let to_dist = nearest[to as usize];

            // Exact pre-filter: a rook attacks only along its file and rank,
            // so `rook_attacks(to, _)` can contain `commoner_sq` only if the
            // two squares are aligned. Skips the sliding scan for the ~98% of
            // entries that are not aligned. Identical scores, verified by the
            // exact-score pins and the drift gate.
            let aligned = atomic_movegen::types::file_of(to)
                == atomic_movegen::types::file_of(commoner_sq)
                || atomic_movegen::types::rank_of(to)
                    == atomic_movegen::types::rank_of(commoner_sq);
            let mut first_hit = false;
            if aligned {
                let from_bb = atomic_movegen::types::Bitboard::square_bb(from);
                let occupied_without_from = board.occupied() & !from_bb;
                let rook_attacks = attacks::rook_attacks(to, occupied_without_from);
                first_hit = (rook_attacks
                    & atomic_movegen::types::Bitboard::square_bb(commoner_sq))
                    != atomic_movegen::types::Bitboard::EMPTY;
            }
            if first_hit {
                let reduction = (from_dist - to_dist).max(0);
                score += rook_open_file + i32::from(reduction) * rook_open_file_step;
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
                    let back_rank_mask = ctx.back_rank_mask;
                    let file_mask = atomic_movegen::types::Bitboard(
                        0x0101_0101_0101_0101u64
                            << u32::from(atomic_movegen::types::file_of(to) as u8),
                    );
                    // Exact pre-filter: without an enemy piece on this file's
                    // back rank the scan below cannot hit.
                    let enemy_back_rank_pieces =
                        board.pieces_color(them) & back_rank_mask & file_mask;
                    if !enemy_back_rank_pieces.is_empty() {
                        let from_bb = atomic_movegen::types::Bitboard::square_bb(from);
                        let occupied_no_own_pawns = board.occupied()
                            & !board.pieces_color_pt(us, PieceType::Pawn)
                            & !from_bb;
                        let rook_attacks_semi = attacks::rook_attacks(to, occupied_no_own_pawns);
                        if (rook_attacks_semi & enemy_back_rank_pieces)
                            != atomic_movegen::types::Bitboard::EMPTY
                        {
                            let reduction = (from_dist - to_dist).max(0);
                            score += rook_open_file + i32::from(reduction) * rook_open_file_step;
                        }
                    }
                }
            }

            // Back-rank presence when the enemy commoner is on or near it.
            if u32::from(to as u8 / 8) == ctx.enemy_back_rank && chebyshev(to, commoner_sq) <= 2 {
                score += rook_back_rank;
            }
        }

        // 8. Centralizing / attacking moves.
        let from_dist = ctx.nearest[from as usize];
        let to_dist = ctx.nearest[to as usize];
        if from_dist < i8::MAX && to_dist < i8::MAX && to_dist < from_dist {
            score += approach + i32::from(from_dist - to_dist) * approach_step;
        }

        let (f, r) = file_rank_of(to);
        let centrality = 3 - (f - 3).abs().max(r - 3).abs();
        if centrality > 0 {
            score += center + i32::from(centrality) * center_step;
            if matches!(from_pt, PieceType::Rook | PieceType::Queen) {
                score += rook_center * i32::from(centrality);
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
        self.score_with_map(board, m, state, &nearest, true)
    }
}

#[cfg(test)]
mod tests;
