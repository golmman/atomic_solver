//! Lean plan 8 regression tests.
//!
//! This file exceeds the ~10 KiB guideline because it embeds two verbatim
//! transcriptions of pre-plan8 implementations (the brute-force
//! `nearest_commoner_map` and the full pre-plan8 scorer) as the bit-identical
//! witnesses; keeping them in one place next to the tests that exercise them
//! makes them diffable against git history.
//!
//! Part 1 (phase 1): `nearest_commoner_map` must be output-identical to the
//! original O(64 × k) min-Chebyshev brute-force scan for every input. The
//! brute-force implementation is kept here as a test-only reference (it
//! cannot live in `src/search/ordering.rs` behind `#[cfg(test)]` because
//! integration tests link the library without its test cfg) and compared over
//! random seeded boards with 0–8 enemy commoners, plus the dedicated
//! empty-commoners case.
//!
//! Part 2 (phase 2): the whole pre-plan8 `StaticAtomicScorer` scoring path is
//! transcribed verbatim as a reference scorer and compared per move against
//! the current implementation over random boards and pinned positions, for
//! both OR- and AND-node profiles. This is the primary bit-identical witness
//! for the phase-2 fast paths.

use atomic_movegen::board::Board;
use atomic_movegen::types::{Color, Square};

use atomic_solver::search::ordering::nearest_commoner_map;

/// Deterministic xorshift64* RNG so failures are reproducible from the seed.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below_us(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Reference implementation: the original O(64 × k) Chebyshev-pair scan.
fn brute_force_nearest_commoner_map(board: &Board, them: Color) -> [i8; 64] {
    fn chebyshev(a: Square, b: Square) -> i8 {
        use atomic_movegen::types::{file_of, rank_of};
        let (af, ar) = (i16::from(file_of(a) as u8), i16::from(rank_of(a) as u8));
        let (bf, br) = (i16::from(file_of(b) as u8), i16::from(rank_of(b) as u8));
        let d = (af - bf).abs().max((ar - br).abs());
        i8::try_from(d).unwrap()
    }

    let mut map = [i8::MAX; 64];
    let mut commoners = board.commoners(them);
    while !commoners.is_empty() {
        let c = commoners.pop_lsb();
        for sq in 0..64 {
            let d = chebyshev(Square::from_u8(sq), c);
            if d < map[sq as usize] {
                map[sq as usize] = d;
            }
        }
    }
    map
}

/// Build a board from a piece-placement string (8 ranks, '/'-separated,
/// standard FEN piece letters, digits as empties). Returns `None` when the
/// FEN parser rejects it.
fn board_from_placement(placement: &str) -> Option<Board> {
    let fen = format!("{placement} w - - 0 1");
    Board::from_fen(&fen).ok()
}

#[test]
fn empty_commoners_maps_to_max() {
    for placement in ["8/8/8/8/8/8/8/8", "R7/8/8/8/8/8/8/7R", "N7/8/8/8/8/8/8/7N"] {
        let board = board_from_placement(placement).expect("valid placement");
        for them in [Color::White, Color::Black] {
            let map = nearest_commoner_map(&board, them);
            assert!(
                map.iter().all(|&d| d == i8::MAX),
                "empty enemy commoners must map to i8::MAX (placement={placement}, them={them:?})"
            );
        }
    }
}

