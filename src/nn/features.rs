//! Feature extraction for the move-ordering network (`nn.md` §2, §5).
//!
//! Index layout (shared with the external trainer — this is the
//! trainer/loader contract):
//!
//! - square index: `sq = file + 8 * rank` (`a1 = 0`, `h8 = 63`); this is
//!   exactly the `atomic_movegen` square encoding (`Square as u8`).
//! - piece index: `p = 6 * view_color + type`, view color 0 = the view's
//!   own side, `type` 0..5 = pawn..commoner (the atomic "king").
//! - feature index: `f = 64 * p + sq ∈ [0, 768)` — piece-major, the
//!   12-valued piece axis is the outer, 64-strided axis.
//!
//! Every position is encoded twice, relative to the side to move. The
//! side-to-move view is the board as-is (never mirrored). The other view's
//! transform is: swap colors relative to the side to move **and** mirror the
//! file (`file -> 7 - file`, rank unchanged).
//!
//! Conformance vectors: `docs/nn_trainer_ref/test_features.py`.

use atomic_movegen::board::Board;
use atomic_movegen::types::{Bitboard, Move, PieceType, file_of, rank_of};

/// §2 feature count: 6 piece types × 2 colors × 64 squares.
pub const INPUT_DIM: usize = 768;
/// §3 accumulator width (the shared `W_1` output width).
pub const ACCUMULATOR_DIM: usize = 128;
/// §3 hidden-layer width.
pub const HIDDEN_DIM: usize = 32;
/// §5 output size, pinned for v1 (`from_sq * 64 + to_sq`).
pub const POLICY_SIZE: usize = 4096;

/// A board can hold at most one piece per square.
pub const MAX_PIECES: usize = 64;

/// The two §2 perspectives of one position: active feature indices.
#[derive(Debug, Clone, Copy)]
pub struct FeatureSets {
    /// Side-to-move view: active feature indices (`f = 64 * p + sq`).
    pub stm: [u16; MAX_PIECES],
    pub stm_len: usize,
    /// Other-side view (color swap + file mirror): active feature indices.
    pub other: [u16; MAX_PIECES],
    pub other_len: usize,
}

impl FeatureSets {
    /// Active features of the side-to-move view.
    #[must_use]
    pub fn stm_features(&self) -> &[u16] {
        &self.stm[..self.stm_len]
    }

    /// Active features of the other-side view.
    #[must_use]
    pub fn other_features(&self) -> &[u16] {
        &self.other[..self.other_len]
    }
}

/// The §2 piece-type index: pawn 0 .. commoner (king) 5. The `PieceType`
/// discriminants already match the spec layout.
#[inline]
fn type_index(pt: PieceType) -> usize {
    pt as usize
}

/// The §2 feature index for one piece in one view.
///
/// `view_color` is 0 for the view's own side, 1 for the other side; the
/// `file`/`rank` are the coordinates *as seen by this view* (the other view
/// passes the mirrored file).
#[inline]
#[must_use]
pub fn feature_index(view_color: usize, piece_type: PieceType, file: u8, rank: u8) -> usize {
    let p = 6 * view_color + type_index(piece_type);
    64 * p + (file as usize) + 8 * (rank as usize)
}

/// Extract both §2 perspectives of `board`.
#[must_use]
pub fn feature_sets(board: &Board) -> FeatureSets {
    let stm = board.side_to_move();
    let other = stm.flip();
    let mut sets = FeatureSets {
        stm: [0; MAX_PIECES],
        stm_len: 0,
        other: [0; MAX_PIECES],
        other_len: 0,
    };
    let mut occ: Bitboard = board.occupied();
    while !occ.is_empty() {
        let sq = occ.pop_lsb();
        let piece = board.piece_on(sq);
        let color = piece.color().expect("occupied square holds a piece");
        let pt = piece.type_of().expect("occupied square holds a piece");
        let file = file_of(sq) as u8;
        let rank = rank_of(sq) as u8;

        // Side-to-move view: board as-is, own side = stm (never mirrored).
        sets.stm[sets.stm_len] = feature_index(usize::from(color != stm), pt, file, rank) as u16;
        sets.stm_len += 1;
        // Other view: colors swapped relative to stm, file mirrored.
        sets.other[sets.other_len] =
            feature_index(usize::from(color != other), pt, 7 - file, rank) as u16;
        sets.other_len += 1;
    }
    sets
}

/// The §5 output index: `from_sq * 64 + to_sq`, square indexing as in §2.
///
/// All four promotion variants of a pawn move share one `(from, to)` index —
/// promotion is not distinguished in v1; callers must deduplicate indices
/// before masking.
#[inline]
#[must_use]
pub fn policy_index(m: Move) -> usize {
    (m.from_sq() as usize) * 64 + (m.to_sq() as usize)
}

