//! 3-man atomic WDL table core (egtb `plan1` Task B).
//!
//! This file is larger than the 20 KiB guideline because the index layout,
//! FEN synthesis, terminal classification, child-reference resolution,
//! fixpoint sweep, symmetry self-check, and raw-file encoding all operate on
//! the same flat-table invariant (`index` ⇔ placement); splitting them would
//! force the placement type across module boundaries for no gain in a
//! throwaway format sketch.
//!
//! Generates K+x vs K tables (x ∈ {Q, R, B, N, P}) for both strong-side
//! colors over `atomic_movegen::board::Board` semantics, as a forward value
//! iteration to fixpoint ("retrograde" in effect: iterate until no outcome
//! changes). This is a throwaway format sketch for the go/no-go spike; the
//! plan2 storage format will need block compression, shared king-square
//! indexing, and a real header.
//!
//! Index layout (per material class, one flat vector):
//! `index = (((strong * 2 + stm) * 64 + sk) * 64 + wk) * 64 + xs` with
//! `strong` the color owning king+man, `stm` the side to move, `sk`/`wk` the
//! two king squares and `xs` the extra man's square. `2·2·64³ = 1 MiB`
//! entries per class.
//!
//! Ruleset grounding (deviation from the plan1 sketch, see report1): base
//! cases use the solver's terminal classification
//! (`Position::outcome_from_state` minus rule50, WDL at clock 0) — **not**
//! `Board::outcome()`, which classifies any moves-empty position as a draw
//! and therefore disagrees with the solver on checkmate. The generator
//! counts these divergences on a 1-in-64 sample for the report.

use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::movegen::generate_legal;
use atomic_movegen::types::{Color, Move, MoveList, Outcome, PieceType};

/// In-RAM value codes (unknown must be distinct from every real value).
pub const U_UNKNOWN: u8 = 0;
pub const U_LOSS: u8 = 1;
pub const U_DRAW: u8 = 2;
pub const U_WIN: u8 = 3;
pub const U_INVALID: u8 = 4;

/// Generation order: pawn last, so its promotion children resolve against
/// the finished Q/R/B/N tables.
pub const CLASS_ORDER: [PieceType; 5] = [
    PieceType::Queen,
    PieceType::Rook,
    PieceType::Bishop,
    PieceType::Knight,
    PieceType::Pawn,
];
pub const CLASS_CHARS: [char; 5] = ['q', 'r', 'b', 'n', 'p'];

/// Entries per material class: 2 strong colors × 2 stm × 64³ placements.
pub const NB: usize = 2 * 2 * 64 * 64 * 64;

/// Packed child reference tags (top two bits).
const TAG_CROSS: u32 = 0x4000_0000;
const TAG_CONST: u32 = 0x8000_0000;
const IDX_MASK: u32 = 0x0FFF_FFFF;

#[inline]
pub fn index(strong: usize, stm: usize, sk: usize, wk: usize, xs: usize) -> usize {
    (((strong * 2 + stm) * 64 + sk) * 64 + wk) * 64 + xs
}

/// Inverse of [`index`].
#[must_use]
pub fn decode_index(i: usize) -> (usize, usize, usize, usize, usize) {
    let xs = i % 64;
    let wk = (i / 64) % 64;
    let sk = (i / 4096) % 64;
    let rest = i / 262_144;
    (rest / 2, rest % 2, sk, wk, xs)
}

fn piece_char(pt: PieceType, white: bool) -> u8 {
    let c = match pt {
        PieceType::Queen => b'q',
        PieceType::Rook => b'r',
        PieceType::Bishop => b'b',
        PieceType::Knight => b'n',
        PieceType::Pawn => b'p',
        _ => b'k',
    };
    if white { c - 32 } else { c }
}

/// Build the FEN for one placement (castling/ep empty, clock 0).
pub fn fen_for(
    strong: usize,
    stm: usize,
    sk: usize,
    wk: usize,
    xs: usize,
    pt: PieceType,
) -> String {
    let mut grid = [b'.'; 64];
    let strong_white = strong == 0;
    grid[sk] = if strong_white { b'K' } else { b'k' };
    grid[wk] = if strong_white { b'k' } else { b'K' };
    grid[xs] = piece_char(pt, strong_white);
    let mut s = String::with_capacity(80);
    for r in (0..8).rev() {
        let mut run = 0;
        for f in 0..8 {
            if grid[r * 8 + f] == b'.' {
                run += 1;
            } else {
                if run > 0 {
                    s.push_str(itoa(run));
                    run = 0;
                }
                s.push(grid[r * 8 + f] as char);
            }
        }
        if run > 0 {
            s.push_str(itoa(run));
        }
        if r > 0 {
            s.push('/');
        }
    }
    s.push(' ');
    s.push(if stm == 0 { 'w' } else { 'b' });
    s.push_str(" - - 0 1");
    s
}

