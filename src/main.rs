use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::{ExitReason, Search};
use std::io::BufRead;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
///   --outcome-only             Print only the outcome/PV and skip the pre-exit
///                              summary. No stdin reader is spawned.
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
///   atomic_solver --timeout 10
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
    println!("  --outcome-only             Print only the outcome/PV;");
    println!("                             do not spawn stdin reader or pre-exit hook");
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

type PreExitHook = Box<dyn FnOnce(ExitReason, Outcome, u64) + Send>;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or("atomic_solver");
    let mut fen = Position::STARTPOS_FEN.to_string();
    let mut tt_size: usize = 64;
    let mut epsilon = 0.125;
    let mut refine_shortest = true;
    let mut timeout: u64 = 5;
    let mut outcome_only = false;
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
            "--outcome-only" => {
                outcome_only = true;
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

    let mut search = Search::new(tt_size);
    search.set_timeout(timeout);
    search.set_epsilon(epsilon);
    search.refine_shortest(refine_shortest);

    let stop_flag = Arc::new(AtomicBool::new(false));

    let hook: Option<PreExitHook> = if outcome_only {
        None
    } else {
        let flag = Arc::clone(&stop_flag);
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                match line {
                    Ok(l) if l.trim() == "q" => {
                        flag.store(true, Ordering::Release);
                        break;
                    }
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        });

        Some(Box::new(|reason, outcome, nodes| {
            println!("pre_exit: reason={reason} outcome={outcome} nodes={nodes}");
        }))
    };

    search.set_stop_flag(if outcome_only {
        None
    } else {
        Some(Arc::clone(&stop_flag))
    });

    let run_search = |pos: &mut Position, search: &mut Search| -> (Outcome, bool) {
        let outcome = search.solve_outcome(pos);
        if search.time_exceeded() {
            return (outcome, true);
        }

        println!("outcome: {}", outcome_str(outcome));
        if outcome == Outcome::Draw {
            return (outcome, false);
        }

        if let Some(ppv) = search.find_ppv(pos, outcome) {
            println!("pv: {}", pv_str(&ppv));

            if refine_shortest {
                search.refine_sppv(pos, outcome, |shorter| {
                    println!("pv: {}", pv_str(shorter));
                });
                if !search.time_exceeded() {
                    println!("sppv search finished");
                }
            }
        }

        (outcome, search.time_exceeded())
    };

    let (outcome, timed_out) = run_search(&mut pos, &mut search);
    if timed_out {
        let msg = match search.exit_reason() {
            ExitReason::Quit => "quit",
            _ => "timeout",
        };
        println!("{msg}");
    }

    if let Some(hook) = hook {
        hook(search.exit_reason(), outcome, search.nodes());
    }
}
