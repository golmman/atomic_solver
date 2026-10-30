//! Static immediate-explosion closure (provari plan4, backlog item 4a).
//!
//! **Experimental heuristic, default off** (`--static-tactics`). At `dfpn`
//! node entry, after the terminal/depth/repetition/TT solved fast paths and
//! before move ordering, a position whose side to move can legally capture an
//! enemy piece *beside the enemy commoner* is claimed as a `Win` in one move.
//!
//! This is a sound static verdict, not an estimate: the claim is derived from
//! a move of the frame's own legal move list, and the outcome is from the
//! side-to-move perspective (so it is valid for OR and AND nodes alike). A
//! miss is never a correctness issue — a position this module declines simply
//! reaches the ordinary search, which must return the same value.
//!
//! The scan is internal work: it generates no moves, plays nothing, mutates
//! nothing, touches no transposition entry, and is **not** billed as a child
//! evaluation.
//!
//! # Target shape
//!
//! A capturing move qualifies when its destination
//!
//! 1. holds an enemy non-commoner piece,
//! 2. lies in the blast zone of **every** enemy commoner (so the explosion
//!    leaves the opponent with no commoner at all and the game ends at
//!    once), and
//! 3. lies outside the blast zone of every commoner of ours (nothing of ours
//!    is destroyed; this is the conservative direction and also the shape
//!    `atomic_movegen`'s pseudo-royal rule would reject as illegal).
//!
//! # Approximations (vs provari's `exploding_capture`)
//!
//! - **Only beside-commoner captures.** Provari's static closure also derives
//!   exposed-defender and mate-in-one verdicts; plan 4a implements only the
//!   immediate explosion, and a dedicated mate-in-one pre-scan is deferred.
//! - **Legal-move scan only.** The candidate must be one of `Position`'s legal
//!   moves, so no pseudo-legal move can create a false win. An explosion the
//!   move generator rejects is not claimed.
//! - **All enemy commoners, not any.** The plan sketched a *union* over the
//!   enemy commoners; this module intersects instead. With one enemy commoner
//!   (the norm) the two are identical; with several, a union would claim a win
//!   for an explosion that leaves another enemy commoner on the board — a
//!   false win. The intersection is the sound reading of the same idea.
//! - **No own-commoner blast.** Squares adjacent to one of our own commoners
//!   are excluded.
//! - **Commoner capturers are rejected.** Under `atomic_movegen`'s rules the
//!   capturing piece is destroyed by its own blast (pawns and the capturing
//!   pawn excepted), so a commoner capturer would lose its own commoner; the
//!   geometry of rule 3 already excludes such a move, and the explicit mover
//!   guard keeps the intent local instead of relying on that interaction.
//! - **Direct captures of the enemy commoner are not part of the shape.** They
//!   cannot occur in a legal game (the opponent would have had to leave the
//!   commoner attacked on our turn) and the terminal child classification
//!   covers them if they do.

use atomic_movegen::attacks;
use atomic_movegen::board::Board;
use atomic_movegen::types::{Bitboard, Move, MoveList, NO_PIECE, PieceType};

/// The first legal move of `moves` that is an immediate exploding capture
/// (see the module docs for the target shape).
///
/// `moves` must be the side-to-move-legal move list of `board` (the `dfpn`
/// frame's own list); the function never plays a move. `None` means "no
/// static win of this shape", never "not a win".
pub(super) fn immediate_exploding_capture(board: &Board, moves: &MoveList) -> Option<Move> {
    let targets = explosion_targets(board);
    if targets == Bitboard::EMPTY {
        return None;
    }
    moves.into_iter().find(|&mv| qualifies(board, targets, mv))
}

/// Whether `mv` explodes the enemy commoners given the precomputed
/// [`explosion_targets`] bitboard.
///
/// Split out so the piece rules (capture, non-en-passant, non-commoner mover)
/// are unit-testable independently of the geometry that makes a legal-move
/// instance of some of them unreachable.
fn qualifies(board: &Board, targets: Bitboard, mv: Move) -> bool {
    if !board.is_capture(mv) || mv.is_en_passant() {
        return false;
    }
    if (targets & Bitboard::square_bb(mv.to_sq())) == Bitboard::EMPTY {
        return false;
    }
    let mover = board.piece_on(mv.from_sq());
    mover != NO_PIECE && mover.type_of() != Some(PieceType::Commoner)
}

/// Enemy non-commoner squares whose capture destroys every enemy commoner
/// without destroying one of ours.
///
/// The blast zone of a square is its 8-neighbourhood. An empty bitboard means
/// the position has no such target at all (also when the enemy commoner is
/// already gone — those nodes are terminal and never reach this module).
fn explosion_targets(board: &Board) -> Bitboard {
    let us = board.side_to_move();
    let them = us.flip();
    let enemy_commoners = board.commoners(them);
    if enemy_commoners.is_empty() {
        return Bitboard::EMPTY;
    }

    // Intersection over the enemy commoners: a target must be beside *all* of
    // them, otherwise the explosion leaves an enemy commoner alive.
    let mut candidates = Bitboard(u64::MAX);
    let mut commoners = enemy_commoners;
    while !commoners.is_empty() {
        candidates = candidates & attacks::king_attacks(commoners.pop_lsb());
    }

    let mut forbidden = Bitboard::EMPTY;
    let mut own = board.commoners(us);
    while !own.is_empty() {
        forbidden = forbidden | attacks::king_attacks(own.pop_lsb());
    }

    candidates & board.pieces_color(them) & !board.pieces_pt(PieceType::Commoner) & !forbidden
}

#[cfg(test)]
mod tests;
