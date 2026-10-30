//! Quiet-move grouping at OR nodes (provari plan3).
//!
//! **Experimental heuristic, default off** (`--quiet-grouping`). At an OR
//! node, moves accepted by [`is_plain`] are hidden behind one delayed group
//! child and only evaluated when DF-PN selects that group. Nothing here is a
//! correctness rule: a false positive merely delays a move until the group is
//! opened, and every opened member is reclassified by the existing
//! `move_class` code.
//!
//! The group's starting proof numbers are estimates in the same sense as the
//! move-class numbers: `pn = GROUP_PN` and `dn = hidden_count` do not prove
//! anything. A group is never solved; a pending group therefore cannot let a
//! parent conclude a Loss or Draw through it.
//!
//! # Predicate approximations
//!
//! [`is_plain`] is a conservative *superset* of provari's `Mover::plain`: it
//! may reject moves that could safely be grouped (losing some benefit), but it
//! accepts only moves that are reasonably safe to delay. It is a parent-side
//! test; the child is never played here.
//!
//! - **Discovered checks/threats through another line are not detected.** The
//!   predicate tests the moved piece's attacks from its origin square (before
//!   the move) and from its destination (under the post-move occupancy). A
//!   move that uncovers a stationary check or threat along a line that does
//!   not originate on the moving piece's square can be misclassified as
//!   plain. That only delays it: the group opens before a parent can be
//!   decided through it.
//! - **The destination is not required to be empty beyond the capture
//!   rejection.** Normal quiet moves have an empty destination by
//!   construction; en passant/castling/promotions are rejected explicitly.
//! - **Hanging plain moves are grouped too.** They receive the existing
//!   `HangingQuiet` starting number only when the group opens.
//! - **The target shape is the plan2 shape** (`move_class::threat_targets`):
//!   an enemy non-commoner adjacent to an enemy commoner and not adjacent to
//!   one of our commoners. It is deliberately duplicated locally so ordering
//!   scores cannot change.

use atomic_movegen::attacks;
use atomic_movegen::board::Board;
use atomic_movegen::types::{Bitboard, Color, Move, NO_PIECE, PieceType};

use crate::position::Position;

/// Starting proof number for the delayed plain-move group (provari's
/// `QUIET` number).
pub(super) const GROUP_PN: u64 = 9;

/// Whether `mv` may be hidden behind the OR-node quiet-move group.
///
/// `pos` is the **parent** position; the move is not played. All checks are
/// conservative: a `false` result only loses grouping benefit, never
/// soundness.
pub(super) fn is_plain(pos: &Position, mv: Move) -> bool {
    let board = pos.board();
    if board.is_capture(mv) || mv.is_castling() || mv.is_promotion() || mv.is_en_passant() {
        return false;
    }

    let from = mv.from_sq();
    let to = mv.to_sq();
    let from_piece = board.piece_on(from);
    if from_piece == NO_PIECE {
        return false;
    }
    let Some(pt) = from_piece.type_of() else {
        return false;
    };
    if pt == PieceType::Commoner {
        return false;
    }

    let us = board.side_to_move();
    let them = us.flip();
    let occupied = board.occupied();
    let from_bb = Bitboard::square_bb(from);
    let to_bb = Bitboard::square_bb(to);
    let after_occupied = (occupied & !from_bb) | to_bb;

    // Giving check is not plain. Test both the destination (the usual case)
    // and the origin square (the moved piece's own abandoned attack).
    let enemy_commoners = board.commoners(them);
    if !enemy_commoners.is_empty()
        && ((attacks_from(pt, us, to, after_occupied) & enemy_commoners) != Bitboard::EMPTY
            || (attacks_from(pt, us, from, occupied) & enemy_commoners) != Bitboard::EMPTY)
    {
        return false;
    }

    // Creating a king-explosion target is not plain, for the same two
    // squares. Targets are static with respect to this move: the shape only
    // depends on commoner placement and enemy pieces.
    let targets = threat_targets(board, us);
    if !targets.is_empty()
        && ((attacks_from(pt, us, to, after_occupied) & targets) != Bitboard::EMPTY
            || (attacks_from(pt, us, from, occupied) & targets) != Bitboard::EMPTY)
    {
        return false;
    }

    true
}

/// The plan2 king-explosion target shape: enemy non-commoner squares adjacent
/// to an enemy commoner and not adjacent to one of our commoners.
fn threat_targets(board: &Board, us: Color) -> Bitboard {
    let them = us.flip();
    let their_commoners = board.commoners(them);
    if their_commoners.is_empty() {
        return Bitboard::EMPTY;
    }

    let mut forbidden = Bitboard::EMPTY;
    let mut own = board.commoners(us);
    while !own.is_empty() {
        forbidden = forbidden | attacks::king_attacks(own.pop_lsb());
    }

    let mut candidates = Bitboard::EMPTY;
    let mut commoners = their_commoners;
    while !commoners.is_empty() {
        candidates = candidates | attacks::king_attacks(commoners.pop_lsb());
    }

    candidates & board.pieces_color(them) & !board.pieces_pt(PieceType::Commoner) & !forbidden
}

/// Attack set of `pt` on `sq` under an explicit occupancy (the ordering
/// scorer's private helper is deliberately not exposed).
fn attacks_from(
    pt: PieceType,
    color: Color,
    sq: atomic_movegen::types::Square,
    occupied: Bitboard,
) -> Bitboard {
    match pt {
        PieceType::Pawn => attacks::pawn_attacks(color, sq),
        PieceType::Knight => attacks::knight_attacks(sq),
        PieceType::Bishop => attacks::bishop_attacks(sq, occupied),
        PieceType::Rook => attacks::rook_attacks(sq, occupied),
        PieceType::Queen => attacks::queen_attacks(sq, occupied),
        PieceType::Commoner => attacks::king_attacks(sq),
        _ => Bitboard::EMPTY,
    }
}

#[cfg(test)]
mod tests;
