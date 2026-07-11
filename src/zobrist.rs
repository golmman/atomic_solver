//! Deterministic Zobrist hashing for atomic-chess positions.

use atomic_movegen::board::Board;
use atomic_movegen::types::{Color, NO_PIECE};
use std::sync::OnceLock;

pub const INF: u64 = 1 << 60;

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
    piece_keys: [[[u64; 64]; 6]; 2],
    side_key: u64,
    castling_keys: [u64; 4],
    ep_file_keys: [u64; 8],
}

impl Zobrist {
    fn new() -> Self {
        let mut rng = SplitMix64(0x9e3779b97f4a7c15);
        let mut piece_keys = [[[0u64; 64]; 6]; 2];
        for pt_arr in piece_keys.iter_mut() {
            for sq_arr in pt_arr.iter_mut() {
                for key in sq_arr.iter_mut() {
                    *key = rng.next();
                }
            }
        }
        let side_key = rng.next();
        let mut castling_keys = [0u64; 4];
        for key in castling_keys.iter_mut() {
            *key = rng.next();
        }
        let mut ep_file_keys = [0u64; 8];
        for key in ep_file_keys.iter_mut() {
            *key = rng.next();
        }
        Self {
            piece_keys,
            side_key,
            castling_keys,
            ep_file_keys,
        }
    }

    fn get() -> &'static Self {
        ZOBRIST.get_or_init(Zobrist::new)
    }
}

pub fn hash(board: &Board) -> u64 {
    let z = Zobrist::get();
    let mut h = 0u64;

    if board.side_to_move() == Color::Black {
        h ^= z.side_key;
    }

    let mut occ = board.occupied();
    while !occ.is_empty() {
        let sq = occ.pop_lsb();
        let p = board.piece_on(sq);
        if p != NO_PIECE {
            let c = p.color() as usize;
            let pt = p.type_of() as usize;
            let sq_idx = sq as u8 as usize;
            h ^= z.piece_keys[c][pt][sq_idx];
        }
    }

    let cr = board.castling_rights();
    for i in 0..4 {
        if cr & (1 << i) != 0 {
            h ^= z.castling_keys[i];
        }
    }

    if let Some(ep) = board.ep_square() {
        use atomic_movegen::types::file_of;
        h ^= z.ep_file_keys[file_of(ep) as u8 as usize];
    }

    h
}