fn itoa(v: usize) -> &'static str {
    match v {
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        7 => "7",
        8 => "8",
        _ => unreachable!("run length 1..=8"),
    }
}

/// Whether the placement is representable: distinct squares; pawns excluded
/// from their promotion/back ranks.
#[must_use]
pub fn valid_placement(pt: PieceType, sk: usize, wk: usize, xs: usize) -> bool {
    if sk == wk || xs == sk || xs == wk {
        return false;
    }
    if pt == PieceType::Pawn {
        let rank = xs / 8; // Square is rank*8 + file
        return (1..=6).contains(&rank);
    }
    true
}

/// Solver-equivalent terminal classification at clock 0 (rule50 excluded by
/// construction): extinction, moves-empty with the checkers-bit mate/stalemate
/// split, then the board-static `KvK` draw. Returns
/// `(value, is_checkmate, is_stalemate)` on terminal positions.
pub fn classify_terminal(board: &Board) -> Option<(u8, bool, bool)> {
    let us = board.side_to_move();
    if board.commoners(us).is_empty() {
        return Some((U_LOSS, false, false));
    }
    if board.commoners(us.flip()).is_empty() {
        return Some((U_WIN, false, false));
    }
    let mut moves = MoveList::new();
    generate_legal(board, &mut moves);
    if moves.is_empty() {
        if board.checkers().is_empty() {
            return Some((U_DRAW, false, true));
        }
        return Some((U_LOSS, true, false));
    }
    if board.occupied().count() == 2 {
        return Some((U_DRAW, false, false));
    }
    None
}

/// Generation statistics for one material class (both strong colors).
#[derive(Default)]
pub struct GenStats {
    pub wins: [u64; 2],
    pub losses: [u64; 2],
    pub draws: [u64; 2],
    pub invalid: u64,
    pub checkmates: [u64; 2],
    pub stalemates: [u64; 2],
    /// Placements where the side *not* to move is attacked (unreachable in
    /// play; kept in the table but excluded from cross-validation sampling).
    pub illegal: [u64; 2],
    pub max_dtm: u32,
    pub passes: u32,
    pub child_refs: u64,
    pub unknown_left: u64,
    pub outcome_divergences: u64,
    pub outcome_samples: u64,
    pub symmetry_mismatches: u64,
}

pub struct Table3 {
    pub class: usize,
    pub value: Vec<u8>,
    /// Plies from the decided position to the end at decision time (0 for
    /// terminal base cases); curiosity metric only, no DTM claim. Retained
    /// on the struct (never read after generation) because the dtm array is
    /// part of the format sketch plan2 will extend.
    #[allow(dead_code)]
    pub dtm: Vec<u8>,
    /// CSR child references: entry `i`'s children are
    /// `children[child_off[i]..child_off[i + 1]]`. Retained (never read
    /// after generation) as the format-sketch working set; memory is
    /// reported via `stats.child_refs`.
    #[allow(dead_code)]
    pub child_off: Vec<u32>,
    #[allow(dead_code)]
    pub children: Vec<u32>,
    pub stats: GenStats,
}

