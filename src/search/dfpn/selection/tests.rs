use super::super::children::ChildInfo;
use super::*;
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
    let (outcome, depth, mv, all_solved, _idx) = is_solved_by_children(&children, true).unwrap();
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
    let (outcome, depth, mv, all_solved, _idx) = is_solved_by_children(&children, false).unwrap();
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
    let (outcome, depth, mv, all_solved, _idx) = is_solved_by_children(&children, false).unwrap();
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
    let (outcome, depth, mv, all_solved, _idx) = is_solved_by_children(&children, true).unwrap();
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
    let (outcome, depth, mv, all_solved, _idx) = is_solved_by_children(&children, true).unwrap();
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
    assert!(is_solved_by_children(&children, true).is_none());
}

#[test]
fn win_with_unsolved_returns_not_all_solved() {
    let children = vec![
        child(Some(Outcome::Loss), 5, Square::A1, Square::A2),
        child(None, 0, Square::B1, Square::B2),
    ];
    let (outcome, depth, mv, all_solved, _idx) = is_solved_by_children(&children, true).unwrap();
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
    let (outcome, depth, mv, all_solved, _idx) = is_solved_by_children(&children, true).unwrap();
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
    let (outcome, depth, mv, all_solved, _idx) = is_solved_by_children(&children, true).unwrap();
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
    let solved = is_solved_by_children(&children, true);
    let selection = select_child_with_early_exit(&children, solved);
    assert!(selection.is_some());
    assert_eq!(selection.unwrap().solved_outcome, Some(Outcome::Win));
}

#[test]
fn select_from_children_can_be_called_without_search_instance() {
    // A single solved win child should produce an immediate selection.
    let children = vec![child(Some(Outcome::Loss), 0, Square::A1, Square::A2)];
    let selection = select_from_children(&children, true, None, None);
    assert_eq!(selection.solved_outcome, Some(Outcome::Win));
    assert_eq!(selection.pn, 0);
    assert_eq!(selection.dn, INF);
}

#[test]
fn best_and_second_unsolved_orders_by_or_pn() {
    let children = vec![
        ChildInfo {
            mv: Move::make_move(Square::A1, Square::A2),
            pn: 5,
            dn: 1,
            outcome: None,
            depth: 0,
            repetition_seen: false,
            explored: false,
        },
        ChildInfo {
            mv: Move::make_move(Square::B1, Square::B2),
            pn: 2,
            dn: 9,
            outcome: None,
            depth: 0,
            repetition_seen: false,
            explored: false,
        },
        ChildInfo {
            mv: Move::make_move(Square::C1, Square::C2),
            pn: 8,
            dn: 1,
            outcome: None,
            depth: 0,
            repetition_seen: false,
            explored: false,
        },
    ];
    let (best, second) = best_and_second_unsolved(&children, true);
    assert_eq!(best, Some(1));
    assert_eq!(second, Some(0));
}

#[test]
fn solved_children_are_skipped_by_best_and_second() {
    let children = vec![
        child(Some(Outcome::Loss), 0, Square::A1, Square::A2),
        ChildInfo {
            mv: Move::make_move(Square::B1, Square::B2),
            pn: 3,
            dn: 1,
            outcome: None,
            depth: 0,
            repetition_seen: false,
            explored: false,
        },
    ];
    let (best, second) = best_and_second_unsolved(&children, true);
    assert_eq!(best, Some(1));
    assert_eq!(second, None);
}
