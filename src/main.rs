use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;

/// Command-line solver for atomic chess.
///
/// Usage:
///   atomic_solver [OPTIONS]
///
/// Options:
///   -h, --help                 Show this help message and exit.
///   --fen <FEN>                Position to solve in Forsyth-Edwards Notation.
///                              Defaults to the standard atomic start position
///                              ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").
///   --epsilon <VALUE>          DF-PN+ threshold parameter in the range [0.0, 1.0].
///                              Defaults to 0.25.
///   --no-refine-shortest       Disable shortest-PV refinement (enabled by default).
///
/// Examples:
///   atomic_solver --help
///   atomic_solver --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
///   atomic_solver --epsilon 0.5 --no-refine-shortest
fn print_help(program: &str) {
    println!("atomic chess solver");
    println!();
    println!("Usage:");
    println!("  {program} [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -h, --help                 Show this help message and exit");
    println!("  --fen <FEN>                Position to solve in Forsyth-Edwards Notation");
    println!("                             (default: standard atomic start position)");
    println!("  --epsilon <VALUE>          DF-PN+ threshold parameter in [0.0, 1.0]");
    println!("                             (default: 0.25)");
    println!("  --no-refine-shortest       Disable shortest-PV refinement");
    println!("                             (default: enabled)");
    println!();
    println!("Examples:");
    println!("  {program} --help");
    println!("  {program} --fen \"4k3/8/8/8/8/8/8/4KRR1 w - - 0 1\"");
    println!("  {program} --epsilon 0.5 --no-refine-shortest");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or("atomic_solver");
    let mut fen = Position::STARTPOS_FEN.to_string();
    let mut epsilon = 0.25;
    let mut refine_shortest = true;
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "-h" | "--help" => {
                print_help(program);
                std::process::exit(0);
            }
            "--fen" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --fen requires a value");
                    std::process::exit(1);
                }
                fen = args[i + 1].clone();
                i += 2;
            }
            "--epsilon" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --epsilon requires a value");
                    std::process::exit(1);
                }
                match args[i + 1].parse::<f64>() {
                    Ok(v) if (0.0..=1.0).contains(&v) => epsilon = v,
                    Ok(v) => {
                        eprintln!("error: epsilon must be in [0.0, 1.0], got {v}");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("error: invalid epsilon value: {e}");
                        std::process::exit(1);
                    }
                }
                i += 2;
            }
            "--no-refine-shortest" => {
                refine_shortest = false;
                i += 1;
            }
            _ => {
                eprintln!("error: unknown option '{arg}'");
                eprintln!("Run '{program} --help' for usage.");
                std::process::exit(1);
            }
        }
    }

    let mut pos = Position::from_fen(&fen).unwrap_or_else(|e| {
        eprintln!("Failed to parse FEN: {e}");
        std::process::exit(1);
    });

    let mut search = Search::new(64);
    if refine_shortest {
        search.refine_shortest(true);
    }
    search.set_timeout(5);
    search.set_epsilon(epsilon);
    let (outcome, pv, _nodes) = search.solve(&mut pos);

    let outcome_str = match outcome {
        Outcome::Win => "win",
        Outcome::Loss => "loss",
        Outcome::Draw => "draw",
    };

    if matches!(outcome, Outcome::Win | Outcome::Loss) {
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
