//! DF-PN solved-child detection and unsolved-child ordering.

#![allow(clippy::similar_names)]

use atomic_movegen::types::Move;

use crate::position::Outcome;

use super::children::{ChildInfo, ChildSelection};
use super::{INF, ProofMode, Search};

impl Search {
    /// Detect whether the children already determine the parent's outcome.
    ///
    /// The returned `Outcome` is from the side-to-move perspective at the
    /// parent.  A side-to-move `Win` requires any child that is a `Loss` for
    /// the player to move at that child; a side-to-move `Loss` requires all
    /// children to be `Win` for that player.  The best child is chosen with
    /// the node-type-aware depth rule:
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
                    // side-to-move can choose it and win.  Pick the shortest
                    // such win.
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
                    // A child Win means the next player wins.  If all children
                    // are Wins, the parent side-to-move loses; pick the one
                    // that delays the loss longest.
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

    /// If the children already prove the parent is a win and we do not need a
    /// fully minimax refinement, return the decisive selection immediately.
    ///
    /// For `Outcome` and `Ppv` a single winning child is enough; for `Sppv`
    /// we wait until all children are solved (or a losing parent outcome is
    /// fully proven, which is always `all_solved`).
    pub(super) fn select_child_with_early_exit(
        children: &[ChildInfo],
        _is_or_node: bool,
        proof_mode: ProofMode,
        solved: Option<(Outcome, u32, Move, bool, usize)>,
    ) -> Option<ChildSelection> {
        if let Some((Outcome::Win, depth, mv, all_solved, idx)) = solved
            && (all_solved || proof_mode != ProofMode::Sppv)
        {
            return Some(ChildSelection {
                best_child: (Move::NONE, INF, INF),
                second_child: (INF, INF),
                best_child_index: None,
                pn: 0,
                dn: INF,
                depth,
                best_move: mv,
                solved_outcome: Some(Outcome::Win),
                all_solved,
                repetition_seen: children[idx].repetition_seen,
            });
        }
        if let Some((outcome, depth, mv, all_solved, idx)) = solved
            && all_solved
        {
            return Some(ChildSelection {
                best_child: (Move::NONE, INF, INF),
                second_child: (INF, INF),
                best_child_index: None,
                pn: 0,
                dn: INF,
                depth,
                best_move: mv,
                solved_outcome: Some(outcome),
                all_solved,
                repetition_seen: children[idx].repetition_seen,
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
}

#[cfg(test)]
mod tests {
    use super::super::children::ChildInfo;
    use super::super::{ProofMode, Search};
    use crate::position::Outcome;
    use atomic_movegen::types::{Move, Square};

    fn child(outcome: Option<Outcome>, depth: u32, from: Square, to: Square) -> ChildInfo {
        ChildInfo {
            mv: Move::make_move(from, to),
            pn: 1,
            dn: 1,
            outcome,
            depth,
            repetition_seen: false,
            explored: false,
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
    fn and_node_win_picks_longest_loss_for_attacker() {
        // At an AND node an attacker win is a side-to-move Loss: all children
        // are Wins for the attacker and the defender delays the longest.
        let children = vec![
            child(Some(Outcome::Win), 2, Square::A1, Square::A2),
            child(Some(Outcome::Win), 5, Square::B1, Square::B2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, false).unwrap();
        assert_eq!(outcome, Outcome::Loss);
        assert_eq!(depth, 6);
        assert_eq!(mv, Move::make_move(Square::B1, Square::B2));
        assert!(all_solved);
    }

    #[test]
    fn and_node_defender_win_picks_shortest_loss() {
        // At an AND node a defender win is a side-to-move Win: the defender
        // finds a reply that makes the attacker lose as fast as possible.
        let children = vec![
            child(Some(Outcome::Loss), 5, Square::A1, Square::A2),
            child(Some(Outcome::Loss), 2, Square::B1, Square::B2),
            child(Some(Outcome::Win), 0, Square::C1, Square::C2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, false).unwrap();
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
            Search::is_solved_by_children(&children, true).unwrap();
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

    #[test]
    fn mixed_win_and_draw_children_is_draw() {
        let children = vec![
            child(Some(Outcome::Win), 2, Square::A1, Square::A2),
            child(Some(Outcome::Draw), 4, Square::B1, Square::B2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, true).unwrap();
        assert_eq!(outcome, Outcome::Draw);
        assert_eq!(depth, 5);
        assert_eq!(mv, Move::make_move(Square::B1, Square::B2));
        assert!(all_solved);
    }

    #[test]
    fn mixed_win_depths_returns_longest_loss() {
        let children = vec![
            child(Some(Outcome::Win), 3, Square::A1, Square::A2),
            child(Some(Outcome::Win), 6, Square::B1, Square::B2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, true).unwrap();
        assert_eq!(outcome, Outcome::Loss);
        assert_eq!(depth, 7);
        assert_eq!(mv, Move::make_move(Square::B1, Square::B2));
        assert!(all_solved);
    }

    #[test]
    fn early_exit_allows_win_when_not_all_solved() {
        let children = vec![
            child(Some(Outcome::Loss), 5, Square::A1, Square::A2),
            child(None, 0, Square::B1, Square::B2),
        ];
        let solved = Search::is_solved_by_children(&children, true);
        let selection =
            Search::select_child_with_early_exit(&children, true, ProofMode::Ppv, solved);
        assert!(selection.is_some());
        assert_eq!(selection.unwrap().solved_outcome, Some(Outcome::Win));
    }

    #[test]
    fn sppv_does_not_early_exit_on_partial_win() {
        let children = vec![
            child(Some(Outcome::Loss), 5, Square::A1, Square::A2),
            child(None, 0, Square::B1, Square::B2),
        ];
        let solved = Search::is_solved_by_children(&children, true);
        let selection =
            Search::select_child_with_early_exit(&children, true, ProofMode::Sppv, solved);
        assert!(selection.is_none());
    }
}
