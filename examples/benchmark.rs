//! Reproducible benchmark harness for the atomic-chess solver.
//!
//! Run with:
//!     cargo run --release --example benchmark -- --runs 10 --refine-shortest

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use std::time::{Duration, Instant};

struct Case {
    name: &'static str,
    fen: &'static str,
}

const SUITE: &[Case] = &[
    // Quick sanity checks from the speed reports.
    Case {
        name: "two_rook_mate",
        fen: "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1",
    },
    Case {
        name: "epsilon_mate",
        fen: "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3",
    },
    Case {
        name: "promotion_transposition",
        fen: "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1",
    },
    // Longer decisive positions (expected >2 s, <5 s with default timeout).
    Case {
        name: "m26",
        fen: "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26",
    },
    Case {
        name: "opening_f2",
        fen: "rnbqkbnr/ppppp1pp/5p2/8/8/4P3/PPPP1PPP/RNBQKBNR w KQkq - 0 2",
    },
    Case {
        name: "rook_pawn_endgame",
        fen: "8/4Pk2/8/8/8/8/PP2K1p1/6R1 w - - 1 28",
    },
    // Timeout-limited regression position.
    Case {
        name: "m19",
        fen: "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19",
    },
    // Standard starting position.
    Case {
        name: "startpos",
        fen: Position::STARTPOS_FEN,
    },
];

struct Run {
    elapsed: Duration,
    outcome: Outcome,
    nodes: u64,
    child_evals: u64,
    pv: Vec<String>,
}

struct BenchResult {
    outcome: Outcome,
    nodes: u64,
    child_evals: u64,
    pv: Vec<String>,
    mean: Duration,
    min: Duration,
    max: Duration,
}

fn main() {
    let mut runs = 10usize;
    let mut timeout = 5u64;
    let mut epsilon = 0.125f64;
    let mut refine_shortest = false;
    let mut filter: Option<String> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--runs" => {
                runs = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .expect("--runs needs a positive integer");
                i += 2;
            }
            "--timeout" => {
                timeout = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .expect("--timeout needs a positive integer");
                i += 2;
            }
            "--epsilon" => {
                epsilon = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .expect("--epsilon needs a number in [0,1]");
                i += 2;
            }
            "--refine-shortest" => {
                refine_shortest = true;
                i += 1;
            }
            other => {
                filter = Some(other.to_string());
                i += 1;
            }
        }
    }

    println!("runs={runs} timeout={timeout}s epsilon={epsilon} refine_shortest={refine_shortest}");
    println!();

    let mut results = Vec::new();
    for case in SUITE {
        if let Some(ref f) = filter
            && !case.name.contains(f)
        {
            continue;
        }
        println!("benchmarking {} ...", case.name);
        results.push((
            case.name,
            bench_case(case, runs, timeout, epsilon, refine_shortest),
        ));
    }

    println!();
    println!("| name | outcome | nodes | child_evals | mean (s) | min (s) | max (s) | pv_len |");
    println!("|------|---------|------:|------------:|---------:|--------:|--------:|-------:|");
    for (name, r) in &results {
        let mean = r.mean.as_secs_f64();
        let min = r.min.as_secs_f64();
        let max = r.max.as_secs_f64();
        let pv_len = r.pv.len();
        let outcome = outcome_str(r.outcome);
        println!(
            "| {name} | {outcome} | {} | {} | {mean:.3} | {min:.3} | {max:.3} | {pv_len} |",
            r.nodes, r.child_evals
        );
    }
}

fn bench_case(case: &Case, runs: usize, timeout: u64, epsilon: f64, refine: bool) -> BenchResult {
    // Warm-up run, excluded from statistics (matching the report style).
    let _ = run_once(case.fen, timeout, epsilon, refine);

    let mut times = Vec::with_capacity(runs);
    let mut first: Option<Run> = None;

    for _ in 0..runs {
        let run = run_once(case.fen, timeout, epsilon, refine);
        times.push(run.elapsed);
        if first.is_none() {
            first = Some(run);
        }
    }

    let first = first.unwrap();
    let nanos: Vec<u128> = times.iter().map(|d| d.as_nanos()).collect();
    let total: u128 = nanos.iter().sum();
    let mean = Duration::from_nanos((total / runs as u128) as u64);
    let min = *times.iter().min_by_key(|d| d.as_nanos()).unwrap();
    let max = *times.iter().max_by_key(|d| d.as_nanos()).unwrap();

    BenchResult {
        outcome: first.outcome,
        nodes: first.nodes,
        child_evals: first.child_evals,
        pv: first.pv,
        mean,
        min,
        max,
    }
}

fn run_once(fen: &str, timeout: u64, epsilon: f64, refine: bool) -> Run {
    let mut pos = Position::from_fen(fen).expect("valid FEN");
    let mut search = Search::new(64);
    search.set_timeout(timeout);
    search.set_epsilon(epsilon);
    search.refine_shortest(refine);

    let start = Instant::now();
    let (outcome, pv, nodes) = search.solve(&mut pos);
    let elapsed = start.elapsed();

    let child_evals = search.child_evaluations();
    let pv = pv.iter().map(|&m| move_to_uci(m)).collect();

    Run {
        elapsed,
        outcome,
        nodes,
        child_evals,
        pv,
    }
}

fn outcome_str(o: Outcome) -> &'static str {
    match o {
        Outcome::Win => "win",
        Outcome::Loss => "loss",
        Outcome::Draw => "draw",
    }
}
