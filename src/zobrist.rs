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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;
    use atomic_movegen::board::Board;

    #[test]
    fn hash_is_deterministic_for_same_position() {
        let board = Board::from_fen(Position::STARTPOS_FEN).unwrap();
        assert_eq!(hash(&board, 0), hash(&board, 0));
    }

    #[test]
    fn hash_differs_for_distinct_placements() {
        let a = Board::from_fen(Position::STARTPOS_FEN).unwrap();
        let b =
            Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1").unwrap();
        assert_ne!(hash(&a, 0), hash(&b, 0), "side to move should change hash");
    }

    #[test]
    fn hash_includes_rule50() {
        let board = Board::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert_ne!(hash(&board, 0), hash(&board, 10));
    }

    #[test]
    fn incremental_zobrist_matches_full_hash_after_random_game() {
        let mut board = Board::from_fen(Position::STARTPOS_FEN).unwrap();
        // Use a fixed PRNG sequence.
        let mut rng = 0x1234_5678_9abc_def0u64;

        // Play 50 random plies from the start position.
        for _ in 0..50 {
            let mut moves = atomic_movegen::types::MoveList::new();
            atomic_movegen::movegen::generate_legal(&board, &mut moves);
            if moves.is_empty() {
                break;
            }
            rng = rng
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let idx = (rng as usize) % moves.len();
            let mv = moves[idx];
            let mut si = atomic_movegen::board::StateInfo::new();
            board.do_move(mv, &mut si);
            assert_eq!(
                board.hash() ^ rule50_key(board.rule50()),
                hash(&board, board.rule50()),
                "incremental board hash must equal full zobrist hash"
            );
        }
    }
}
