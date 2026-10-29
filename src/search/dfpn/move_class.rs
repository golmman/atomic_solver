//! OR-node move-class starting proof numbers (provari plan2).
//!
//! This file is slightly above 10 KiB because the class table, the threat
//! counter, the evasion cap, and the cheap-blast test form one classification
//! contract; unit tests live in `move_class/tests.rs`.
//!
//! **Experimental heuristic, default off** (`--move-class-pn`). This module
//! changes only the *initial estimate* of a newly seen, non-terminal child of
//! an OR node (the side whose win is being proved). It never classifies
//! terminal positions, never overwrites learned or TT-reused bounds, and makes
//! no soundness claim: the values below are starting proof numbers for
//! DF-PN's selection, not proofs.
//!
//! The class table mirrors provari's published starting numbers
//! (`src/dfpn.rs` in the oracle, consulted read-only) with deliberate
//! approximations, all documented on the helper that implements them:
//!
//! | Class | Starting `pn` |
//! | --- | ---: |
//! | Check | the number of legal replies, capped at 8 (at least 1) |
//! | Double threat | 1 |
//! | Threat | 4 |
//! | Weak threat | 8 |
//! | Hanging quiet | 15 |
//! | Quiet | 9 |
//!
//! Every returned number is in `[1, 15]`: `Children::unsolved_bounds`'s
//! non-degeneracy invariant and the TT pre-probe rely on positive, finite
//! bounds.
//!
//! # Approximations (vs provari)
//!
//! - **Legal evasions count every legal reply**, not only replies that
//!   survive an immediate explosion. The safe-reply filter needs an
//!   explosion detector and is deferred (backlog item 4).
//! - **Cheaply blastable** is a conservative attack/defence test with a
//!   coarse piece-value table, not a blast-aware exchange evaluation. It
//!   never refines ordering scores and does not share the ordering aSEE.
//! - **Immediately lost** is a cheap over-approximation: any defender
//!   non-commoner attacking the ring around our commoner counts, even if
//!   that attack cannot legally be used. It can only *suppress* a
//!   double-threat `pn = 1`, never manufacture one.
//! - Provari's plain-move requirement (`mover.plain`) is not implemented:
//!   captures and promotions that create no target follow the quiet or
//!   hanging rule, matching the plan's stated scope. Castling lands with an
//!   empty destination square and therefore never counts as hanging.

use atomic_movegen::attacks;
use atomic_movegen::board::Board;
use atomic_movegen::types::{Bitboard, Color, Move, MoveList, NO_PIECE, PieceType};

use crate::position::Position;

/// The class of an attacker (OR-node) move, from the parent's perspective.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MoveClass {
    Check,
    DoubleThreat,
    Threat,
    WeakThreat,
    HangingQuiet,
    Quiet,
}

impl MoveClass {
    /// Starting proof number for this class. `legal_evasions` is only
    /// consulted for [`MoveClass::Check`] and is clamped to `[1, 8]` here so
    /// no caller can produce a degenerate bound.
    pub(super) fn starting_proof_number(self, legal_evasions: u32) -> u64 {
        match self {
            MoveClass::Check => u64::from(legal_evasions.clamp(1, 8)),
            MoveClass::DoubleThreat => 1,
            MoveClass::Threat => 4,
            MoveClass::WeakThreat => 8,
            MoveClass::HangingQuiet => 15,
            MoveClass::Quiet => 9,
        }
    }
}

/// Starting proof number for `mv` played in `pos`'s parent, with `pos` the
/// **child** position after the move. Must only be called for a non-terminal
/// child whose bounds are still the neutral `(1, 1)`.
///
/// The parent's side to move is `pos.side_to_move().flip()`.
pub(super) fn starting_proof_number(pos: &Position, mv: Move) -> u64 {
    let class = classify(pos.board(), mv);
    let evasions = if class == MoveClass::Check {
        legal_reply_count(pos)
    } else {
        0
    };
    class.starting_proof_number(evasions)
}

/// Classify `mv` (already played, so `board` is the child position).
fn classify(board: &Board, mv: Move) -> MoveClass {
    // After the move the child is to move; the move's mover is the other side.
    let us = board.side_to_move().flip();

    // A check outranks every threat class (provari's `threats = if check`).
    // A checkmate/stalemate child is terminal and never reaches this module.
    if !board.checkers().is_empty() {
        return MoveClass::Check;
    }

    let targets = threat_targets(board, us);
    if targets >= 2 {
        // Provari turns a double threat into a static win when no defence
        // parries it; that shortcut is backlog item 4. Here the double
        // threat merely starts at 1, and only when we are not immediately
        // lost ourselves.
        return if immediately_lost(board, us) {
            MoveClass::Threat
        } else {
            MoveClass::DoubleThreat
        };
    }

    let blastable = cheaply_blastable(board, mv, us);
    match targets {
        1 if blastable => MoveClass::WeakThreat,
        1 => MoveClass::Threat,
        0 if blastable => MoveClass::HangingQuiet,
        _ => MoveClass::Quiet,
    }
}

