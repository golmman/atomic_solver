//! Region-closure fixpoint over the reachable board-only region (plan13,
//! architecture R).
//!
//! The closure is a forward BFS over *exact* packed position keys (no hash
//! collisions are possible), restricted to positions with at most 3 men, no
//! pawns, and no castling rights — the pre-flight detector's class. Within
//! that class the only state that can affect movegen is (piece placement,
//! side to move): an en-passant capture needs a pawn and castling rights are
//! excluded by the detector, so the board-only abstraction is exact for
//! movegen. The halfmove clock is deliberately excluded; see the rank guard
//! in [`super`] for how rule50 is kept sound anyway.
//!
//! Values are computed from the side-to-move perspective (matching
//! `Outcome`): a node is `WIN` iff some child is `LOSS`, `LOSS` iff all
//! children are `WIN`, and the fixpoint remainder is `DRAW`. Terminal
//! classification mirrors `Position::outcome_from_state` without the rule50
//! branch (classification happens at clock 0).
//!
//! Termination of the undecided remainder as `DRAW` is the monotonicity
//! lemma (plan12 T1b): repetition and rule50 semantics only replace outcomes
//! with draws, so a board-only Draw is a genuine Draw under the solver's
//! semantics, and no claim can win the board-only game's refutation.
//!
//! This file is larger than the 10 KiB guideline because the packed-key
//! codec, the closure BFS, the exact-rank fixpoint, and the strategy/PV
//! extraction all operate on the same region arrays (`keys`, `children`,
//! `off`, `val`, `dist`); splitting them would scatter the invariants over
//! the shared indexing scheme. It stays under the ~20 KiB split threshold.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::movegen::generate_legal;
use atomic_movegen::types::{MoveList, Piece};

use crate::position::Outcome;

pub(super) const UNDECIDED: u8 = 0;
pub(super) const WIN: u8 = 1;
pub(super) const LOSS: u8 = 2;
pub(super) const DRAW: u8 = 3;

/// Why the closure/fixpoint aborted.
#[derive(Debug, Clone, Copy)]
pub(super) struct Abort {
    pub reason: &'static str,
    pub evals: u64,
    pub region: usize,
}

/// The closed region plus its exact-rank fixpoint.
#[derive(Debug)]
pub(super) struct Analysis {
    /// Region size (= `keys.len()` = number of BFS expansions).
    pub region: usize,
    /// Child evaluations spent by the closure BFS (clone + `do_move` + key).
    pub evals: u64,
    /// Packed keys in BFS discovery order (index = node id).
    pub keys: Vec<u32>,
    /// Terminal outcome per node (clock-0 board-only classification).
    pub terminal: Vec<Option<Outcome>>,
    /// Fixpoint value per node (`WIN`/`LOSS`/`DRAW`).
    pub val: Vec<u8>,
    /// Exact mate/loss distance per decided node (0 at terminals).
    pub dist: Vec<u32>,
    /// Children arena; node `i`'s children are `children[off[i]..off[i+1]]`.
    pub children: Vec<u32>,
    /// Children-arena offsets (`len == keys.len() + 1`).
    pub off: Vec<u32>,
}

impl Analysis {
    pub(super) fn children_of(&self, idx: u32) -> &[u32] {
        let start = self.off[idx as usize] as usize;
        let end = self.off[idx as usize + 1] as usize;
        &self.children[start..end]
    }

    /// Outcome for the root's side to move, from the fixpoint value.
    pub(super) fn root_outcome(&self) -> Outcome {
        match self.val[0] {
            WIN => Outcome::Win,
            LOSS => Outcome::Loss,
            _ => Outcome::Draw,
        }
    }

    /// Exact distance to the terminal for a decided root (`dist[0]`).
    pub(super) fn root_rank(&self) -> u32 {
        self.dist[0]
    }
}

