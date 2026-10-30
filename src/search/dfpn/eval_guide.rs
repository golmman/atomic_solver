//! Hand evaluation and EF-PN child scaling (provari plan5).
//!
//! **Experimental heuristic, default off** (`--eval-pn <off|both|or>`). This
//! module changes only the *starting* proof/disproof numbers of newly seen
//! children: a parent's static hand evaluation is squashed to a three-valued
//! step and, per EF-PN (Winands & Schadd), scales the fresh child's bounds.
//! It never creates an outcome, never touches the TT by itself, and never
//! overrides a solved, repetition, rule50, or TT-reused bound — those children
//! are returned before the fresh tail that applies the scale.
//!
//! The evaluation mirrors provari's hand guide (`src/eval.rs` of the oracle,
//! consulted read-only) with deliberate approximations, documented here:
//!
//! - **Commoners instead of a single king.** Our board may carry several
//!   commoners. The king ring is the union of all enemy commoners' king
//!   rings, and the attack terms are dropped for a side whose commoner stands
//!   adjacent to an enemy commoner (provari's "touching kings" exclusion —
//!   with touching commoners neither can be checked or exploded safely). For
//!   the single-commoner norm the two readings coincide.
//! - **Per-piece reach, not attackers-to per square.** The attack terms are
//!   generated in one pass over the side's non-commoner pieces with the
//!   crate's attack tables, so they come out of the same bitboards provari
//!   intersects. A disagreement is an evaluation difference, never a
//!   soundness one.
//! - **Fixed constants.** `EF_A`, `EF_B`, `DECISIVE` and the term weights are
//!   provari's shipped values, not CLI knobs and not the ordering scorer's
//!   tunable `ScorerParams`. Tuning them is a later question.
//! - **Linear branch only.** Provari's evaluation may take a logit/network
//!   path; a three-valued hand step never does, so only the exact integer
//!   linear branch is implemented.
//!
//! The scale is a `Copy` value computed once per frame visit and threaded
//! down the child-evaluation calls; it is deliberately *not* stored on
//! `Search` (no hidden frame state, restart-safe recursion).
//!
//! This file is slightly above 10 KiB because the hand evaluation, its
//! documented approximations, and the EF-PN scale table form one contract;
//! unit tests live in `eval_guide/tests.rs`.

use atomic_movegen::attacks;
use atomic_movegen::board::Board;
use atomic_movegen::types::{Bitboard, Color, PieceType, Square};

use crate::zobrist::INF;

/// Atomic piece values (provari's Atomic-Stockfish midgame/endgame blend),
/// indexed by `PieceType` (pawn, knight, bishop, rook, queen, commoner).
const VALUES: [i32; 6] = [305, 545, 634, 930, 1670, 0];
/// One of their pieces beside their commoner that we attack.
const TARGET: i32 = 400;
/// Two or more such pieces: nearly decisive.
const TARGETS: i32 = 1000;
/// One of their pieces beside their commoner, which a capture would explode.
const FUSE: i32 = 60;
/// Each of our attacks on a square beside their commoner.
const PRESSURE: i32 = 20;
/// Each of our pieces that can check their commoner next move.
const CHECKER: i32 = 60;
/// The evaluation that decides a position (provari's `HAND_DECISIVE`).
const DECISIVE: i32 = 500;

/// EF-PN parameters: how much a parent's evaluation deviates a fresh child's
/// disproof (A) and proof (B) number.
const EF_A: u64 = 10;
const EF_B: u64 = 5;

/// Which node kinds EF-PN scales.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EvalPnMode {
    /// Shipped behavior: no evaluation, no scaling.
    #[default]
    Off,
    /// Provari's formula at both node kinds.
    Both,
    /// The attacking (OR-parent) branch only; AND-parent children stay
    /// neutral.
    Or,
}

impl EvalPnMode {
    /// Whether the evaluation runs at all.
    #[must_use]
    pub fn is_enabled(self) -> bool {
        !matches!(self, Self::Off)
    }
}

