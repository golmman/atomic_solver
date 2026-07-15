//! Deterministic Zobrist hashing for atomic-chess positions.
//!
//! Uses `Board::hash()` for the piece/side/castling/en-passant component and
//! adds a rule50 key for transposition-table lookup.

use atomic_movegen::board::Board;
use atomic_movegen::types::{Move, PieceType};
use std::sync::OnceLock;

pub const INF: u64 = 1 << 60;

const MAX_PATH_DEPTH: usize = 4096;
const PATH_MOVE_NB: usize = 64 * 64 * PieceType::NB;

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
        let promotion = mv.promotion_type() as u8 as usize;
        let move_index = from + to * 64 + promotion * 64 * 64;
        let depth_index = depth % MAX_PATH_DEPTH;
        self.path_move_keys[move_index] ^ self.path_depth_keys[depth_index]
    }
}

pub fn hash(board: &Board, rule50: u16) -> u64 {
    let z = Zobrist::get();
    board.hash() ^ z.rule50_keys[rule50.min(100) as usize]
}

pub fn path_random(mv: Move, depth: usize) -> u64 {
    Zobrist::get().path_random(mv, depth)
}