/// Raw piece code used in the packed key: `(color << 3) | (type + 1)` — the
/// same layout as `atomic_movegen::types::Piece`'s internal encoding (whose
/// raw accessor is crate-private), so it is re-derived from the public API.
fn piece_code(p: Piece) -> u8 {
    let color = p.color().expect("piece on occupied square") as u8;
    let pt = p.type_of().expect("piece on occupied square") as u8;
    (color << 3) | (pt + 1)
}

fn code_piece(code: u8) -> (u8, u8) {
    (code >> 3, (code & 7) - 1)
}

/// Exact, collision-free packed key for a position with at most 3 pieces.
///
/// Layout: bit 0 = side to move; then up to three 10-bit slots (bits
/// 1/11/21), each `(square << 4) | piece_code`, assigned in ascending square
/// order. Slots beyond the piece count are zero. Returns `None` for
/// positions with more than 3 pieces (the detector never lets these in; the
/// `Option` is defense-in-depth).
pub(crate) fn region_key(board: &Board) -> Option<u32> {
    let occupied = board.occupied();
    if occupied.count() > 3 {
        return None;
    }
    let mut slots: Vec<(u8, u8)> = Vec::with_capacity(occupied.count() as usize);
    let mut occ = occupied;
    while !occ.is_empty() {
        let sq = occ.pop_lsb();
        slots.push((sq as u8, piece_code(board.piece_on(sq))));
    }
    slots.sort_unstable();
    let mut key = board.side_to_move() as u32;
    for (i, (sq, code)) in slots.iter().enumerate() {
        key |= ((u32::from(*sq) << 4) | u32::from(*code)) << (1 + 10 * i);
    }
    Some(key)
}

/// Reconstruct the `Board` for a packed key (clock 0, no castling, no ep).
///
/// # Panics
///
/// Panics if the key encodes more than three pieces (never produced by
/// [`region_key`]).
pub(super) fn board_from_key(key: u32) -> Board {
    let mut grid = [b'.'; 64];
    for i in 0..3 {
        let slot = (key >> (1 + 10 * i)) & 0x3FF;
        if slot == 0 {
            continue;
        }
        let sq = (slot >> 4) as usize;
        let (color, pt) = code_piece((slot & 0xF) as u8);
        // PieceType order: Pawn, Knight, Bishop, Rook, Queen, Commoner
        // (the atomic king). Pawns cannot pass the detector, but the decode
        // stays total.
        let ch = if color == 0 {
            b"PNBRQK"[pt as usize]
        } else {
            b"pnbrqk"[pt as usize]
        };
        grid[sq] = ch;
    }
    let mut fen = String::with_capacity(40);
    for r in (0..8).rev() {
        let mut run = 0;
        for f in 0..8 {
            let c = grid[r * 8 + f];
            if c == b'.' {
                run += 1;
            } else {
                if run > 0 {
                    fen.push_str(&run.to_string());
                    run = 0;
                }
                fen.push(c as char);
            }
        }
        if run > 0 {
            fen.push_str(&run.to_string());
        }
        if r > 0 {
            fen.push('/');
        }
    }
    fen.push(' ');
    fen.push(if key & 1 == 0 { 'w' } else { 'b' });
    fen.push_str(" - - 0 1");
    Board::from_fen(&fen).expect("packed key decodes to a valid FEN")
}

/// Clock-0 board-only terminal classification: mirrors
/// `Position::outcome_from_state` precedence (extinction, then moves-empty,
/// then the two-piece draw) without the rule50 branch, which cannot fire at
/// clock 0.
pub(super) fn classify_terminal(board: &Board) -> Option<Outcome> {
    let us = board.side_to_move();
    if board.commoners(us).is_empty() {
        return Some(Outcome::Loss);
    }
    if board.commoners(us.flip()).is_empty() {
        return Some(Outcome::Win);
    }
    let mut moves = MoveList::new();
    generate_legal(board, &mut moves);
    if moves.is_empty() {
        return Some(if board.checkers().is_empty() {
            Outcome::Draw
        } else {
            Outcome::Loss
        });
    }
    if board.occupied().count() == 2 {
        return Some(Outcome::Draw);
    }
    None
}

