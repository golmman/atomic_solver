//! Command-line solver for atomic chess.
//!
//! This file is larger than 10 KiB because it contains the argument parsing,
//! help text, search setup, and result printing in one place so the binary is
//! self-contained.
//!
//! Usage:
//!   `atomic_solver` [OPTIONS]
//!
//! The user-facing option text is `print_help()` (`--help`); the defaults
//! live in `CliOptions::default()` (`src/cli.rs`), and the per-option
//! contract for flags with non-obvious resource behaviour is stated next to
//! the field that implements it. The output contract below is normative.
//!
//! Output:
//!   Each newly discovered decisive line is logged as
//!   `outcome: <win|loss|draw> length: <plies>`. For wins and losses the final
//!   line is followed by `pv: <UCI moves>`, an informational best-effort line
//!   from the transposition table, and `pv_status: <label>` describing whether
//!   the PV is proven shortest (`proven-shortest`), the unrefined first
//!   outcome (`first-outcome`), cut by the refinement work cap (`cap-cut`),
//!   cut by a global resource limit / not shorter (`cut-short`), or extracted
//!   from a replay-verified pre-phase certificate (`preflight-proof`).
//!   `pv_status` qualifies the PV length, not its validity. Without
//!   `--outcome-only` the pre-phase hook prints one
//!   `preflight: decided|deferred ...` line before the outcome, and the
//!   pre-exit hook prints a `pre_exit:` summary line. A root the cheap
//!   screening probe already solved prints
//!   `preflight: deferred reason=probe-solved evals=<n>`; its outcome then
//!   comes from the ordinary search and its `pv_status` is that search's
//!   (`first-outcome` / `proven-shortest` / ...), never `preflight-proof`.
//!   On stderr the total
//!   child-evaluation count is reported as one final `evals: <n>` line (in
//!   both modes); it includes any pre-phase evals (pre-probe evals
//!   included).
//!
//! Examples:
//!   `atomic_solver` --help
//!   `atomic_solver` --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
//!   `atomic_solver` --epsilon 0.5 --first-outcome
//!   `atomic_solver` --timeout 10

use atomic_movegen::types::Move;
use atomic_solver::config;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::{ExitReason, PvStatus, Search};
use atomic_solver::search::ordering::StaticAtomicScorer;
use atomic_solver::search::preflight::PreflightReport;
use atomic_solver::tt_snapshot::write_tt_snapshot;
use std::io::BufRead;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

mod cli;
use cli::{CliOptions, ParseResult, parse_args};

/// The `--help` text; `{program}` is replaced with the invoked name.
const HELP: &str = r#"atomic chess solver

Usage:
  {program} [OPTIONS]

Options:
  -h, --help                 Show this help message and exit
  --fen <FEN>                Position in Forsyth-Edwards Notation
                             (default: standard atomic start position)
  --tt-size <MB>             Transposition-table size in megabytes
                             (default: 256; never allocates more
                             than requested, and pages are populated
                             on first use; RSS is bounded by, not
                             equal to, the label)
  --epsilon <VALUE>          DF-PN+ threshold parameter in [0.0, 1.0]
                             (default: 0.125)
  --timeout <SECONDS>        Search time limit in seconds
                             (default: 5)
  --first-outcome            Stop after the first decisive outcome
                             and skip iterative PV refinement
  --refine-cap <FACTOR>      Per-refinement-round work-cap factor vs.
                             the first-outcome eval count; a round that
                             hits the cap without a shorter decisive
                             line is abandoned. 0 disables capping
                             (default: 0.25)
  --outcome-only             Print only the outcome/PV;
                             no stdin reader and no pre-exit summary
  --no-preflight             Disable the bounded pre-phase that
                             decides small-space positions (occupied
                             <= 3 men, no pawns/castling) before the
                             DF-PN loop, and the cheap probe that
                             screens such roots first
  --tt-dump-path <FILE>      Write a binary TT snapshot after the search
                             (transfer artifact for offline proof
                             reconstruction via reconstruct_pt; optional)
  --salt <U64>               TT bucket-index salt; 0 (default) keeps
                             shipped indexing, s > 0 remaps bucket
                             sharing without changing semantics
  --budget <EVALS>           Cap the search at this many child
                             evaluations; hitting the cap returns
                             draw and prints 'budget exhausted'
  --move-class-pn            Give newly seen OR-node children
                             provari-style move-class starting proof
                             numbers (checks by evasions, double
                             threat 1, threat 4, weak threat 8,
                             quiet 9, hanging 15). Experimental
                             heuristic; off by default
  --quiet-grouping           Hide OR-node plain moves behind one
                             delayed group child, opened only if the
                             search selects it. Experimental
                             heuristic; off by default
  --static-tactics           Claim a legal capture beside the enemy
                             commoner as a static one-move win at node
                             entry (sound, no child evaluations
                             billed). Experimental; off by default
  --config <FILE>            Path to a TOML file overriding scorer
                             parameters; defaults to built-in values