#[test]
fn bfs_matches_brute_force_on_random_boards() {
    let mut rng = Rng(0x2026_0914_dead_beef);
    let filler_chars = ["P", "p", "N", "n", "R", "r", "B", "b", "Q"];
    for case in 0..512u32 {
        // 0–8 enemy commoners per board, both colors scanned per board.
        let k: usize = (case % 9) as usize;
        // Exactly k distinct commoner squares, then a sparse random mix of
        // other pieces on the remaining squares (some boards stay sparse so
        // every layer depth is exercised).
        let mut squares: [u8; 64] = core::array::from_fn(|i| i as u8);
        // Deterministic partial Fisher–Yates: shuffle the first `take` slots.
        let take = 8 + k + rng.below_us(16);
        for i in 0..take.min(64) {
            let j = i + rng.below_us(64 - i);
            squares.swap(i, j);
        }
        let mut board_chars = ['1'; 64];
        // FEN spells a commoner 'K' (atomic chess keeps the standard label).
        for &sq in &squares[..k] {
            board_chars[sq as usize] = 'K';
        }
        let fill = rng.below_us(14); // 0–13 extra non-commoner pieces
        for &sq in &squares[k..(k + fill).min(64)] {
            let ch = filler_chars[rng.below_us(filler_chars.len())];
            board_chars[sq as usize] = ch.chars().next().unwrap();
        }
        let placement = (0..8)
            .map(|r| {
                let mut row = String::new();
                let mut run = 0usize;
                for f in 0..8 {
                    let c = board_chars[r * 8 + f];
                    if c == '1' {
                        run += 1;
                    } else {
                        if run > 0 {
                            row.push_str(&run.to_string());
                            run = 0;
                        }
                        row.push(c);
                    }
                }
                if run > 0 {
                    row.push_str(&run.to_string());
                }
                row
            })
            .collect::<Vec<_>>()
            .join("/");
        let board = board_from_placement(&placement)
            .unwrap_or_else(|| panic!("generated placement must parse: {placement}"));
        for them in [Color::White, Color::Black] {
            let fast = nearest_commoner_map(&board, them);
            let reference = brute_force_nearest_commoner_map(&board, them);
            assert_eq!(
                fast, reference,
                "BFS map diverged (case {case}, placement={placement}, them={them:?})"
            );
        }
    }
}

#[test]
fn single_commoner_layers_are_chebyshev() {
    // Dedicated k=1 case: the map must be the Chebyshev distance to the
    // commoner (BFS layer index). "8/.../4K3" puts the commoner on e1
    // (board square 4: file 4, rank 0).
    let board = board_from_placement("8/8/8/8/8/8/8/4K3").expect("valid placement");
    let map = nearest_commoner_map(&board, Color::White);
    for sq in 0..64u8 {
        let (f, r) = (i16::from(sq % 8), i16::from(sq / 8));
        let d = (f - 4).abs().max(r).min(127) as i8;
        assert_eq!(map[sq as usize], d, "square {sq}");
    }
}

#[test]
fn multi_source_bfs_takes_the_minimum() {
    // Two commoners (a8 = square 56 and h1 = square 7 in board indexing):
    // every square must take the closer one.
    let board = board_from_placement("K7/8/8/8/8/8/8/7K").expect("valid placement");
    let map = nearest_commoner_map(&board, Color::White);
    for sq in 0..64u8 {
        let (f, r) = (i16::from(sq % 8), i16::from(sq / 8));
        let d_a8 = f.max(7 - r); // a8: f=0, r=7
        let d_h1 = (7 - f).max(r); // h1: f=7, r=0
        assert_eq!(map[sq as usize], d_a8.min(d_h1) as i8, "square {sq}");
    }
}

// ---------------------------------------------------------------------------
// Part 2: full pre-plan8 scorer reference (differential bit-identical check)
// ---------------------------------------------------------------------------

mod reference {
    //! Verbatim transcription of the pre-plan8 scoring path (old
    //! `capture_net_value` per-square loop, old threat block, old rook block,
    //! old per-pair Chebyshev arithmetic).

    use atomic_movegen::attacks;
    use atomic_movegen::board::{Board, StateInfo};
    use atomic_movegen::types::{Bitboard, Color, Move, NO_PIECE, PieceType, Square};

    use atomic_solver::search::ordering::ScorerParams;

    fn capture_net_value(params: &ScorerParams, board: &Board, m: Move) -> i32 {
        let from = m.from_sq();
        let to = m.to_sq();
        let moving_piece = board.piece_on(from);
        let moving_value = params.piece_value(moving_piece.type_of().unwrap());

        let victim_value = if m.is_en_passant() {
            params.piece_value(PieceType::Pawn)
        } else {
            params.piece_value(board.piece_on(to).type_of().unwrap())
        };

        let mut own_destroyed = moving_value;
        let mut enemy_destroyed = victim_value;

        let blast = attacks::king_attacks(to) & !board.pieces_pt(PieceType::Pawn);

        let mut b = blast;
        while !b.is_empty() {
            let sq = b.pop_lsb();
            if sq == from {
                continue;
            }
            let p = board.piece_on(sq);
            if p == NO_PIECE {
                continue;
            }
            let value = params.piece_value(p.type_of().unwrap());
            if p.color().unwrap() == board.side_to_move() {
                own_destroyed += value;
            } else {
                enemy_destroyed += value;
            }
        }

        enemy_destroyed - own_destroyed
    }

