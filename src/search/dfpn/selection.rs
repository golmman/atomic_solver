//! DF-PN solved-child detection and unsolved-child ordering.
//!
//! This file is larger than 10 KiB because the OR/AND selection logic and
//! best/second-unsolved search are all expressed directly over the `ChildInfo`
//! table. Unit tests live in `selection/tests.rs`.

#![allow(clippy::similar_names)]

use atomic_movegen::types::Move;

use crate::position::Outcome;

use super::children::{ChildInfo, ChildSelection};
use crate::zobrist::INF;

/// Detect whether the children already determine the parent's outcome.
///
/// The returned `Outcome` is from the side-to-move perspective at the
/// parent.  A side-to-move `Win` requires any child that is a `Loss` for the
/// player to move at that child; a side-to-move `Loss` requires all children
/// to be `Win` for that player.  The best child is chosen with the node-type-
/// aware depth rule:
///
/// * `Win` (OR or AND): shortest decisive child.
/// * `Loss` (OR or AND): longest decisive child (most resistance).
/// * `Draw`: longest draw child.
pub(super) fn is_solved_by_children(
    children: &[ChildInfo],
    _is_or_node: bool,
) -> Option<(Outcome, u32, Move, bool, usize)> {
    let mut all_solved = true;
    let mut win_child_idx: Option<usize> = None;
    let mut win_depth = u32::MAX;
    let mut loss_child_idx: Option<usize> = None;
    let mut loss_depth = 0u32;
    let mut draw_child_idx: Option<usize> = None;
    let mut draw_depth = 0u32;
    let mut found_draw = false;

    for (i, c) in children.iter().enumerate() {
        let d = c.depth.saturating_add(1);
        match c.outcome {
            None => {
                all_solved = false;
            }
            Some(Outcome::Loss) => {
                // A child Loss means the next player loses, so the parent
                // side-to-move can choose it and win.  Pick the shortest such win.
                if d < win_depth
                    || (d == win_depth
                        && win_child_idx.is_some()
                        && !c.repetition_seen
                        && children[win_child_idx.unwrap()].repetition_seen)
                {
                    win_depth = d;
                    win_child_idx = Some(i);
                }
            }
            Some(Outcome::Draw) => {
                if d > draw_depth
                    || (d == draw_depth
                        && draw_child_idx.is_some()
                        && !c.repetition_seen
                        && children[draw_child_idx.unwrap()].repetition_seen)
                {
                    draw_depth = d;
                    draw_child_idx = Some(i);
                }
                found_draw = true;
            }
            Some(Outcome::Win) => {
                // A child Win means the next player wins.  If all children are
                // Wins, the parent side-to-move loses; pick the one that delays
                // the loss longest.
                if d > loss_depth
                    || (d == loss_depth
                        && loss_child_idx.is_some()
                        && !c.repetition_seen
                        && children[loss_child_idx.unwrap()].repetition_seen)
                {
                    loss_depth = d;
                    loss_child_idx = Some(i);
                }
            }
        }
    }

    if let Some(idx) = win_child_idx {
        return Some((Outcome::Win, win_depth, children[idx].mv, all_solved, idx));
    }

    if all_solved {
        if found_draw {
            let idx = draw_child_idx.unwrap_or(0);
            return Some((Outcome::Draw, draw_depth, children[idx].mv, true, idx));
        }
        let idx = loss_child_idx.unwrap_or(0);
        return Some((Outcome::Loss, loss_depth, children[idx].mv, true, idx));
    }

    None
}

/// If the children already prove the parent is a win, return the decisive
/// selection immediately. For Loss and Draw we need all children solved.
pub(super) fn select_child_with_early_exit(
    children: &[ChildInfo],
    solved: Option<(Outcome, u32, Move, bool, usize)>,
) -> Option<ChildSelection> {
    if let Some((Outcome::Win, depth, mv, _, _)) = solved {
        return Some(ChildSelection {
            best_child: (Move::NONE, INF, INF),
            second_child: (INF, INF),
            best_child_index: None,
            pn: 0,
            dn: INF,
            depth,
            best_move: mv,
            solved_outcome: Some(Outcome::Win),
            repetition_seen: false,
        });
    }
    if let Some((outcome, depth, mv, all_solved, idx)) = solved
        && all_solved
    {
        // A Win or Loss is path-independent. A Draw may depend on a
        // repetition; carry the selected draw child's flag.
        let repetition_seen = matches!(outcome, Outcome::Draw) && children[idx].repetition_seen;
        return Some(ChildSelection {
            best_child: (Move::NONE, INF, INF),
            second_child: (INF, INF),
            best_child_index: None,
            pn: 0,
            dn: INF,
            depth,
            best_move: mv,
            solved_outcome: Some(outcome),
            repetition_seen,
        });
    }
    None
}

pub(super) fn best_and_second_unsolved(
    children: &[ChildInfo],
    is_or_node: bool,
) -> (Option<usize>, Option<usize>) {
    let mut best: Option<usize> = None;
    let mut second: Option<usize> = None;

    for i in 0..children.len() {
        if children[i].outcome.is_some() || children[i].explored {
            continue;
        }
        let cmp_c = if is_or_node {
            children[i].pn
        } else {
            children[i].dn
        };
        match best {
            None => {
                best = Some(i);
            }
            Some(b) => {
                let cmp_best = if is_or_node {
                    children[b].pn
                } else {
                    children[b].dn
                };
                if cmp_c < cmp_best {
                    second = best;
                    best = Some(i);
                } else {
                    match second {
                        None => {
                            second = Some(i);
                        }
                        Some(s) => {
                            let cmp_second = if is_or_node {
                                children[s].pn
                            } else {
                                children[s].dn
                            };
                            if cmp_c < cmp_second {
                                second = Some(i);
                            }
                        }
                    }
                }
            }
        }
    }
    (best, second)
}