impl Table3 {
    /// Generate the table for `class` (an index into [`CLASS_ORDER`]). For
    /// the pawn class, `deps` must hold the finished Q/R/B/N tables indexed
    /// by their `CLASS_ORDER` position; for the other classes it stays empty
    /// (their move graphs are closed — no captures exist at 3 men).
    pub fn build(class: usize, deps: &[&Table3]) -> Table3 {
        let pt = CLASS_ORDER[class];
        let mut value = vec![U_UNKNOWN; NB];
        let mut dtm = vec![0u8; NB];
        let mut stats = GenStats::default();

        // Phase 0: invalid placements and terminal base cases.
        for strong in 0..2 {
            for stm in 0..2 {
                for sk in 0..64 {
                    for wk in 0..64 {
                        for xs in 0..64 {
                            let i = index(strong, stm, sk, wk, xs);
                            if !valid_placement(pt, sk, wk, xs) {
                                value[i] = U_INVALID;
                                stats.invalid += 1;
                                continue;
                            }
                            let board =
                                Board::from_fen(&fen_for(strong, stm, sk, wk, xs, pt)).unwrap();
                            // Legality: the side *not* to move must not be
                            // attackable. Probed on the same placement with
                            // the other side to move (checkers() is computed
                            // for the side to move there).
                            let flipped =
                                Board::from_fen(&fen_for(strong, 1 - stm, sk, wk, xs, pt)).unwrap();
                            if !flipped.checkers().is_empty() {
                                stats.illegal[strong] += 1;
                            }
                            if let Some((v, mate, stale)) = classify_terminal(&board) {
                                value[i] = v;
                                if mate {
                                    stats.checkmates[strong] += 1;
                                } else if stale {
                                    stats.stalemates[strong] += 1;
                                }
                                if (sk ^ wk ^ xs) % 32 == 0 {
                                    // 1-in-64 sample: document
                                    // `Board::outcome()` divergence from the
                                    // solver's mate/stalemate semantics.
                                    stats.outcome_samples += 1;
                                    let expected = match v {
                                        U_LOSS => Some(Outcome::Loss),
                                        U_DRAW => Some(Outcome::Draw),
                                        _ => Some(Outcome::Win),
                                    };
                                    if board.outcome() != expected {
                                        stats.outcome_divergences += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Phase 1: child-reference lists (CSR) for non-terminal entries. The
        // offset of every entry is filled (skipped entries get the running
        // count) so `child_off[i]..child_off[i + 1]` is always well-formed.
        let mut child_off = vec![0u32; NB + 1];
        let mut children: Vec<u32> = Vec::new();
        for strong in 0..2 {
            for stm in 0..2 {
                for sk in 0..64 {
                    for wk in 0..64 {
                        for xs in 0..64 {
                            let i = index(strong, stm, sk, wk, xs);
                            child_off[i] = children.len() as u32;
                            if value[i] != U_UNKNOWN {
                                continue;
                            }
                            child_off[i] = children.len() as u32;
                            let board =
                                Board::from_fen(&fen_for(strong, stm, sk, wk, xs, pt)).unwrap();
                            let mut moves = MoveList::new();
                            generate_legal(&board, &mut moves);
                            for mi in 0..moves.len() {
                                children
                                    .push(resolve_child(&board, moves[mi], class, strong, deps));
                            }
                        }
                    }
                }
            }
        }
        child_off[NB] = children.len() as u32;
        stats.child_refs = children.len() as u64;

        // Phase 2: value iteration to fixpoint. A position is a Win as soon
        // as one child is a Loss (child perspective); a Loss once every child
        // is a Win and none is unresolved; everything left at the fixpoint is
        // a Draw.
        let mut pass: u32 = 0;
        loop {
            pass += 1;
            let mut changed = 0u64;
            for i in 0..NB {
                if value[i] != U_UNKNOWN {
                    continue;
                }
                let range = child_off[i] as usize..child_off[i + 1] as usize;
                if range.is_empty() {
                    // Cannot happen for a non-terminal position; guard against
                    // a silent all-win misclassification anyway.
                    continue;
                }
                let mut any_loss = false;
                let mut any_unknown = false;
                let mut all_win = true;
                for &r in &children[range] {
                    let v = if r & TAG_CONST != 0 {
                        (r & 0xFF) as u8
                    } else if r & TAG_CROSS != 0 {
                        deps[((r >> 28) & 3) as usize].value[(r & IDX_MASK) as usize]
                    } else {
                        value[r as usize]
                    };
                    match v {
                        U_LOSS => any_loss = true,
                        U_UNKNOWN => {
                            any_unknown = true;
                            all_win = false;
                        }
                        U_WIN => {}
                        _ => all_win = false,
                    }
                }
                if any_loss {
                    value[i] = U_WIN;
                    dtm[i] = pass.min(u32::from(u8::MAX)) as u8;
                    changed += 1;
                } else if all_win && !any_unknown {
                    value[i] = U_LOSS;
                    dtm[i] = pass.min(u32::from(u8::MAX)) as u8;
                    changed += 1;
                }
            }
            if changed == 0 {
                break;
            }
        }
        stats.passes = pass;
        for i in 0..NB {
            match value[i] {
                U_WIN => {
                    stats.wins[decode_index(i).0] += 1;
                    stats.max_dtm = stats.max_dtm.max(u32::from(dtm[i]));
                }
                U_LOSS => {
                    stats.losses[decode_index(i).0] += 1;
                    stats.max_dtm = stats.max_dtm.max(u32::from(dtm[i]));
                }
                U_DRAW => stats.draws[decode_index(i).0] += 1,
                U_UNKNOWN => {
                    value[i] = U_DRAW;
                    stats.draws[decode_index(i).0] += 1;
                    stats.unknown_left += 1;
                }
                _ => {}
            }
        }

        // Color-symmetry self-check: flipping colors + vertical mirroring is
        // an automorphism of the rules, so *values* must agree. (dtm is a
        // Gauss-Seidel pass counter, not a mate distance: it legitimately
        // differs between the two strong-color blocks and is not compared.)
        let mut sym_bad = 0u64;
        for strong in 0..2 {
            for stm in 0..2 {
                for sk in 0..64usize {
                    for wk in 0..64usize {
                        for xs in 0..64usize {
                            let i = index(strong, stm, sk, wk, xs);
                            if value[i] == U_INVALID {
                                continue;
                            }
                            let j = index(1 - strong, 1 - stm, sk ^ 56, wk ^ 56, xs ^ 56);
                            if value[i] != value[j] {
                                sym_bad += 1;
                            }
                        }
                    }
                }
            }
        }
        stats.symmetry_mismatches = sym_bad;

        Table3 {
            class,
            value,
            dtm,
            child_off,
            children,
            stats,
        }
    }

    /// Table value for one entry index.
    #[must_use]
    pub fn value_at(&self, i: usize) -> u8 {
        self.value[i]
    }

    /// Free the format-sketch working set (child CSR lists). Only `value`
    /// is needed after this table's dump — later classes' deps read `value`
    /// only. Called between materials to keep the whole run's footprint low.
    pub fn drop_child_lists(&mut self) {
        self.child_off = Vec::new();
        self.children = Vec::new();
    }

    /// Raw byte-per-entry WDL file: 0 = Loss, 1 = Draw, 2 = Win, 3 = invalid.
    pub fn write_raw(&self, path: &std::path::Path) -> std::io::Result<usize> {
        let buf: Vec<u8> = self
            .value
            .iter()
            .map(|&v| match v {
                U_LOSS => 0u8,
                U_DRAW => 1,
                U_WIN => 2,
                _ => 3,
            })
            .collect();
        std::fs::write(path, &buf)?;
        Ok(buf.len())
    }
}

/// Resolve one legal child of a 3-man `parent` into a packed reference:
/// terminal constants, a same-table index, or a cross-class index (pawn
/// promotions into `deps`).
fn resolve_child(parent: &Board, m: Move, class: usize, strong: usize, deps: &[&Table3]) -> u32 {
    let mut child = parent.clone();
    let mut state = StateInfo::new();
    child.do_move(m, &mut state);
    if child.occupied().count() == 2 {
        return TAG_CONST | u32::from(U_DRAW);
    }
    let cus = child.side_to_move();
    if child.commoners(cus).is_empty() {
        return TAG_CONST | u32::from(U_LOSS);
    }
    if child.commoners(cus.flip()).is_empty() {
        return TAG_CONST | u32::from(U_WIN);
    }
    let strong_c = if strong == 0 {
        Color::White
    } else {
        Color::Black
    };
    let sk = child.commoners(strong_c).lsb() as usize;
    let wk = child.commoners(strong_c.flip()).lsb() as usize;
    for (cid, t) in CLASS_ORDER.iter().enumerate() {
        let bb = child.pieces_color_pt(strong_c, *t);
        if !bb.is_empty() {
            let idx = index(strong, cus as usize, sk, wk, bb.lsb() as usize);
            if cid == class {
                return idx as u32;
            }
            debug_assert!(!deps.is_empty(), "cross-class child without deps");
            return TAG_CROSS | ((cid as u32) << 28) | idx as u32;
        }
    }
    unreachable!("a non-terminal 3-man child always has a strong extra man");
}

/// Whether the placement with side to move `stm` is a legal position (the
/// side not to move is not attackable). Used to exclude unreachable
/// placements from cross-validation sampling.
#[must_use]
pub fn position_is_legal(
    strong: usize,
    stm: usize,
    sk: usize,
    wk: usize,
    xs: usize,
    pt: PieceType,
) -> bool {
    let flipped = Board::from_fen(&fen_for(strong, 1 - stm, sk, wk, xs, pt)).unwrap();
    flipped.checkers().is_empty()
}

/// Map a FEN of a 3-man position onto `(class, entry index)` in the generated
/// space, or `None` if it is not a K+x vs K position of this class. Used by
/// the cross-validation runner for explicit edge-case FENs.
#[must_use]
pub fn resolve_fen(class: usize, fen: &str) -> Option<usize> {
    let board = Board::from_fen(fen).ok()?;
    if board.occupied().count() != 3 {
        return None;
    }
    let strong_c = if board.pieces_color(Color::White).count() == 2 {
        Color::White
    } else {
        Color::Black
    };
    let stm = board.side_to_move() as usize;
    let sk = board.commoners(strong_c).lsb() as usize;
    let wk = board.commoners(strong_c.flip()).lsb() as usize;
    for (cid, t) in CLASS_ORDER.iter().enumerate() {
        let bb = board.pieces_color_pt(strong_c, *t);
        if !bb.is_empty() {
            if cid != class {
                return None;
            }
            return Some(index(strong_c as usize, stm, sk, wk, bb.lsb() as usize));
        }
    }
    None
}
