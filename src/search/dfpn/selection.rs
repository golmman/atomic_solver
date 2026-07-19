//! DF-PN solved-child detection and unsolved-child ordering.

#![allow(clippy::similar_names)]

use atomic_movegen::types::Move;

use crate::position::Outcome;

use super::Search;
use super::children::ChildInfo;

impl Search {
    /// Detect whether all children are solved, and if so return the parent's
    /// outcome, the best child index, and the proof depth. The names below use
    /// the parent-side perspective: a child `Loss` means the parent can force a
    /// `Win`, and a child `Win` means the parent is forced to `Loss`.
    pub(super) fn is_solved_by_children(
        children: &[ChildInfo],
        _is_or_node: bool,
    ) -> Option<(Outcome, u32, Move, bool, usize)> {
        let mut all_solved = true;
        let mut parent_win_child_idx: Option<usize> = None;
        let mut parent_win_depth = u32::MAX;
        let mut parent_draw_child_idx: Option<usize> = None;
        let mut parent_draw_depth = 0;
        let mut found_draw = false;
        let mut parent_loss_child_idx: Option<usize> = None;
        let mut parent_loss_depth = 0;

        for (i, c) in children.iter().enumerate() {
            let d = c.depth.saturating_add(1);
            match c.outcome {
                None => {
                    all_solved = false;
                }
                Some(Outcome::Loss) => {
                    // Prefer shortest loss for the child, which is the shortest
                    // win for the parent. Among ties prefer path-independent.
                    if d < parent_win_depth
                        || (d == parent_win_depth
                            && parent_win_child_idx.is_some()
                            && !c.repetition_seen
                            && children[parent_win_child_idx.unwrap()].repetition_seen)
                    {
                        parent_win_depth = d;
                        parent_win_child_idx = Some(i);
                    }
                }
                Some(Outcome::Draw) => {
                    if d > parent_draw_depth
                        || (d == parent_draw_depth
                            && parent_draw_child_idx.is_some()
                            && !c.repetition_seen
                            && children[parent_draw_child_idx.unwrap()].repetition_seen)
                    {
                        parent_draw_depth = d;
                        parent_draw_child_idx = Some(i);
                    }
                    found_draw = true;
                }
                Some(Outcome::Win) => {
                    if d > parent_loss_depth
                        || (d == parent_loss_depth
                            && parent_loss_child_idx.is_some()
                            && !c.repetition_seen
                            && children[parent_loss_child_idx.unwrap()].repetition_seen)
                    {
                        parent_loss_depth = d;
                        parent_loss_child_idx = Some(i);
                    }
                }
            }
        }

        if let Some(idx) = parent_win_child_idx {
            return Some((
                Outcome::Win,
                parent_win_depth,
                children[idx].mv,
                all_solved,
                idx,
            ));
        }

        if all_solved {
            if found_draw {
                let idx = parent_draw_child_idx.unwrap_or(0);
                return Some((
                    Outcome::Draw,
                    parent_draw_depth,
                    children[idx].mv,
                    true,
                    idx,
                ));
            }
            let idx = parent_loss_child_idx.unwrap_or(0);
            return Some((
                Outcome::Loss,
                parent_loss_depth,
                children[idx].mv,
                true,
                idx,
            ));
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
            if children[i].outcome.is_some() {
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
}

#[cfg(test)]
mod tests {
    use super::super::children::ChildInfo;
    use crate::position::Outcome;
    use crate::search::dfpn::Search;
    use atomic_movegen::types::{Move, Square};

    fn child(outcome: Option<Outcome>, depth: u32, from: Square, to: Square) -> ChildInfo {
        ChildInfo {
            mv: Move::make_move(from, to),
            pn: 1,
            dn: 1,
            outcome,
            depth,
            repetition_seen: false,
        }
    }

    #[test]
    fn win_picks_shortest_loss_child() {
        let children = vec![
            child(Some(Outcome::Loss), 5, Square::A1, Square::A2),
            child(Some(Outcome::Loss), 2, Square::B1, Square::B2),
            child(Some(Outcome::Win), 0, Square::C1, Square::C2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, true).unwrap();
        assert_eq!(outcome, Outcome::Win);
        assert_eq!(depth, 3);
        assert_eq!(mv, Move::make_move(Square::B1, Square::B2));
        assert!(all_solved);
    }

    #[test]
    fn draw_picks_longest_draw_child() {
        let children = vec![
            child(Some(Outcome::Win), 4, Square::A1, Square::A2),
            child(Some(Outcome::Draw), 1, Square::B1, Square::B2),
            child(Some(Outcome::Draw), 7, Square::C1, Square::C2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, false).unwrap();
        assert_eq!(outcome, Outcome::Draw);
        assert_eq!(depth, 8);
        assert_eq!(mv, Move::make_move(Square::C1, Square::C2));
        assert!(all_solved);
    }

    #[test]
    fn loss_picks_longest_win_child() {
        let children = vec![
            child(Some(Outcome::Win), 2, Square::A1, Square::A2),
            child(Some(Outcome::Win), 5, Square::B1, Square::B2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, true).unwrap();
        assert_eq!(outcome, Outcome::Loss);
        assert_eq!(depth, 6);
        assert_eq!(mv, Move::make_move(Square::B1, Square::B2));
        assert!(all_solved);
    }

    #[test]
    fn unsolved_returns_none() {
        let children = vec![
            child(Some(Outcome::Win), 0, Square::A1, Square::A2),
            child(None, 0, Square::B1, Square::B2),
        ];
        assert!(Search::is_solved_by_children(&children, true).is_none());
    }

    #[test]
    fn win_with_unsolved_returns_not_all_solved() {
        let children = vec![
            child(Some(Outcome::Loss), 5, Square::A1, Square::A2),
            child(None, 0, Square::B1, Square::B2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, true).unwrap();
        assert_eq!(outcome, Outcome::Win);
        assert_eq!(depth, 6);
        assert_eq!(mv, Move::make_move(Square::A1, Square::A2));
        assert!(!all_solved);
    }
}
