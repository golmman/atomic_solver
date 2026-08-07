//! Deterministic Zobrist hashing for atomic-chess positions.
//!
//! Uses `Board::hash()` for the piece/side/castling/en-passant component and
//! adds a rule50 key for transposition-table lookup.

use atomic_movegen::board::Board;

pub const INF: u64 = 1 << 60;

const RULE50_KEY_COUNT: usize = 101;
const RULE50_KEY_SEED: u64 = 0x9e37_79b9_7f4a_7c15;

/// A single 64-bit SplitMix64 mixing round.
/// This is a bijection on `u64`, so each distinct input maps to a distinct output.
const fn mix(z: u64) -> u64 {
    let z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Advance the SplitMix64 state and return the next output.
const fn splitmix64_next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(RULE50_KEY_SEED);
    mix(*state)
}

const fn generate_rule50_keys() -> [u64; RULE50_KEY_COUNT] {
    let mut keys = [0u64; RULE50_KEY_COUNT];
    let mut state = RULE50_KEY_SEED;
    let mut i = 0;
    while i < RULE50_KEY_COUNT {
        keys[i] = splitmix64_next(&mut state);
        i += 1;
    }
    keys
}

/// Precomputed Zobrist keys for the halfmove clock.
pub const RULE50_KEYS: [u64; RULE50_KEY_COUNT] = generate_rule50_keys();

pub fn rule50_key(rule50: u16) -> u64 {
    RULE50_KEYS[rule50.min(100) as usize]
}

pub fn hash(board: &Board, rule50: u16) -> u64 {
    board.hash() ^ rule50_key(rule50)
}

/// Board-only hash, ignoring the halfmove clock.  This is the same board
/// representation for the purpose of repetition detection: a position reached
/// by reversible moves with a higher `rule50` is a repetition.
pub fn board_hash(board: &Board) -> u64 {
    board.hash()
}
