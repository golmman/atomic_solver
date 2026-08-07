//! Command-line solver for atomic chess.
//!
//! This file is larger than 10 KiB because it contains the argument parsing,
//! help text, search setup, proof-tree wiring, and pre-exit hook in one place
//! so the binary is self-contained.
//!
//! Usage:
//!   atomic_solver [OPTIONS]
//!
//! Options:
//!   -h, --help                 Show this help message and exit.
//!   --fen <FEN>                Position to solve in Forsyth-Edwards Notation.
//!                              Defaults to [`Position::STARTPOS_FEN`].
//!   --tt-size <MB>             Transposition-table size in megabytes.
//!                              Defaults to 64.
//!   --epsilon <VALUE>          DF-PN+ threshold parameter in the range [0.0, 1.0].
//!                              Defaults to 0.125.
//!   --timeout <SECONDS>        Search time limit in seconds.
//!                              Defaults to 5.
//!   --first-outcome             Stop after the first decisive outcome and skip
//!                              the iterative PV refinement.
//!   --outcome-only             Print only the outcome/PV and skip the pre-exit
//!                              summary. No stdin reader is spawned.
//!   --pt-size <MB>             Maximum in-memory proof-tree size in megabytes.
//!                              Defaults to 256.
//!   --dump-path <FILE>         Path for the compact binary proof-tree dump.
//!                              Defaults to `proof_tree.bin`.
//!
//! Output:
//!   Each newly discovered decisive line is logged as
//!   `outcome: <win|loss|draw> length: <plies>`. For wins and losses the final
//!   line is followed by `pv: <UCI moves>`, an informational best-effort line
//!   from the transposition table. If the timeout is reached after any result,
//!   `timeout` is printed on its own line. The pre-exit hook writes the
//!   accumulated proof tree to `proof_tree.bin`.
//!
//! Examples:
//!   atomic_solver --help
//!   atomic_solver --fen "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1"
//!   atomic_solver --epsilon 0.5 --first-outcome
//!   atomic_solver --timeout 10

use atomic_movegen::types::Move;
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofMessage, ProofResponse, ProofTreeWorker};
use atomic_solver::search::dfpn::{ExitReason, Search};
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
    println!("                             (default: 64)");
    println!("  --epsilon <VALUE>          DF-PN+ threshold parameter in [0.0, 1.0]");
    println!("                             (default: 0.125)");
    println!("  --timeout <SECONDS>        Search time limit in seconds");
    println!("                             (default: 5)");
    println!("  --first-outcome            Stop after the first decisive outcome");
    println!("                             and skip iterative PV refinement");
    println!("  --outcome-only             Print only the outcome/PV;");
    println!("                             do not spawn stdin reader or pre-exit hook");
    println!("  --pt-size <MB>             Maximum in-memory proof-tree size in megabytes");
    println!("                             (default: 256)");
    println!("  --dump-path <FILE>         Path for the compact binary proof-tree dump");
    println!("                             (default: proof_tree.bin)");
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
        outcome_only,
        pt_size,
        dump_path,
    } = opts;

    let mut pos = Position::from_fen(&fen).unwrap_or_else(|e| {
        eprintln!("Failed to parse FEN: {e}");
        std::process::exit(1);
    });

    let mut search = Search::new(tt_size);
    search.set_timeout(timeout);
    search.set_epsilon(epsilon);
    search.set_first_outcome_only(first_outcome);

    let stop_flag = Arc::new(AtomicBool::new(false));
    let memory_limited = Arc::new(AtomicBool::new(false));
    let (proof_tx, proof_handle) = if outcome_only {
        (None, None)
    } else {
        let (tx, handle) =
            ProofTreeWorker::spawn(fen.clone(), pt_size, Arc::clone(&memory_limited));
        (Some(tx), Some(handle))
    };

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

        let hook_tx = proof_tx.as_ref().unwrap().clone();
        Some(Box::new(move |reason, outcome, nodes, _pv: &[Move]| {
            println!("pre_exit: reason={reason} outcome={outcome} nodes={nodes}");

            let (stats_tx, stats_rx) = std::sync::mpsc::channel();
            if let Err(e) = hook_tx.send(ProofMessage::GetStats(stats_tx)) {
                eprintln!("failed to request proof-tree stats: {e}");
                return;
            }
            if let Ok(ProofResponse::Stats(stats)) = stats_rx.recv() {
                println!(
                    "proof_tree: nodes={} win={} loss={} root_depth={}",
                    stats.nodes, stats.win_nodes, stats.loss_nodes, stats.root_depth
                );
            }

            let (tree_tx, tree_rx) = std::sync::mpsc::channel();
            if let Err(e) = hook_tx.send(ProofMessage::GetTree(tree_tx)) {
                eprintln!("failed to request proof tree: {e}");
                return;
            }
            if let Ok(ProofResponse::Tree(tree)) = tree_rx.recv() {
                if let Err(e) =
                    std::fs::File::create(&dump_path).and_then(|mut file| tree.to_bin(&mut file))
                {
                    eprintln!("failed to write proof-tree dump to {dump_path}: {e}");
                } else {
                    println!("proof_tree_dump: {dump_path}");
                }
            }
        }))
    };

    search.set_stop_flag(if outcome_only {
        None
    } else {
        Some(Arc::clone(&stop_flag))
    });
    search.set_memory_limited(if outcome_only {
        None
    } else {
        Some(Arc::clone(&memory_limited))
    });
    search.set_proof_tree_sender(proof_tx.clone());

    let (outcome, pv, timed_out) = {
        let (outcome, pv, _nodes) = search.solve_with_progress(&mut pos, |o, line| {
            eprintln!("outcome: {} length: {}", o.as_str(), line.len());
        });

        println!("outcome: {} length: {}", outcome.as_str(), pv.len());
        if outcome != Outcome::Draw {
            println!("pv: {}", pv_str(&pv));
        }

        (outcome, pv, search.time_exceeded())
    };

    if timed_out {
        let msg = match search.exit_reason() {
            ExitReason::Quit => "quit",
            ExitReason::MemoryLimit => "memory",
            _ => "timeout",
        };
        println!("{msg}");
    }

    if let Some(hook) = hook {
        hook(search.exit_reason(), outcome, search.nodes(), &pv);
    }

    drop(search);
    drop(proof_tx);
    if let Some(handle) = proof_handle {
        let _ = handle.join();
    }
}
