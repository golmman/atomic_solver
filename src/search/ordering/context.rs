//! Per-node scoring context and distance geometry for the static scorer.
//!
//! Split out of `ordering.rs` (plan 8): the tables and the context builder
//! are data, not scoring logic, and `ordering.rs` would otherwise exceed the
//! ~20 KB split guideline.

use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::types::{Bitboard, Color, Square};

/// `CHEBYSHEV[a][b]` is the Chebyshev distance between squares `a` and `b`,
/// const-evaluated once at compile time (4 KiB of `.rodata`; one row is the
/// 64-byte distance field of a single source square).
static CHEBYSHEV: [[i8; 64]; 64] = chebyshev_table();

const fn chebyshev_table() -> [[i8; 64]; 64] {
    let mut table = [[0i8; 64]; 64];
    let mut a = 0;
    while a < 64 {
        let (af, ar) = ((a % 8) as i8, (a / 8) as i8);
        let mut b = 0;
        while b < 64 {
            let (bf, br) = ((b % 8) as i8, (b / 8) as i8);
            let df = if af - bf < 0 { bf - af } else { af - bf };
            let dr = if ar - br < 0 { br - ar } else { ar - br };
            table[a][b] = if df > dr { df } else { dr };
            b += 1;
        }
        a += 1;
    }
    table
}

/// Chebyshev distance between two squares (const-table lookup).
#[must_use]
pub(crate) fn chebyshev(a: Square, b: Square) -> i8 {
    CHEBYSHEV[a as usize][b as usize]
}

/// `QUEEN_RAYS[to]` is every square sharing a file, rank, or diagonal with
/// `to` (the union of the rook and bishop rays), const-evaluated (512 B of
/// `.rodata`).
static QUEEN_RAYS: [u64; 64] = queen_rays_table();

const fn queen_rays_table() -> [u64; 64] {
    const DIRS: [(i8, i8); 8] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    let mut table = [0u64; 64];
    let mut sq = 0;
    while sq < 64 {
        let (f0, r0) = ((sq % 8) as i8, (sq / 8) as i8);
        let mut mask = 0u64;
        let mut di = 0;
        while di < 8 {
            let (df, dr) = DIRS[di];
            let (mut f, mut r) = (f0 + df, r0 + dr);
            while f >= 0 && f < 8 && r >= 0 && r < 8 {
                mask |= 1u64 << (r * 8 + f);
                f += df;
                r += dr;
            }
            di += 1;
        }
        table[sq] = mask;
        sq += 1;
    }
    table
}

/// Queen-ray mask through `sq` (used as an exact alignment pre-filter for
/// sliding-piece attack queries).
#[must_use]
pub(crate) fn queen_rays(sq: Square) -> Bitboard {
    Bitboard(QUEEN_RAYS[sq as usize])
}

/// Precompute the nearest enemy commoner distance for every square.
///
/// If the opponent has no commoners, every entry is set to `i8::MAX`.
///
/// Distances come from the const `CHEBYSHEV` table instead of per-pair
/// arithmetic. The dominant case is exactly one enemy commoner (the enemy
/// king), where the map is a single 64-byte table-row copy; with k ≥ 2 the
/// rows are min-combined per source (vectorizable). This replaces the former
/// O(64 × k) per-pair arithmetic scan; output is identical for every input,
/// including the all-`i8::MAX` map when the enemy has no commoners. (A
/// multi-source BFS over king-move adjacency was measured first per the plan
/// and rejected: at k = 1 it is ~8% slower wall on both validation cases.)
#[must_use]
pub fn nearest_commoner_map(board: &Board, them: Color) -> [i8; 64] {
    let mut map = [i8::MAX; 64];
    let mut commoners = board.commoners(them);
    if commoners.is_empty() {
        return map;
    }
    if commoners.count() == 1 {
        // Single source: the map is exactly that square's distance row.
        let c = commoners.pop_lsb();
        return CHEBYSHEV[c as usize];
    }
    while !commoners.is_empty() {
        let c = commoners.pop_lsb() as usize;
        let row = &CHEBYSHEV[c];
        for (i, d) in map.iter_mut().enumerate() {
            *d = (*d).min(row[i]);
        }
    }
    map
}

/// Per-node invariants for the static scorer, hoisted out of the per-move
/// scoring loop.
///
/// Every field is constant across all moves of one node, so building the
/// context once per node (in `sort_moves`) replaces the per-move
/// recomputation of `board.commoners(them)`, the lone-commoner bit scan, and
/// the enemy back-rank mask. The values are identical to what
/// [`crate::search::ordering::StaticAtomicScorer::score_with_map`] would
/// have computed per move.
pub(crate) struct ScoreContext {
    pub us: Color,
    pub them: Color,
    /// Enemy commoners bitboard (`board.commoners(them)`).
    pub them_commoners: Bitboard,
    /// `them_commoners.count()`, equal to `state.them_commoners_count`.
    pub them_commoners_count: u32,
    /// The single enemy commoner square when exactly one exists.
    pub lone_commoner: Option<Square>,
    /// Mask of the enemy back rank (rank 8 for White to move, rank 1 for Black).
    pub enemy_back_rank: u32,
    pub back_rank_mask: Bitboard,
    /// Nearest enemy commoner Chebyshev distance per square.
    pub nearest: [i8; 64],
}

impl ScoreContext {
    /// Build the per-node invariants for `board` + `state` from a precomputed
    /// `nearest` map (see [`nearest_commoner_map`]).
    ///
    /// `state` must be the `StateInfo` of `board` (its `them_commoners_count`
    /// field is reused, exactly as the per-move path did).
    #[must_use]
    pub(crate) fn build(board: &Board, state: &StateInfo, nearest: [i8; 64]) -> Self {
        let us = board.side_to_move();
        let them = us.flip();
        let them_commoners = board.commoners(them);
        let lone_commoner = if state.them_commoners_count == 1 {
            let mut c = them_commoners;
            let sq = c.pop_lsb();
            if sq == Square::NONE { None } else { Some(sq) }
        } else {
            None
        };
        let enemy_back_rank = if us == Color::White { 7u32 } else { 0u32 };
        let back_rank_mask = Bitboard(0xFFu64 << (enemy_back_rank * 8));
        Self {
            us,
            them,
            them_commoners,
            them_commoners_count: state.them_commoners_count,
            lone_commoner,
            enemy_back_rank,
            back_rank_mask,
            nearest,
        }
    }
}
