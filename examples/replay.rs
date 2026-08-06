use atomic_solver::notation::uci_to_move;
use atomic_solver::position::Position;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fen = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or(Position::STARTPOS_FEN);
    let mut pos = Position::from_fen(fen).unwrap();
    for token in args[2..].iter() {
        let mv = uci_to_move(token, &pos).unwrap_or_else(|| panic!("illegal move {token}"));
        pos.do_move(mv);
    }
    println!("fen: {}", pos.fen());
    println!("outcome: {:?}", pos.outcome());
}