/// The evaluation for `side`, from `side`'s perspective.
pub(super) fn evaluation(board: &Board, side: Color) -> i32 {
    terms(board, side).score() - terms(board, side.flip()).score()
}

/// EF-PN's three-valued step: `0` = the OR side is clearly behind, `1` =
/// unclear, `2` = clearly ahead. `or_side` is the attacking side (the root
/// side to move); the evaluation is taken from that side's perspective, so no
/// sign flip is needed here (provari's `band` applies the same rule by
/// comparing the side to move with the attacker).
pub(super) fn step(board: &Board, or_side: Color) -> u8 {
    let score = evaluation(board, or_side);
    if score >= DECISIVE {
        2
    } else if score <= -DECISIVE {
        0
    } else {
        1
    }
}

/// What one side has: its material and its attack on the other commoners.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct Terms {
    material: i32,
    /// Their non-commoner pieces beside their commoner that we attack, capped
    /// at two.
    targets: u32,
    /// Their non-commoner pieces beside their commoner.
    fuses: u32,
    /// Our attacks on the squares beside their commoner.
    pressure: u32,
    /// Our pieces that can check their commoner next move.
    checkers: u32,
}

impl Terms {
    fn score(&self) -> i32 {
        let targets = match self.targets {
            0 => 0,
            1 => TARGET,
            _ => TARGETS,
        };
        self.material
            + targets
            + FUSE * self.fuses as i32
            + PRESSURE * self.pressure as i32
            + CHECKER * self.checkers as i32
    }
}

/// One side's terms from `us`'s perspective. Material is always counted; the
/// attack terms are dropped (leaving material only) when either side has no
/// commoner or one of our commoners touches an enemy commoner.
fn terms(board: &Board, us: Color) -> Terms {
    let them = us.flip();
    let mut terms = Terms {
        material: material(board, us),
        ..Terms::default()
    };

    let our_commoners = board.commoners(us);
    let their_commoners = board.commoners(them);
    if our_commoners.is_empty() || their_commoners.is_empty() {
        return terms;
    }

    // The union of the enemy commoners' king rings.
    let mut ring = Bitboard::EMPTY;
    let mut rest = their_commoners;
    while !rest.is_empty() {
        ring = ring | attacks::king_attacks(rest.pop_lsb());
    }

    // Touching commoners can be neither checked nor exploded.
    let mut own = our_commoners;
    while !own.is_empty() {
        let sq = own.pop_lsb();
        if (ring & Bitboard::square_bb(sq)) != Bitboard::EMPTY {
            return terms;
        }
    }

    let mut forbidden = Bitboard::EMPTY;
    let mut own = our_commoners;
    while !own.is_empty() {
        forbidden = forbidden | attacks::king_attacks(own.pop_lsb());
    }
    let candidates =
        ring & board.pieces_color(them) & !board.pieces_pt(PieceType::Commoner) & !forbidden;

    let occupied = board.occupied();
    let ours = board.pieces_color(us);
    let mut pieces = ours & !board.pieces_pt(PieceType::Commoner);
    let mut attacked = Bitboard::EMPTY;
    let mut pressure = 0u32;
    let mut checkers = 0u32;
    while !pieces.is_empty() {
        let sq = pieces.pop_lsb();
        let Some(pt) = board.piece_on(sq).type_of() else {
            continue;
        };
        let reach = attacks_from(pt, us, sq, occupied);
        attacked = attacked | reach;
        pressure += (reach & ring).count();
        // Squares from which this piece would attack one of their commoners.
        // `us` is the piece colour; the helper flips it internally so that a
        // pawn's reverse attack direction is the enemy's.
        let checks = checking_squares(pt, us, their_commoners, occupied);
        // `!ours`: a square occupied by one of our own pieces is not reachable.
        if (reach & checks & !ours) != Bitboard::EMPTY {
            checkers += 1;
        }
    }

    terms.targets = (attacked & candidates).count().min(2);
    terms.fuses = (ring & board.pieces_color(them) & !board.pieces_pt(PieceType::Commoner)).count();
    terms.pressure = pressure;
    terms.checkers = checkers;
    terms
}

