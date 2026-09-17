use atomic_solver::notation::uci_to_move;
use atomic_solver::position::Position;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fen = args
        .get(1)
        .map_or(Position::STARTPOS_FEN, std::string::String::as_str);
    let mut pos = Position::from_fen(fen).unwrap();
    for token in &args[2..] {
        let mv = uci_to_move(token, &pos).unwrap_or_else(|| panic!("illegal move {token}"));
        pos.do_move(mv);
    }
    println!("fen: {}", pos.fen());
    println!("outcome: {:?}", pos.outcome());
}
