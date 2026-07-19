use atomic_solver::position::Position;

#[test]
fn hash_changes_after_move() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let mut moves = atomic_movegen::types::MoveList::new();
    pos.legal_moves(&mut moves);
    assert!(!moves.is_empty());
    let m = moves[0];
    let h1 = pos.hash();
    pos.do_move(m);
    let h2 = pos.hash();
    assert_ne!(h1, h2, "hash should change after move");
    pos.undo_move(m);
    assert_eq!(pos.hash(), h1, "hash should restore");
}

#[test]
fn do_undo_restores_fen() {
    let mut pos = Position::from_fen(Position::STARTPOS_FEN).unwrap();
    let mut moves = atomic_movegen::types::MoveList::new();
    pos.legal_moves(&mut moves);
    let m = moves[0];
    let before = pos.fen();
    pos.do_move(m);
    pos.undo_move(m);
    assert_eq!(pos.fen(), before);
}