    fn file_rank_of(sq: Square) -> (i8, i8) {
        use atomic_movegen::types::{file_of, rank_of};
        (file_of(sq) as u8 as i8, rank_of(sq) as u8 as i8)
    }

    fn chebyshev(a: Square, b: Square) -> i8 {
        let (af, ar) = file_rank_of(a);
        let (bf, br) = file_rank_of(b);
        (af - bf).abs().max((ar - br).abs())
    }

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

    /// Old `nearest_commoner_map` (pre-plan8 arithmetic scan).
    pub fn nearest_map(board: &Board, them: Color) -> [i8; 64] {
        let mut map = [i8::MAX; 64];
        let mut commoners = board.commoners(them);
        if commoners.is_empty() {
            return map;
        }
        while !commoners.is_empty() {
            let c = commoners.pop_lsb();
            for sq in 0..64 {
                let d = chebyshev(Square::from_u8(sq), c);
                if d < map[sq as usize] {
                    map[sq as usize] = d;
                }
            }
        }
        map
    }

    /// Old `score_with_map` (pre-plan8): context fields recomputed inline.
    pub fn score(
        board: &Board,
        m: Move,
        state: &StateInfo,
        params: &ScorerParams,
        is_or_node: bool,
    ) -> i32 {
        let from = m.from_sq();
        let to = m.to_sq();
        let from_piece = board.piece_on(from);
        if from_piece == NO_PIECE {
            return 0;
        }
        let from_pt = from_piece.type_of().unwrap();
        let us = board.side_to_move();
        let them = us.flip();

        let is_capture = board.is_capture(m);

        // 1. Winning capture.
        if is_capture {
            let blast_zone = attacks::king_attacks(to) | Bitboard::square_bb(to);
            let them_commoners = board.commoners(them);
            if state.them_commoners_count == 1 && (them_commoners & blast_zone) != Bitboard::EMPTY {
                return params.score_winning_capture;
            }
        }

        // 2. Promotion.
        if m.is_promotion() && !is_capture {
            return params.score_promotion + params.piece_value(m.promotion_type());
        }

        // 3. aSEE capture.
        if is_capture {
            let net = capture_net_value(params, board, m);
            return params.score_capture + net * params.capture_net_scale;
        }

        let mut score = 0;

        let (pawn_storm, pawn_storm_step) = if is_or_node {
            (params.score_pawn_storm, params.score_pawn_storm_step)
        } else {
            (
                params.score_pawn_storm * params.and_pawn_storm_scale / 100,
                params.score_pawn_storm_step * params.and_pawn_storm_scale / 100,
            )
        };
        let (rook_open_file, rook_open_file_step, rook_back_rank) = if is_or_node {
            (
                params.score_rook_open_file,
                params.score_rook_open_file_step,
                params.score_rook_back_rank,
            )
        } else {
            (
                params.score_rook_open_file * params.and_rook_attack_scale / 100,
                params.score_rook_open_file_step * params.and_rook_attack_scale / 100,
                params.score_rook_back_rank * params.and_rook_attack_scale / 100,
            )
        };
        let (approach, approach_step, center, center_step, rook_center) = if is_or_node {
            (
                params.score_approach,
                params.score_approach_step,
                params.score_center,
                params.score_center_step,
                params.score_rook_center,
            )
        } else {
            (
                params.score_approach * params.and_approach_scale / 100,
                params.score_approach_step * params.and_approach_scale / 100,
                params.score_center * params.and_approach_scale / 100,
                params.score_center_step * params.and_approach_scale / 100,
                params.score_rook_center * params.and_approach_scale / 100,
            )
        };

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
        let nearest = nearest_map(board, them);

        // 4. Direct commoner threat (old: occupancy rewrite for every mover).
        {
            let from_bb = Bitboard::square_bb(from);
            let to_bb = Bitboard::square_bb(to);
            let new_occupied = (board.occupied() & !from_bb) | to_bb;
            let attack_bb = attacks_from(from_pt, us, to, new_occupied);
            if (attack_bb & them_commoners) != Bitboard::EMPTY {
                let base = if state.them_commoners_count == 1 {
                    params.score_threat_last
                } else {
                    params.score_threat
                };
                let enemy_attackers =
                    board.attackers_to(to, new_occupied) & board.pieces_color(them);
                let bonus = if enemy_attackers.is_empty() {
                    base
                } else {
                    base / 2
                };
                score += bonus;
            }
        }

        // 5. Kamikaze.
        if (attacks::king_attacks(to) & them_commoners) != Bitboard::EMPTY {
            if state.them_commoners_count == 1 {
                score += params.score_kamikaze_last;
            } else {
                score += params.score_kamikaze;
            }
        }

        // 6. Pawn storm.
        if from_pt == PieceType::Pawn
            && let Some(commoner_sq) = lone_commoner
        {
            let from_dist = nearest[from as usize];
            let to_dist = nearest[to as usize];
            if to_dist < from_dist {
                let attack_list = attacks::pawn_attacks(us, to);
                let mut b = attack_list;
                let mut near = false;
                while !b.is_empty() {
                    let sq = b.pop_lsb();
                    if chebyshev(sq, commoner_sq) <= 2 {
                        near = true;
                        break;
                    }
                }
                if near {
                    score += pawn_storm + i32::from(from_dist - to_dist) * pawn_storm_step;
                }
            }
        }

        // 7. Heavy-piece block (old: unconditional first scan).
        if matches!(from_pt, PieceType::Rook | PieceType::Queen)
            && let Some(commoner_sq) = lone_commoner
        {
            let from_dist = nearest[from as usize];
            let to_dist = nearest[to as usize];

            let from_bb = Bitboard::square_bb(from);
            let occupied_without_from = board.occupied() & !from_bb;
            let rook_attacks = attacks::rook_attacks(to, occupied_without_from);
            if (rook_attacks & Bitboard::square_bb(commoner_sq)) == Bitboard::EMPTY {
                let changed_file =
                    atomic_movegen::types::file_of(to) != atomic_movegen::types::file_of(from);
                if changed_file {
                    let file_mask = Bitboard(
                        0x0101_0101_0101_0101u64
                            << u32::from(atomic_movegen::types::file_of(to) as u8),
                    );
                    let occupied_no_own_pawns =
                        board.occupied() & !board.pieces_color_pt(us, PieceType::Pawn) & !from_bb;
                    let rook_attacks_semi = attacks::rook_attacks(to, occupied_no_own_pawns);
                    let enemy_back_rank_pieces =
                        board.pieces_color(them) & back_rank_mask & file_mask;
                    if (rook_attacks_semi & enemy_back_rank_pieces) != Bitboard::EMPTY {
                        let reduction = (from_dist - to_dist).max(0);
                        score += rook_open_file + i32::from(reduction) * rook_open_file_step;
                    }
                }
            } else {
                let reduction = (from_dist - to_dist).max(0);
                score += rook_open_file + i32::from(reduction) * rook_open_file_step;
            }

            if u32::from(to as u8 / 8) == enemy_back_rank && chebyshev(to, commoner_sq) <= 2 {
                score += rook_back_rank;
            }
        }

        // 8. Centralizing / attacking moves.
        let from_dist = nearest[from as usize];
        let to_dist = nearest[to as usize];
        if from_dist < i8::MAX && to_dist < i8::MAX && to_dist < from_dist {
            score += approach + i32::from(from_dist - to_dist) * approach_step;
        }

        let (f, r) = file_rank_of(to);
        let centrality = 3 - (f - 3).abs().max(r - 3).abs();
        if centrality > 0 {
            score += center + i32::from(centrality) * center_step;
            if matches!(from_pt, PieceType::Rook | PieceType::Queen) {
                score += rook_center * i32::from(centrality);
            }
        }

        score
    }
}

