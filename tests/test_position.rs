use atomic_movegen::types::MoveList;
use atomic_solver::position::Position;
use atomic_solver::zobrist;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn range(&mut self, max: usize) -> usize {
        (self.next() as usize) % max
    }
}

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

#[test]
fn incremental_hash_matches_full_hash_in_random_game() {
    let mut pos = Position::from_fen(Position::STARTPOS_FEN).unwrap();
    let mut rng = Rng(0x1234_5678_9abc_def0);
    let mut played = Vec::new();

    for _ in 0..100 {
        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);
        if moves.is_empty() {
            break;
        }
        let m = moves[rng.range(moves.len())];
        pos.do_move(m);
        played.push(m);
        assert_eq!(
            pos.hash(),
            zobrist::hash(&pos.board, pos.board.rule50()),
            "incremental Position hash must equal full zobrist hash after do_move"
        );
    }

    for m in played.into_iter().rev() {
        pos.undo_move(m);
        assert_eq!(
            pos.hash(),
            zobrist::hash(&pos.board, pos.board.rule50()),
            "incremental Position hash must equal full zobrist hash after undo_move"
        );
    }
}
