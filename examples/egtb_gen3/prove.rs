//! Independent depth-limited AND/OR proof oracle (egtb `plan1` Task C).
//!
//! Deliberately shares no code with the table generator or the solver: a
//! plain memoized minimax over `atomic_movegen::board::Board` legal moves
//! with the solver's terminal classification. Used as a second oracle in
//! cross-validation: a forced mate within the depth bound must be found
//! (confirming table Wins), and a proof contradicting a table value is a
//! defect on one of the two sides.
//!
//! Repetition/rule50 caveats: a proven Win within `depth` plies is valid
//! regardless of repetition or the 50-move rule (the mate preempts both,
//! depth ≤ 98). `None` is inconclusive, never a Draw proof. A proven Loss
//! (all defender replies proven Wins) is likewise repetition-free within
//! the bound.

use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::movegen::generate_legal;
use atomic_movegen::types::MoveList;
use std::collections::HashMap;

pub const P_LOSS: u8 = 1;
pub const P_DRAW: u8 = 2;
pub const P_WIN: u8 = 3;

/// Terminal classification, solver-equivalent (clock-0 semantics).
fn classify(b: &Board) -> Option<u8> {
    let us = b.side_to_move();
    if b.commoners(us).is_empty() {
        return Some(P_LOSS);
    }
    if b.commoners(us.flip()).is_empty() {
        return Some(P_WIN);
    }
    let mut m = MoveList::new();
    generate_legal(b, &mut m);
    if m.is_empty() {
        return Some(if b.checkers().is_empty() {
            P_DRAW
        } else {
            P_LOSS
        });
    }
    if b.occupied().count() == 2 {
        return Some(P_DRAW);
    }
    None
}

/// Memoized depth-limited value (the memo stores `None` for unknown) of `b` from the side-to-move perspective.
/// The memo is keyed by `(fen, depth)` and may be shared across calls.
pub fn prove(b: &Board, depth: u32, memo: &mut HashMap<(String, u32), Option<u8>>) -> Option<u8> {
    if let Some(v) = classify(b) {
        return Some(v);
    }
    if depth == 0 {
        return None;
    }
    let key = (b.fen(), depth);
    if let Some(&v) = memo.get(&key) {
        return v;
    }
    let mut m = MoveList::new();
    generate_legal(b, &mut m);
    let mut all_win = true;
    let mut result: Option<u8> = None;
    let mut child;
    let mut st = StateInfo::new();
    for i in 0..m.len() {
        child = b.clone();
        child.do_move(m[i], &mut st);
        match prove(&child, depth - 1, memo) {
            Some(P_LOSS) => {
                result = Some(P_WIN);
                break;
            }
            Some(P_WIN) => {}
            _ => all_win = false,
        }
    }
    if result.is_none() && all_win {
        result = Some(P_LOSS);
    }
    memo.insert(key, result);
    result
}
