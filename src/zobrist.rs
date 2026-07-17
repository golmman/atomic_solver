//! Deterministic Zobrist hashing for atomic-chess positions.
//!
//! Uses `Board::hash()` for the piece/side/castling/en-passant component and
//! adds a rule50 key for transposition-table lookup.

use atomic_movegen::board::Board;
use atomic_movegen::types::{Move, PieceType};
use std::sync::OnceLock;

pub const INF: u64 = 1 << 60;

const MAX_PATH_DEPTH: usize = 4096;
const PATH_MOVE_NB: usize = 64 * 64 * (PieceType::NB + 3);

static ZOBRIST: OnceLock<Zobrist> = OnceLock::new();

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

pub struct Zobrist {
    rule50_keys: [u64; 101],
    path_move_keys: Vec<u64>,
    path_depth_keys: Vec<u64>,
}

impl Zobrist {
    fn new() -> Self {
        let mut rng = SplitMix64(0x9e3779b97f4a7c15);

        let mut rule50_keys = [0u64; 101];
        for key in rule50_keys.iter_mut() {
            *key = rng.next();
        }

        let path_move_keys = (0..PATH_MOVE_NB).map(|_| rng.next()).collect();
        let path_depth_keys = (0..MAX_PATH_DEPTH).map(|_| rng.next()).collect();

        Self {
            rule50_keys,
            path_move_keys,
            path_depth_keys,
        }
    }

    fn get() -> &'static Self {
        ZOBRIST.get_or_init(Zobrist::new)
    }

    fn path_random(&self, mv: Move, depth: usize) -> u64 {
        let from = mv.from_sq() as u8 as usize;
        let to = mv.to_sq() as u8 as usize;
        let kind = move_kind(mv);
        let move_index = from + to * 64 + kind * 64 * 64;
        let depth_index = depth % MAX_PATH_DEPTH;
        self.path_move_keys[move_index] ^ self.path_depth_keys[depth_index]
    }
}

fn move_kind(mv: Move) -> usize {
    if mv.is_promotion() {
        match mv.promotion_type() {
            PieceType::Queen => 3,
            PieceType::Rook => 4,
            PieceType::Bishop => 5,
            PieceType::Knight => 6,
            _ => 0,
        }
    } else if mv.is_castling() {
        1
    } else if mv.is_en_passant() {
        2
    } else {
        0
    }
}

pub fn hash(board: &Board, rule50: u16) -> u64 {
    let z = Zobrist::get();
    board.hash() ^ z.rule50_keys[rule50.min(100) as usize]
}

pub fn path_random(mv: Move, depth: usize) -> u64 {
    Zobrist::get().path_random(mv, depth)
}

#[cfg(test)]
mod tests {
    use super::path_random;
    use atomic_movegen::types::{Move, PieceType, Square};

    #[test]
    fn normal_and_queen_promotion_differ() {
        let normal = Move::make_move(Square::A7, Square::A8);
        let promo = Move::make_promotion(Square::A7, Square::A8, PieceType::Queen);
        assert_ne!(path_random(normal, 0), path_random(promo, 0));
    }

    #[test]
    fn promotion_pieces_have_distinct_keys() {
        let q = Move::make_promotion(Square::A7, Square::A8, PieceType::Queen);
        let r = Move::make_promotion(Square::A7, Square::A8, PieceType::Rook);
        let b = Move::make_promotion(Square::A7, Square::A8, PieceType::Bishop);
        let n = Move::make_promotion(Square::A7, Square::A8, PieceType::Knight);

        let keys: [u64; 4] = [
            path_random(q, 0),
            path_random(r, 0),
            path_random(b, 0),
            path_random(n, 0),
        ];

        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                assert_ne!(keys[i], keys[j]);
            }
        }
    }

    #[test]
    fn normal_and_castling_differ() {
        let normal = Move::make_move(Square::E1, Square::H1);
        let castle = Move::make_castling(Square::E1, Square::H1);
        assert_ne!(path_random(normal, 0), path_random(castle, 0));
    }

    #[test]
    fn normal_and_en_passant_differ() {
        let normal = Move::make_move(Square::D5, Square::E6);
        let ep = Move::make_enpassant(Square::D5, Square::E6);
        assert_ne!(path_random(normal, 0), path_random(ep, 0));
    }
}
