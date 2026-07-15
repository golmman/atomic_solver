use atomic_movegen::attacks;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;

const DEFAULT_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn main() {
    attacks::init();

    let args: Vec<String> = std::env::args().collect();
    let mut fen = DEFAULT_FEN.to_string();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--fen" && i + 1 < args.len() {
            fen = args[i + 1].clone();
            i += 2;
        } else {
            i += 1;
        }
    }

    let mut pos = Position::from_fen(&fen).unwrap_or_else(|e| {
        eprintln!("Failed to parse FEN: {e}");
        std::process::exit(1);
    });

    let mut search = Search::new(64);
    search.refine_shortest(true);
    search.set_timeout(5);
    let (outcome, pv, _nodes) = search.solve(&mut pos);

    let outcome_str = match outcome {
        atomic_solver::position::Outcome::Win => "win",
        atomic_solver::position::Outcome::Loss => "loss",
        atomic_solver::position::Outcome::Draw => "draw",
    };

    if matches!(
        outcome,
        atomic_solver::position::Outcome::Win | atomic_solver::position::Outcome::Loss
    ) {
        let pv_str: String = pv
            .iter()
            .map(|&m| move_to_uci(m))
            .collect::<Vec<_>>()
            .join(" ");
        println!("outcome: {outcome_str}\npv: {pv_str}");
    } else {
        println!("outcome: {outcome_str}");
    }
}