/// Differential check: current `StaticAtomicScorer` vs the verbatim pre-plan8
/// reference scorer, over random boards (all legal moves, OR and AND) and the
/// pinned move-order positions.
#[test]
fn scorer_matches_pre_plan8_reference() {
    use atomic_movegen::board::StateInfo;
    use atomic_movegen::movegen::generate_legal;
    use atomic_movegen::types::MoveList;

    use atomic_solver::search::ordering::{ScorerParams, StaticAtomicScorer};

    let scorer = StaticAtomicScorer::default();
    let params = ScorerParams::default();

    let mut rng = Rng(0x2026_0915_cafe_babe);
    let filler_chars = ["P", "p", "N", "n", "R", "r", "B", "b", "Q", "K", "k"];
    let mut boards: Vec<(Board, String)> = Vec::new();

    // Random sparse boards with both colors present.
    for _ in 0..256 {
        let mut squares: [u8; 64] = core::array::from_fn(|i| i as u8);
        let take = 6 + rng.below_us(20);
        for i in 0..take.min(64) {
            let j = i + rng.below_us(64 - i);
            squares.swap(i, j);
        }
        let mut board_chars = ['1'; 64];
        for &sq in &squares[..take.min(64)] {
            let ch = filler_chars[rng.below_us(filler_chars.len())];
            board_chars[sq as usize] = ch.chars().next().unwrap();
        }
        let placement = (0..8)
            .map(|r| {
                let mut row = String::new();
                let mut run = 0usize;
                for f in 0..8 {
                    let c = board_chars[r * 8 + f];
                    if c == '1' {
                        run += 1;
                    } else {
                        if run > 0 {
                            row.push_str(&run.to_string());
                            run = 0;
                        }
                        row.push(c);
                    }
                }
                if run > 0 {
                    row.push_str(&run.to_string());
                }
                row
            })
            .collect::<Vec<_>>()
            .join("/");
        if let Some(board) = board_from_placement(&placement) {
            boards.push((board, placement));
        }
    }

    // Pinned positions from the move-order unit tests.
    for fen in [
        "rnbq1bnr/pppkpQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR w KQ - 2 5",
        "4k3/1P6/8/8/8/8/8/4K3 w - - 0 1",
        "rnbqkbnr/pppp1ppp/8/4p3/8/5N2/PPPPPPPP/RNBQKB1R w KQkq - 0 3",
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "8/8/8/8/4k3/8/2N5/4K3 w - - 0 1",
        "4k3/8/8/1B2p3/8/8/4Q3/4K3 w - - 0 1",
        "4k3/8/8/2p1pr2/3Q4/8/8/4K3 w - - 0 1",
        "1n2k3/P7/8/8/8/8/8/4K3 w - - 0 1",
        "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22",
        "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21",
        "8/8/8/8/2k5/8/8/4KR2 w - - 0 1",
        "8/8/8/8/8/8/8/K6k w - - 0 1",
    ] {
        if let Ok(board) = Board::from_fen(fen) {
            boards.push((board, fen.to_string()));
        }
    }

    let mut compared = 0usize;
    for (board, label) in &boards {
        let mut state = StateInfo::new();
        board.populate_state(&mut state);
        let mut moves = MoveList::new();
        generate_legal(board, &mut moves);
        for i in 0..moves.len() {
            let m = moves[i];
            // The current implementation builds its context from
            // `nearest_commoner_map`; feed the same map it uses.
            let map = atomic_solver::search::ordering::nearest_commoner_map(
                board,
                board.side_to_move().flip(),
            );
            for is_or_node in [true, false] {
                let got = scorer.score_with_map(board, m, &state, &map, is_or_node);
                let want = reference::score(board, m, &state, &params, is_or_node);
                assert_eq!(
                    got,
                    want,
                    "score drift: board={label} move={} or={is_or_node} got={got} want={want}",
                    m.to_uci()
                );
            }
            compared += 1;
        }
    }
    assert!(
        compared > 1000,
        "differential check must cover real ground: {compared}"
    );
}
