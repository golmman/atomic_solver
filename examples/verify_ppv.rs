//! Verify that a supplied move list is a Proof Principal Variation (PPV).
//!
//! Usage:
//!     cargo run --example verify_ppv -- --fen <FEN> --moves "<UCI moves>" --timeout <SEC>
//!
//! The example prints `is_ppv: true` when every defender reply can be refuted
//! within the remaining PPV length, and `is_ppv: false` otherwise.

mod common;

use std::process;
use std::time::{Duration, Instant};

use atomic_movegen::types::{Move, MoveList};
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use atomic_solver::zobrist;

fn print_help(program: &str) {
    println!("verify a supplied Proof Principal Variation");
    println!();
    println!("Usage:");
    println!("  {program} [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -h, --help          Show this help message and exit");
    println!("  --fen <FEN>         Position in Forsyth-Edwards Notation");
    println!("                      (default: standard atomic start position)");
    println!("  --moves <MOVES>     Space-separated UCI move list");
    println!("  --timeout <SECONDS> Maximum total wall time in seconds");
    println!("                      (default: 60)");
}

fn outcome_str(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Win => "win",
        Outcome::Loss => "loss",
        Outcome::Draw => "draw",
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or("verify_ppv");

    let mut fen = Position::STARTPOS_FEN.to_string();
    let mut timeout: u64 = 60;
    let mut move_args: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help(program);
                process::exit(0);
            }
            "--fen" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --fen requires a value");
                    println!("is_ppv: false");
                    process::exit(1);
                }
                fen = args[i + 1].clone();
                i += 2;
            }
            "--timeout" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --timeout requires a value");
                    println!("is_ppv: false");
                    process::exit(1);
                }
                match args[i + 1].parse::<u64>() {
                    Ok(v) if v > 0 => timeout = v,
                    Ok(v) => {
                        eprintln!("error: timeout must be positive, got {v}");
                        println!("is_ppv: false");
                        process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("error: invalid timeout value: {e}");
                        println!("is_ppv: false");
                        process::exit(1);
                    }
                }
                i += 2;
            }
            "--moves" => {
                i += 1;
                let mut found = false;
                while i < args.len() {
                    if args[i].starts_with('-') {
                        break;
                    }
                    for token in args[i].split_whitespace() {
                        move_args.push(token.to_string());
                    }
                    found = true;
                    i += 1;
                }
                if !found {
                    eprintln!("error: --moves requires at least one move");
                    println!("is_ppv: false");
                    process::exit(1);
                }
            }
            _ => {
                eprintln!("error: unknown option '{}'", args[i]);
                eprintln!("Run '{program} --help' for usage.");
                println!("is_ppv: false");
                process::exit(1);
            }
        }
    }

    if move_args.is_empty() {
        eprintln!("error: --moves is required");
        println!("is_ppv: false");
        process::exit(1);
    }

    let mut positions = vec![Position::from_fen(&fen).unwrap_or_else(|e| {
        eprintln!("error: failed to parse FEN: {e}");
        println!("is_ppv: false");
        process::exit(1);
    })];
    let mut supplied_moves: Vec<Move> = Vec::with_capacity(move_args.len());
    let mut path_codes: Vec<u64> = Vec::with_capacity(move_args.len() + 1);
    path_codes.push(0);

    let n = move_args.len();
    for (i, token) in move_args.iter().enumerate() {
        if positions[i].outcome().is_some() && i < n {
            eprintln!(
                "error: position at ply {} is terminal before all moves are consumed",
                i
            );
            println!("is_ppv: false");
            process::exit(1);
        }

        let m = common::parse_uci(&positions[i], token).unwrap_or_else(|| {
            eprintln!("error: move '{}' at ply {} is not legal", token, i + 1);
            println!("is_ppv: false");
            process::exit(1);
        });

        let mut next = positions[i].clone();
        next.do_move(m);
        positions.push(next);
        supplied_moves.push(m);

        path_codes.push(path_codes[i] ^ zobrist::path_random(m, i + 1));
    }

    let final_outcome = positions[n].outcome().unwrap_or_else(|| {
        eprintln!("error: final position is not decisive (outcome: draw)");
        println!("is_ppv: false");
        process::exit(1);
    });

    let root_outcome = if n.is_multiple_of(2) {
        final_outcome
    } else {
        final_outcome.flip()
    };

    if root_outcome == Outcome::Draw {
        eprintln!(
            "error: final position is not decisive (outcome: {})",
            outcome_str(root_outcome)
        );
        println!("is_ppv: false");
        process::exit(1);
    }

    println!("moves: {}", n);
    println!("outcome: {}", outcome_str(root_outcome));

    let attacker_color = if root_outcome == Outcome::Win {
        positions[0].side_to_move()
    } else {
        positions[0].side_to_move().flip()
    };

    let start = Instant::now();
    let global_deadline = start + Duration::from_secs(timeout);
    let mut search = Search::new(256);
    let mut total_nodes: u64 = 0;

    for i in (0..n).rev() {
        if positions[i].side_to_move() == attacker_color {
            continue;
        }

        let next_remaining = n - i - 1;
        let mut replies = MoveList::new();
        positions[i].legal_moves(&mut replies);
        let legal_count = replies.len();

        eprintln!(
            "checking defender ply {}/{} ({} replies)",
            i + 1,
            n,
            legal_count
        );

        let mut max_reply_depth: u32 = 0;
        let mut chosen_depth: Option<u32> = None;

        for j in 0..legal_count {
            let m = replies[j];
            let mut child = positions[i].clone();
            child.do_move(m);

            let prefix_keys: Vec<u64> = positions[0..=i]
                .iter()
                .map(|p| p.repetition_key())
                .collect();
            let prefix_path_code = path_codes[i];

            let wall_remaining = global_deadline.saturating_duration_since(Instant::now());
            if wall_remaining.is_zero() {
                eprintln!(
                    "error: timeout before verifying defender ply {}/{}",
                    i + 1,
                    n
                );
                println!("is_ppv: false");
                process::exit(1);
            }
            search.set_timeout(wall_remaining.as_secs().max(1));

            let (outcome, depth, nodes) = search.search_depth_with_prefix(
                &mut child,
                next_remaining as u32,
                &prefix_keys,
                prefix_path_code,
            );
            total_nodes += nodes;

            if outcome != Outcome::Win {
                let reply_uci = move_to_uci(m);
                let supplied_uci = &move_args[i];
                eprintln!(
                    "PPV refuted at defender ply {}/{}, supplied move '{}': reply '{}' not proven lost within {} plies (outcome: {:?})",
                    i + 1,
                    n,
                    supplied_uci,
                    reply_uci,
                    next_remaining,
                    outcome
                );
                println!("is_ppv: false");
                process::exit(1);
            }

            if m == supplied_moves[i] {
                chosen_depth = Some(depth);
            }
            max_reply_depth = max_reply_depth.max(depth);
        }

        let chosen_depth = chosen_depth.unwrap_or_else(|| {
            let supplied_uci = &move_args[i];
            eprintln!(
                "PPV refuted at defender ply {}/{}, supplied move '{}' is not among the legal replies",
                i + 1,
                n,
                supplied_uci
            );
            println!("is_ppv: false");
            process::exit(1);
        });

        if chosen_depth != max_reply_depth {
            let supplied_uci = &move_args[i];
            eprintln!(
                "PPV refuted at defender ply {}/{}, supplied move '{}' is not a longest defense (depth {}, longest {})",
                i + 1,
                n,
                supplied_uci,
                chosen_depth,
                max_reply_depth
            );
            println!("is_ppv: false");
            process::exit(1);
        }
    }

    let elapsed = start.elapsed();
    eprintln!(
        "elapsed: {:.3}s, nodes: {}",
        elapsed.as_secs_f64(),
        total_nodes
    );
    println!("is_ppv: true");
}