/// `us`'s non-commoner material at Atomic values.
fn material(board: &Board, us: Color) -> i32 {
    let mut total = 0;
    let mut pieces = board.pieces_color(us) & !board.pieces_pt(PieceType::Commoner);
    while !pieces.is_empty() {
        let sq = pieces.pop_lsb();
        if let Some(pt) = board.piece_on(sq).type_of() {
            total += VALUES[pt as usize];
        }
    }
    total
}

/// The union over `commoners` of the squares from which a `pt` piece of colour
/// `color` attacks the square.
///
/// The reverse test reuses [`attacks_from`] with the *other* colour from the
/// target square: colour only matters for pawns, where a `color` pawn on `s`
/// attacks `t` exactly when an opposite-colour pawn on `t` attacks `s`.
fn checking_squares(
    pt: PieceType,
    color: Color,
    commoners: Bitboard,
    occupied: Bitboard,
) -> Bitboard {
    let reverse = color.flip();
    let mut checks = Bitboard::EMPTY;
    let mut rest = commoners;
    while !rest.is_empty() {
        let sq = rest.pop_lsb();
        checks = checks | attacks_from(pt, reverse, sq, occupied);
    }
    checks
}

/// Attack set of `pt` from `sq` under an explicit occupancy.
fn attacks_from(pt: PieceType, color: Color, sq: Square, occupied: Bitboard) -> Bitboard {
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

/// How a parent node scales its fresh children's starting numbers.
///
/// The four fields are the exact linear-branch arithmetic of provari's
/// `scale` for a three-valued step: `pn' = pn * proof_mul + proof_add`,
/// `dn' = dn * disproof_mul + disproof_add`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ChildScale {
    proof_mul: u64,
    proof_add: u64,
    disproof_mul: u64,
    disproof_add: u64,
}

impl ChildScale {
    /// The scale a node of this kind applies to its fresh children, or `None`
    /// when EF-PN is off or disabled for this parent kind.
    ///
    /// OR ("attacking") parents multiply only: the proof number by
    /// `1 + EF_B·(2 − step)`, the disproof number by `1 + EF_A·step`. AND
    /// ("defending") parents add to the proof number (`+ EF_A·(2 − step)`)
    /// and multiply the disproof number by `1 + EF_B·step`.
    pub(super) fn for_parent(is_or_node: bool, step: u8, mode: EvalPnMode) -> Option<Self> {
        let step = u64::from(step.min(2));
        match mode {
            EvalPnMode::Off => None,
            EvalPnMode::Or if !is_or_node => None,
            _ if is_or_node => Some(Self {
                proof_mul: 1 + EF_B * (2 - step),
                proof_add: 0,
                disproof_mul: 1 + EF_A * step,
                disproof_add: 0,
            }),
            EvalPnMode::Both => Some(Self {
                proof_mul: 1,
                proof_add: EF_A * (2 - step),
                disproof_mul: 1 + EF_B * step,
                disproof_add: 0,
            }),
            // `EvalPnMode::Or` with an AND parent returned above.
            EvalPnMode::Or => None,
        }
    }

    /// Scale one fresh child's `(pn, dn)`. Never returns 0 and never exceeds
    /// [`INF`]; inputs are the neutral `(1, 1)` or a plan2 class number, so
    /// the practical magnitudes stay small (≤ 15 × 21).
    pub(super) fn apply(self, pn: u64, dn: u64) -> (u64, u64) {
        let scale = |value: u64, mul: u64, add: u64| {
            value.saturating_mul(mul).saturating_add(add).clamp(1, INF)
        };
        (
            scale(pn, self.proof_mul, self.proof_add),
            scale(dn, self.disproof_mul, self.disproof_add),
        )
    }
}

#[cfg(test)]
mod tests;