/// Number of legal replies in the child, capped at 8 and floored at 1.
///
/// Deliberately counts *all* legal replies, not only replies that survive an
/// immediate explosion (see the module approximations). The movegen is
/// internal work and is not billed as a child evaluation.
fn legal_reply_count(pos: &Position) -> u32 {
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    (moves.len() as u32).clamp(1, 8)
}

/// Count the distinct squares of enemy non-commoner pieces that the mover
/// attacks, are adjacent to an enemy commoner, and are not adjacent to one of
/// the mover's own commoners.
///
/// Such a square is a king-explosion target: capturing the piece there
/// explodes the enemy commoner on the next move. The shape matches the
/// ordering scorer's §4-§5 threat test for one target; that scorer is left
/// untouched so this plan cannot change ordering scores.
fn threat_targets(board: &Board, us: Color) -> u32 {
    let them = us.flip();
    let their_commoners = board.commoners(them);
    if their_commoners.is_empty() {
        return 0;
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
    let candidates =
        candidates & board.pieces_color(them) & !board.pieces_pt(PieceType::Commoner) & !forbidden;

    let our_attackers = board.pieces_color(us) & !board.pieces_pt(PieceType::Commoner);
    let occupied = board.occupied();
    let mut count = 0;
    let mut rest = candidates;
    while !rest.is_empty() {
        let sq = rest.pop_lsb();
        if (board.attackers_to(sq, occupied) & our_attackers) != Bitboard::EMPTY {
            count += 1;
        }
    }
    count
}

/// Conservative over-approximation of "the defender has a legal move that
/// explodes our commoner at once": some defender non-commoner attacks the
/// commoner square or a square adjacent to it.
///
/// Over-approximating is the safe direction for this plan: the only consumer
/// suppresses a double-threat `pn = 1`, so a false positive merely leaves the
/// child at the threat class instead of the double-threat class.
fn immediately_lost(board: &Board, us: Color) -> bool {
    let them = us.flip();
    let defenders = board.pieces_color(them) & !board.pieces_pt(PieceType::Commoner);
    if defenders.is_empty() {
        return false;
    }
    let occupied = board.occupied();
    let mut commoners = board.commoners(us);
    while !commoners.is_empty() {
        let commoner = commoners.pop_lsb();
        let mut zone = attacks::king_attacks(commoner) | Bitboard::square_bb(commoner);
        while !zone.is_empty() {
            let sq = zone.pop_lsb();
            if (board.attackers_to(sq, occupied) & defenders) != Bitboard::EMPTY {
                return true;
            }
        }
    }
    false
}

/// Conservative "cheaply blastable" test: at least one defender
/// non-commoner attacks the moved piece's square, and either that square is
/// undefended by us or the cheapest attacker is worth no more than the moved
/// piece.
///
/// The value table is coarse and blast-unaware; a real blast-aware exchange
/// is backlog item 4. A move whose destination square is empty after the move
/// (castling) never qualifies.
fn cheaply_blastable(board: &Board, mv: Move, us: Color) -> bool {
    let to = mv.to_sq();
    let moved = board.piece_on(to);
    if moved == NO_PIECE || moved.color() != Some(us) {
        return false;
    }
    let Some(moved_pt) = moved.type_of() else {
        return false;
    };

    let occupied = board.occupied();
    let them = us.flip();
    let attackers = board.attackers_to(to, occupied)
        & board.pieces_color(them)
        & !board.pieces_pt(PieceType::Commoner);
    if attackers.is_empty() {
        return false;
    }

    let defended = (board.attackers_to(to, occupied) & board.pieces_color(us)) != Bitboard::EMPTY;
    if !defended {
        return true;
    }

    let mut cheapest = i32::MAX;
    let mut rest = attackers;
    while !rest.is_empty() {
        let sq = rest.pop_lsb();
        if let Some(pt) = board.piece_on(sq).type_of() {
            cheapest = cheapest.min(piece_value(pt));
        }
    }
    cheapest <= piece_value(moved_pt)
}

/// Coarse relative piece values for the cheap-blast test (not the scorer's
/// tunable material values, and not used for ordering).
fn piece_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Pawn => 1,
        PieceType::Knight | PieceType::Bishop => 3,
        PieceType::Rook => 5,
        PieceType::Queen => 9,
        // Losing a commoner ends the game; keep it above every material sum.
        PieceType::Commoner => 100,
        _ => 0,
    }
}

/// Test-only view of one classification: the class and its starting number.
#[cfg(test)]
fn classify_and_start(pos: &Position, mv: Move) -> (MoveClass, u64) {
    let class = classify(pos.board(), mv);
    let evasions = if class == MoveClass::Check {
        legal_reply_count(pos)
    } else {
        0
    };
    (class, class.starting_proof_number(evasions))
}

#[cfg(test)]
mod tests;
