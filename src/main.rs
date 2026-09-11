//! Command-line solver for atomic chess.
//!
//! This file is larger than 10 KiB because it contains the argument parsing,
//! help text, search setup, and pre-exit hook in one place so the binary is
//! self-contained.
//!
//! Usage:
//!   atomic_solver [OPTIONS]
//!
//! Options:
//!   -h, --help                 Show this help message and exit.
//!   --fen <FEN>                Position to solve in Forsyth-Edwards Notation.
//!                              Defaults to [`Position::STARTPOS_FEN`].
//!   --tt-size <MB>             Transposition-table size in megabytes.
//!                              Defaults to 128.
//!   --epsilon <VALUE>          DF-PN+ threshold parameter in the range [0.0, 1.0].
//!                              Defaults to 0.125.
//!   --timeout <SECONDS>        Search time limit in seconds.
//!                              Defaults to 5.
//!   --first-outcome             Stop after the first decisive outcome and skip
//!                              the iterative PV refinement.
//!   --refine-cap <FACTOR>      Per-refinement-round work-cap factor relative
//!                              to the first-outcome phase's child-eval count.
//!                              Each refinement round is capped at
//!                              max(1,000,000, FACTOR * first-outcome evals);
//!                              a round that hits the cap without a shorter
//!                              decisive line is abandoned. `0` disables
//!                              capping. Defaults to 0.25.
//!   --outcome-only             Print only the outcome/PV: no stdin reader and
//!                              no pre-exit summary.
//!   --tt-dump-path <FILE>      Write a compact binary snapshot of the
//!                              transposition table after the search finishes.
//!                              Optional; the snapshot is the transfer
//!                              artifact for offline proof reconstruction via
//!                              `reconstruct_pt --snapshot` (the search CLI
//!                              itself never builds proof trees).
//!
//! Output:
//!   Each newly discovered decisive line is logged as
//!   `outcome: <win|loss|draw> length: <plies>`. For wins and losses the final
//!   line is followed by `pv: <UCI moves>`, an informational best-effort line
//!   from the transposition table, and `pv_status: <label>` describing whether
//!   the PV is proven shortest (`proven-shortest`), the unrefined first
//!   outcome (`first-outcome`), cut by the refinement work cap (`cap-cut`), or
//!   cut by a global resource limit / not shorter (`cut-short`). `pv_status`
//!   qualifies the PV length, not its validity. If the timeout is reached
//!   after any result, `timeout` is printed on its own line. Without
//!   `--outcome-only` the pre-exit hook prints a `pre_exit:` summary line.
//!
//! Examples:
//!   atomic_solver --help
//!   atomic_solver --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
//!   atomic_solver --epsilon 0.5 --first-outcome
//!   atomic_solver --timeout 10

use atomic_movegen::types::Move;
use atomic_solver::config;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::{ExitReason, PvStatus, Search};
use atomic_solver::search::ordering::StaticAtomicScorer;
use atomic_solver::tt_snapshot::write_tt_snapshot;
use std::io::BufRead;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

mod cli;
use cli::{CliOptions, ParseResult, parse_args};

fn print_help(program: &str) {
    println!("atomic chess solver");
    println!();
    println!("Usage:");
    println!("  {program} [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -h, --help                 Show this help message and exit");
    println!("  --fen <FEN>                Position in Forsyth-Edwards Notation");
    println!("                             (default: standard atomic start position)");
    println!("  --tt-size <MB>             Transposition-table size in megabytes");
    println!("                             (default: 128)");
    println!("  --epsilon <VALUE>          DF-PN+ threshold parameter in [0.0, 1.0]");
    println!("                             (default: 0.125)");
    println!("  --timeout <SECONDS>        Search time limit in seconds");
    println!("                             (default: 5)");
    println!("  --first-outcome            Stop after the first decisive outcome");
    println!("                             and skip iterative PV refinement");
    println!("  --refine-cap <FACTOR>      Per-refinement-round work-cap factor vs.");
    println!("                             the first-outcome eval count; a round that");
    println!("                             hits the cap without a shorter decisive");
    println!("                             line is abandoned. 0 disables capping");
    println!("                             (default: 0.25)");
    println!("  --outcome-only             Print only the outcome/PV;");
    println!("                             no stdin reader and no pre-exit summary");
    println!("  --tt-dump-path <FILE>      Write a binary TT snapshot after the search");
    println!("                             (transfer artifact for offline proof");
    println!("                             reconstruction via reconstruct_pt; optional)");
    println!("  --config <FILE>            Path to a TOML file overriding scorer");
    println!("                             parameters; defaults to built-in values");
    println!();
    println!("Examples:");
    println!("  {program} --help");
    println!("  {program} --fen \"4k3/8/8/8/8/8/8/4KRR1 w - - 0 1\"");
    println!("  {program} --epsilon 0.5 --first-outcome");
    println!("  {program} --timeout 10");
}