Examples:
  {program} --help
  {program} --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
  {program} --epsilon 0.5 --first-outcome
  {program} --timeout 10"#;

fn print_help(program: &str) {
    println!("{}", HELP.replace("{program}", program));
}

fn pv_str(pv: &[Move]) -> String {
    pv.iter()
        .copied()
        .map(move_to_uci)
        .collect::<Vec<_>>()
        .join(" ")
}

/// One `preflight:` line for the pre-phase hook (plan13 R5), same spirit as
/// the `pre_exit:` summary. Only printed in non-`--outcome-only` mode.
fn print_preflight_line(report: &PreflightReport) {
    if report.decided {
        println!(
            "preflight: decided outcome={} evals={} region={} rank={}",
            report.outcome.map_or("none", |o| o.as_str()),
            report.evals,
            report.region,
            report.rank,
        );
    } else {
        println!(
            "preflight: deferred reason={} evals={}",
            report.reason, report.evals,
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args.first().map_or("atomic_solver", String::as_str);

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
        no_preflight,
        salt,
        budget,
        move_class_pn,
        quiet_grouping,
        static_tactics,
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
    search.set_preflight_enabled(!no_preflight);
    search.set_move_class_pn(move_class_pn);
    search.set_quiet_grouping(quiet_grouping);
    search.set_static_tactics(static_tactics);
    search.set_salt(salt);
    if let Some(budget) = budget {
        search.set_child_eval_budget(budget);
    }

    let stop_flag = Arc::new(AtomicBool::new(false));

    if !outcome_only {
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
    }

    search.set_stop_flag(if outcome_only {
        None
    } else {
        Some(Arc::clone(&stop_flag))
    });

    let (outcome, cut_short) = {
        let (outcome, pv, _nodes) = search.solve_with_progress(&mut pos, |o, line| {
            eprintln!("outcome: {} length: {}", o.as_str(), line.len());
        });

        if !outcome_only && let Some(report) = search.preflight_report() {
            print_preflight_line(&report);
        }
        println!("outcome: {} length: {}", outcome.as_str(), pv.len());
        if outcome != Outcome::Draw {
            println!("pv: {}", pv_str(&pv));
            // Qualifies the PV length, not its validity: `proven-shortest`
            // means no shorter decisive line exists within the solver's
            // search semantics.
            let label = match search.pv_status() {
                PvStatus::ProvenShortest => "proven-shortest",
                PvStatus::FirstOutcome => "first-outcome",
                PvStatus::PreflightProof => "preflight-proof",
                PvStatus::Unproven if search.last_refine_round_cap_cut() => "cap-cut",
                PvStatus::Unproven => "cut-short",
                // Unreachable for a decisive outcome; treat defensively as
                // the unrefined first-outcome line.
                PvStatus::None => "first-outcome",
            };
            println!("pv_status: {label}");
        }

        let budget_exhausted = matches!(search.exit_reason(), ExitReason::BudgetExhausted);
        (outcome, search.time_exceeded() || budget_exhausted)
    };
    let search_exit_reason = search.exit_reason();
    let search_nodes = search.nodes();

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
        println!(
            "{}",
            match search.exit_reason() {
                ExitReason::Quit => "quit",
                ExitReason::BudgetExhausted => "budget exhausted",
                _ => "timeout",
            }
        );
    }

    // Total child evaluations (including any pre-phase evals) on stderr, in
    // both modes: the machine-readable work metric for gate/optimizer
    // drivers. Kept off stdout so the outcome-only stdout contract stays
    // byte-exact.
    eprintln!("evals: {}", search.child_evaluations());

    drop(search);

    if !outcome_only {
        println!("pre_exit: reason={search_exit_reason} outcome={outcome} nodes={search_nodes}");
    }
}