pub(super) fn second_best_unsolved_excluding(
    children: &[ChildInfo],
    is_or_node: bool,
    exclude: usize,
) -> Option<usize> {
    let mut best: Option<usize> = None;
    for i in 0..children.len() {
        if i == exclude || children[i].outcome.is_some() || children[i].explored {
            continue;
        }
        let cmp_c = if is_or_node {
            children[i].pn
        } else {
            children[i].dn
        };
        match best {
            None => best = Some(i),
            Some(b) => {
                let cmp_best = if is_or_node {
                    children[b].pn
                } else {
                    children[b].dn
                };
                if cmp_c < cmp_best {
                    best = Some(i);
                }
            }
        }
    }
    best
}

pub(super) fn selection_for_child(
    children: &[ChildInfo],
    is_or_node: bool,
    idx: usize,
) -> ChildSelection {
    let (mut pn, mut dn) = if is_or_node { (INF, 0) } else { (0, INF) };
    for c in children {
        if is_or_node {
            pn = std::cmp::min(pn, c.pn);
            dn = std::cmp::min(INF, dn.saturating_add(c.dn));
        } else {
            pn = std::cmp::min(INF, pn.saturating_add(c.pn));
            dn = std::cmp::min(dn, c.dn);
        }
    }

    let second_idx = second_best_unsolved_excluding(children, is_or_node, idx);
    let best = &children[idx];
    let second = second_idx.map(|i| &children[i]);

    ChildSelection {
        best_child: (best.mv, best.pn, best.dn),
        second_child: second.map_or((INF, INF), |s| (s.pn, s.dn)),
        best_child_index: Some(idx),
        pn,
        dn,
        depth: 0,
        best_move: best.mv,
        solved_outcome: None,
        repetition_seen: false,
    }
}

/// Compute the parent's proof/disproof numbers and pick the best/second
/// unsolved child from a cached `ChildInfo` table.
///
/// `previous_best_move` and `previous_best_child` are hints from the
/// transposition table. If the stored child is still valid and still the
/// most-proving child, it is reused without recomputing the full argmin.
pub(super) fn select_from_children(
    children: &[ChildInfo],
    is_or_node: bool,
    previous_best_move: Option<Move>,
    previous_best_child: Option<u8>,
) -> ChildSelection {
    let solved = is_solved_by_children(children, is_or_node);

    // A single winning child is enough to prove a Win. For Loss and Draw
    // we need all children to be solved.
    if let Some(selection) = select_child_with_early_exit(children, solved) {
        return selection;
    }

    // Reuse the previous best child if it is still valid and still the best.
    if let Some(prev_mv) = previous_best_move
        && let Some(idx) = previous_best_child
            .filter(|&c| (c as usize) < children.len())
            .map(|c| c as usize)
            .filter(|&i| children[i].mv == prev_mv)
            .or_else(|| children.iter().position(|c| c.mv == prev_mv))
        && children[idx].outcome.is_none()
        && !children[idx].explored
    {
        let is_still_best = if is_or_node {
            children
                .iter()
                .filter(|c| c.outcome.is_none())
                .all(|c| c.pn >= children[idx].pn)
        } else {
            children
                .iter()
                .filter(|c| c.outcome.is_none())
                .all(|c| c.dn >= children[idx].dn)
        };
        if is_still_best {
            return selection_for_child(children, is_or_node, idx);
        }
    }

    // Compute proof/disproof numbers from all children.
    let (mut pn, mut dn) = if is_or_node { (INF, 0) } else { (0, INF) };
    if is_or_node {
        for c in children {
            pn = std::cmp::min(pn, c.pn);
            dn = std::cmp::min(INF, dn.saturating_add(c.dn));
        }
    } else {
        for c in children {
            pn = std::cmp::min(INF, pn.saturating_add(c.pn));
            dn = std::cmp::min(dn, c.dn);
        }
    }

    let (best_idx, second_idx) = best_and_second_unsolved(children, is_or_node);
    let best = best_idx.map(|i| &children[i]);
    let second = second_idx.map(|i| &children[i]);

    let best_child = best.map_or((Move::NONE, INF, INF), |b| (b.mv, b.pn, b.dn));
    let second_child = second.map_or((INF, INF), |s| (s.pn, s.dn));

    let best_move = solved.as_ref().map_or_else(
        || best_idx.map_or(Move::NONE, |i| children[i].mv),
        |(_, _, mv, _, _)| *mv,
    );

    let depth = solved.as_ref().map_or(0, |(_, d, _, _, _)| *d);

    // Win and Loss cannot depend on a repetition. Draw may, so use the
    // selected draw child's flag.
    let repetition_seen = if let Some((outcome, _, _, _, idx)) = solved {
        matches!(outcome, Outcome::Draw) && children[idx].repetition_seen
    } else {
        false
    };

    ChildSelection {
        best_child,
        second_child,
        best_child_index: best_idx,
        pn,
        dn,
        depth,
        best_move,
        solved_outcome: solved.map(|(o, _, _, _, _)| o),
        repetition_seen,
    }
}

#[cfg(test)]
mod tests;
