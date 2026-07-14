use atomic_movegen::board::StateInfo;
use atomic_movegen::types::MoveList;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;
use atomic_solver::search::ordering::{MoveScorer, StaticAtomicScorer};

fn main() {
    let pos =
        Position::from_fen("rnbqkbnr/ppp1p2p/3p1pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 4")
            .unwrap();
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    let mut state = StateInfo::new();
    pos.board.populate_state(&mut state);
    let mut scored: Vec<(usize, i32)> = (0..moves.len())
        .map(|i| {
            let m = moves[i];
            let s = StaticAtomicScorer.score(&pos.board, m, &state);
            (i, s)
        })
        .collect();
    scored.sort_by_key(|b| std::cmp::Reverse(b.1));
    for (i, s) in scored {
        let m = moves[i];
        println!("{} {}", move_to_uci(m), s);
    }
}