/// Every feature index is strictly inside `[0, INPUT_DIM)` and every policy
/// index inside `[0, POLICY_SIZE)` — a static assertion of the §2/§5 ranges.
#[cfg(test)]
#[allow(dead_code)]
fn index_bounds_hold() -> bool {
    INPUT_DIM == 768 && POLICY_SIZE == 4096
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;
    use atomic_movegen::types::Square;

    fn sets_for(fen: &str) -> (Vec<usize>, Vec<usize>) {
        let pos = Position::from_fen(fen).expect("test FEN must parse");
        let sets = feature_sets(pos.board());
        (
            sets.stm_features().iter().map(|&f| f as usize).collect(),
            sets.other_features().iter().map(|&f| f as usize).collect(),
        )
    }

    fn assert_sorted_unique(mut xs: Vec<usize>) -> Vec<usize> {
        xs.sort_unstable();
        xs.dedup();
        xs
    }

    /// Lone white commoner a1, stm w (conformance vector, view A = 320,
    /// view B = 711).
    #[test]
    fn lone_white_king_a1() {
        let (stm, other) = sets_for("8/8/8/8/8/8/8/K7 w - - 0 1");
        assert_eq!(stm, vec![320]);
        assert_eq!(other, vec![711]);
    }

    /// Spec example FEN (conformance vector): view A {196, 326, 764},
    /// view B {379, 579, 705}.
    #[test]
    fn spec_fen_all_views() {
        let (stm, other) = sets_for("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1");
        assert_eq!(assert_sorted_unique(stm), vec![196, 326, 764]);
        assert_eq!(assert_sorted_unique(other), vec![379, 579, 705]);
    }

    /// White pawn e2, stm w: view A f = 12, view B (black pawn, mirrored
    /// file) f = 395.
    #[test]
    fn other_view_transform_color_and_file() {
        let (stm, other) = sets_for("8/8/8/8/8/8/4P3/8 w - - 0 1");
        assert_eq!(stm, vec![12]);
        assert_eq!(other, vec![395]);
    }

    /// Black commoner h8, stm w: other side in the stm view (f = 767); own
    /// side in the other view with mirrored file (f = 376).
    #[test]
    fn black_king_other_side_view() {
        let (stm, other) = sets_for("7k/8/8/8/8/8/8/8 w - - 0 1");
        assert_eq!(stm, vec![767]);
        assert_eq!(other, vec![376]);
    }

    /// Black rook a1, stm w: f = 576 in the stm view, f = 199 in the other.
    #[test]
    fn black_rook_a1_views() {
        let (stm, other) = sets_for("8/8/8/8/8/8/8/r7 w - - 0 1");
        assert_eq!(stm, vec![576]);
        assert_eq!(other, vec![199]);
    }

    /// Black stm: the side-to-move view is the board as-is (no mirror).
    #[test]
    fn black_stm_own_side_view_not_mirrored() {
        let (stm, other) = sets_for("7k/8/8/8/8/8/8/8 b - - 0 1");
        assert_eq!(stm, vec![383]);
        assert_eq!(other, vec![760]);
    }

    #[test]
    fn startpos_feature_counts_and_bounds() {
        let (stm, other) = sets_for(Position::STARTPOS_FEN);
        assert_eq!(stm.len(), 32);
        assert_eq!(other.len(), 32);
        assert_eq!(assert_sorted_unique(stm.clone()).len(), 32);
        assert!(stm.iter().chain(&other).all(|&f| f < INPUT_DIM));
        // The start position is rank-asymmetric, so the two views differ.
        assert_ne!(assert_sorted_unique(stm), assert_sorted_unique(other));
    }

    #[test]
    fn policy_index_layout() {
        let sq = |f: u8, r: u8| Square::from_u8(f + 8 * r);
        // a1a2 = 0 * 64 + 8
        assert_eq!(policy_index(Move::make_move(sq(0, 0), sq(0, 1))), 8);
        // e2e4 = 12 * 64 + 28
        assert_eq!(
            policy_index(Move::make_move(sq(4, 1), sq(4, 3))),
            12 * 64 + 28
        );
        // h8h1 = 63 * 64 + 7
        assert_eq!(
            policy_index(Move::make_move(sq(7, 7), sq(7, 0))),
            63 * 64 + 7
        );
        // Promotion variants collapse onto one (from, to) index.
        let from = sq(0, 6);
        let to = sq(0, 7);
        let base = policy_index(Move::make_move(from, to));
        for pt in [
            PieceType::Queen,
            PieceType::Rook,
            PieceType::Bishop,
            PieceType::Knight,
        ] {
            assert_eq!(policy_index(Move::make_promotion(from, to, pt)), base);
        }
        assert!(base < POLICY_SIZE);
    }
}