fn pv_str(pv: &[Move]) -> String {
    pv.iter()
        .copied()
        .map(move_to_uci)
        .collect::<Vec<_>>()
        .join(" ")
}

type PreExitHook = Box<dyn FnOnce(ExitReason, Outcome, u64, &[Move]) + Send>;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or("atomic_solver");

    let opts = match parse_args(&args) {
        Ok(ParseResult::Help) => {
            print_help(program);
            std::process::exit(0);
        }
        Ok(ParseResult::Options(o)) => o,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let CliOptions {
        fen,
        tt_size,
        epsilon,
        timeout,
        first_outcome,
        refine_cap,
        outcome_only,
        tt_dump_path,
        config_path,
    } = opts;

    let config_path = config_path.or_else(|| std::env::var("SCORER_CONFIG").ok());

    let scorer = match config_path {
        Some(path) => match config::load_scorer_config(&path) {
            Ok(params) => StaticAtomicScorer::from_params(params),
            Err(e) => {
                eprintln!("Failed to load config file: {e}");
                std::process::exit(1);
            }
        },
        None => StaticAtomicScorer::default(),
    };

    let mut pos = Position::from_fen(&fen).unwrap_or_else(|e| {
        eprintln!("Failed to parse FEN: {e}");
        std::process::exit(1);
    });

    let mut search = Search::new(tt_size);
    search.set_scorer(scorer);
    search.set_timeout(timeout);
    search.set_epsilon(epsilon);
    search.set_first_outcome_only(first_outcome);
    search.set_refine_cap_factor(refine_cap);

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

        Some(Box::new(|reason, outcome, nodes, _pv: &[Move]| {
            println!("pre_exit: reason={reason} outcome={outcome} nodes={nodes}");
        }))
    };

    search.set_stop_flag(if outcome_only {
        None
    } else {
        Some(Arc::clone(&stop_flag))
    });

    let (outcome, pv, cut_short) = {
        let (outcome, pv, _nodes) = search.solve_with_progress(&mut pos, |o, line| {
            eprintln!("outcome: {} length: {}", o.as_str(), line.len());
        });

        println!("outcome: {} length: {}", outcome.as_str(), pv.len());
        if outcome != Outcome::Draw {
            println!("pv: {}", pv_str(&pv));
            // Qualifies the PV length, not its validity: `proven-shortest`
            // means no shorter decisive line exists within the solver's
            // search semantics.
            let label = match search.pv_status() {
                PvStatus::ProvenShortest => "proven-shortest",
                PvStatus::FirstOutcome => "first-outcome",
                PvStatus::Unproven if search.last_refine_round_cap_cut() => "cap-cut",
                PvStatus::Unproven => "cut-short",
                // Unreachable for a decisive outcome; treat defensively as
                // the unrefined first-outcome line.
                PvStatus::None => "first-outcome",
            };
            println!("pv_status: {label}");
        }

        let budget_exhausted = matches!(search.exit_reason(), ExitReason::BudgetExhausted);
        (outcome, pv, search.time_exceeded() || budget_exhausted)
    };

    // Write the TT snapshot on the normal exit path. Written regardless of
    // `--outcome-only`; a failed debug artifact must not turn a good search
    // result into a failure, so I/O errors are logged and the exit status is
    // unchanged.
    if let Some(tt_dump_path) = &tt_dump_path {
        match std::fs::File::create(tt_dump_path) {
            Ok(file) => {
                let mut writer = std::io::BufWriter::new(file);
                let tt_size_mb = tt_size.min(u32::MAX as usize) as u32;
                match write_tt_snapshot(search.tt(), &fen, tt_size_mb, &mut writer) {
                    Ok(summary) => {
                        if let Err(e) = std::io::Write::flush(&mut writer) {
                            eprintln!("failed to flush TT snapshot to {tt_dump_path}: {e}");
                        } else {
                            println!(
                                "tt_snapshot: {tt_dump_path} solved={} unsolved={} bytes={}",
                                summary.solved, summary.unsolved, summary.bytes
                            );
                        }
                    }
                    Err(e) => eprintln!("failed to write TT snapshot to {tt_dump_path}: {e}"),
                }
            }
            Err(e) => eprintln!("failed to create TT snapshot file {tt_dump_path}: {e}"),
        }
    }

    if cut_short {
        match search.exit_reason() {
            ExitReason::Quit => println!("quit"),
            ExitReason::BudgetExhausted => println!("budget exhausted"),
            _ => println!("timeout"),
        };
    }

    if let Some(hook) = hook {
        hook(search.exit_reason(), outcome, search.nodes(), &pv);
    }

    drop(search);
}
