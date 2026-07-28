use atomic_movegen::types::MoveList;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;

fn main() {
    let fen = "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26";
    let mut pos = Position::from_fen(fen).unwrap();
    let mv = atomic_movegen::types::Move::make_move(
        atomic_movegen::types::Square::B1,
        atomic_movegen::types::Square::B8,
    );
    pos.do_move(mv);

    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    for i in 0..moves.len() {
        println!("{}", move_to_uci(moves[i]));
    }
}