struct RunCtx<'a> {
    evals: u64,
    eval_budget: u64,
    stop: Option<&'a AtomicBool>,
    aborted: bool,
    reason: &'static str,
}

impl RunCtx<'_> {
    fn tick(&mut self) {
        if self.evals >= self.eval_budget {
            self.abort("eval-budget");
        }
    }
    fn check_stop(&mut self) {
        if !self.aborted
            && let Some(flag) = self.stop
            && flag.load(Ordering::Acquire)
        {
            self.abort("stop");
        }
    }
    fn abort(&mut self, reason: &'static str) {
        if !self.aborted {
            self.aborted = true;
            self.reason = reason;
        }
    }
}

/// One child evaluation: clone + `do_move` + packed key (the solver's
/// `evaluate_child` analogue for accounting purposes).
fn child_key(board: &Board, mv: atomic_movegen::types::Move, ctx: &mut RunCtx) -> Option<u32> {
    ctx.evals += 1;
    ctx.tick();
    if ctx.aborted {
        return None;
    }
    let mut child = board.clone();
    let mut state = StateInfo::new();
    child.do_move(mv, &mut state);
    region_key(&child)
}

/// Build the closure from `root` (must be 3-men/no-pawn/no-castling) and run
/// the exact-rank AND/OR fixpoint.
pub(super) fn analyze(
    root: &Board,
    eval_budget: u64,
    stop: Option<&AtomicBool>,
    region_budget: usize,
) -> Result<Analysis, Abort> {
    let mut ctx = RunCtx {
        evals: 0,
        eval_budget,
        stop,
        aborted: false,
        reason: "",
    };

    let root_key = region_key(root).expect("analyze called on a ≤3-men position");
    let mut keys: Vec<u32> = vec![root_key];
    let mut index: HashMap<u32, u32> = HashMap::new();
    index.insert(root_key, 0);
    let mut terminal: Vec<Option<Outcome>> = vec![classify_terminal(root)];
    let mut children: Vec<u32> = Vec::new();
    let mut off: Vec<u32> = vec![0];

    let mut qi = 0usize;
    while qi < keys.len() {
        ctx.check_stop();
        if ctx.aborted {
            break;
        }
        let idx = qi;
        qi += 1;
        if terminal[idx].is_some() {
            off.push(children.len() as u32);
            continue;
        }
        let board = board_from_key(keys[idx]);
        let mut moves = MoveList::new();
        generate_legal(&board, &mut moves);
        for i in 0..moves.len() {
            let Some(ck) = child_key(&board, moves[i], &mut ctx) else {
                break;
            };
            let next_id = keys.len() as u32;
            let cid = *index.entry(ck).or_insert_with(|| {
                keys.push(ck);
                let child = board_from_key(ck);
                terminal.push(classify_terminal(&child));
                next_id
            });
            children.push(cid);
        }
        off.push(children.len() as u32);
        if !ctx.aborted && keys.len() > region_budget {
            ctx.abort("region-budget");
        }
    }

    let region = keys.len();
    if ctx.aborted {
        return Err(Abort {
            reason: ctx.reason,
            evals: ctx.evals,
            region,
        });
    }

    // AND/OR fixpoint with side-to-move-perspective values.
    let mut val: Vec<u8> = terminal
        .iter()
        .map(|t| match t {
            Some(Outcome::Win) => WIN,
            Some(Outcome::Loss) => LOSS,
            Some(Outcome::Draw) => DRAW,
            None => UNDECIDED,
        })
        .collect();
    loop {
        let mut changed = false;
        for k in 0..region {
            if val[k] != UNDECIDED {
                continue;
            }
            let kids = &children[off[k] as usize..off[k + 1] as usize];
            let mut all_win = !kids.is_empty();
            for &c in kids {
                match val[c as usize] {
                    LOSS => {
                        val[k] = WIN;
                        changed = true;
                        all_win = false;
                        break;
                    }
                    WIN => {}
                    _ => all_win = false,
                }
            }
            if all_win {
                val[k] = LOSS;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // The undecided remainder is a genuine draw (monotonicity lemma).
    for v in &mut val {
        if *v == UNDECIDED {
            *v = DRAW;
        }
    }

    // Exact distances: WIN = 1 + min losing-child dist; LOSS = 1 + max child
    // dist. Delta-free relaxation to a fixed point.
    let mut dist: Vec<u32> = vec![0; region];
    loop {
        let mut changed = false;
        for k in 0..region {
            let kids = &children[off[k] as usize..off[k + 1] as usize];
            let d = match val[k] {
                WIN => kids
                    .iter()
                    .filter(|&&c| val[c as usize] == LOSS)
                    .map(|&c| dist[c as usize])
                    .min()
                    .map(|d| d + 1),
                LOSS => kids.iter().map(|&c| dist[c as usize]).max().map(|d| d + 1),
                _ => None,
            };
            if let Some(d) = d
                && dist[k] != d
            {
                dist[k] = d;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    Ok(Analysis {
        region,
        evals: ctx.evals,
        keys,
        terminal,
        val,
        dist,
        children,
        off,
    })
}

impl Analysis {
    /// Extract the rank-decreasing attacking strategy for the reachable
    /// decided subtree: at every `WIN` node (the winner to move) the chosen
    /// child is a `LOSS` child with distance exactly `dist - 1` (first in
    /// child order); at `LOSS` nodes every reply is covered. `None` on any
    /// internal inconsistency (deferral, never a claim).
    pub(super) fn build_strategy(&self) -> Option<HashMap<u32, u32>> {
        let mut strategy: HashMap<u32, u32> = HashMap::new();
        let mut queue = vec![0u32];
        while let Some(idx) = queue.pop() {
            if self.terminal[idx as usize].is_some() {
                continue;
            }
            let kids = self.children_of(idx);
            match self.val[idx as usize] {
                WIN => {
                    let rank = self.dist[idx as usize];
                    let chosen = kids.iter().copied().find(|&c| {
                        self.val[c as usize] == LOSS && self.dist[c as usize] + 1 == rank
                    })?;
                    strategy.insert(self.keys[idx as usize], self.keys[chosen as usize]);
                    queue.push(chosen);
                }
                LOSS => queue.extend(kids.iter().copied()),
                _ => return None,
            }
        }
        Some(strategy)
    }

    /// Rank-decreasing principal line (the pre-phase PV, per the plan's A5):
    /// at `WIN` nodes the distance-decreasing child, at `LOSS` nodes the
    /// max-distance child (longest resistance), ties broken by child order.
    /// Returns the sequence of child keys *excluding* the root; the length is
    /// exactly `root_rank()`. `None` on any internal inconsistency.
    pub(super) fn principal_line(&self) -> Option<Vec<u32>> {
        let mut line = Vec::with_capacity(self.dist[0] as usize);
        let mut cur = 0u32;
        while self.terminal[cur as usize].is_none() {
            let kids = self.children_of(cur);
            let next = match self.val[cur as usize] {
                WIN => {
                    let rank = self.dist[cur as usize];
                    kids.iter().copied().find(|&c| {
                        self.val[c as usize] == LOSS && self.dist[c as usize] + 1 == rank
                    })?
                }
                LOSS => {
                    let mut best = kids[0];
                    for &c in &kids[1..] {
                        if self.dist[c as usize] > self.dist[best as usize] {
                            best = c;
                        }
                    }
                    best
                }
                _ => return None,
            };
            line.push(self.keys[next as usize]);
            cur = next;
        }
        Some(line)
    }
}
