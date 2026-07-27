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
///   --tt-size <MB>             Transposition-table size in megabytes.
///                              Defaults to 64.
///   --epsilon <VALUE>          DF-PN+ threshold parameter in the range [0.0, 1.0].
///                              Defaults to 0.125.
///   --no-refine-shortest       Find and print the outcome and the Proof PV (PPV),
///                              but do not refine toward the Shortest PPV (SPPV).
///   --timeout <SECONDS>        Search time limit in seconds.
///                              Defaults to 5.
///
/// Output:
///   First the decisive outcome (`outcome: win`, `outcome: loss`, or
///   `outcome: draw`) is printed. For wins/losses this is followed by a `pv:`
///   line for the PPV, then additional `pv:` lines for each strictly shorter
///   PPV discovered during SPPV refinement. If the timeout is reached after
///   any result, `timeout` is printed on its own line.
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
    println!("  --tt-size <MB>             Transposition-table size in megabytes");
    println!("                             (default: 64)");
    println!("  --epsilon <VALUE>          DF-PN+ threshold parameter in [0.0, 1.0]");
    println!("                             (default: 0.125)");
    println!("  --no-refine-shortest       Find and print the PPV but do not refine");
    println!("                             toward the Shortest PPV (SPPV)");
    println!("  --timeout <SECONDS>        Search time limit in seconds");
    println!("                             (default: 5)");
    println!();
    println!("Examples:");
    println!("  {program} --help");
    println!("  {program} --fen \"4k3/8/8/8/8/8/8/4KRR1 w - - 0 1\"");
    println!("  {program} --epsilon 0.5 --no-refine-shortest");
    println!("  {program} --timeout 10");
}

fn outcome_str(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Win => "win",
        Outcome::Loss => "loss",
        Outcome::Draw => "draw",
    }
}

fn pv_str(pv: &[atomic_movegen::types::Move]) -> String {
    pv.iter()
        .map(|&m| move_to_uci(m))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or("atomic_solver");
    let mut fen = Position::STARTPOS_FEN.to_string();
    let mut tt_size: usize = 64;
    let mut epsilon = 0.125;
    let mut refine_shortest = true;
    let mut timeout: u64 = 5;
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
            "--tt-size" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --tt-size requires a value");
                    std::process::exit(1);
                }
                match args[i + 1].parse::<usize>() {
                    Ok(v) if v > 0 => tt_size = v,
                    Ok(v) => {
                        eprintln!("error: --tt-size must be positive, got {v}");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("error: invalid --tt-size value: {e}");
                        std::process::exit(1);
                    }
                }
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
            "--timeout" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --timeout requires a value");
                    std::process::exit(1);
                }
                match args[i + 1].parse::<u64>() {
                    Ok(v) if v > 0 => timeout = v,
                    Ok(v) => {
                        eprintln!("error: timeout must be positive, got {v}");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("error: invalid timeout value: {e}");
                        std::process::exit(1);
                    }
                }
                i += 2;
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

    let mut search = Search::new(tt_size);
    search.set_timeout(timeout);
    search.set_epsilon(epsilon);
    search.refine_shortest(refine_shortest);

    let outcome = search.solve_outcome(&mut pos);
    if search.time_exceeded() {
        println!("timeout");
        return;
    }

    println!("outcome: {}", outcome_str(outcome));
    if outcome == Outcome::Draw {
        return;
    }

    if let Some(ppv) = search.find_ppv(&mut pos, outcome) {
        println!("pv: {}", pv_str(&ppv));

        if refine_shortest {
            search.refine_sppv(&mut pos, outcome, |shorter| {
                println!("pv: {}", pv_str(shorter));
            });
            println!("sppv search finished");
        }
    }

    if search.time_exceeded() {
        println!("timeout");
    }
}
