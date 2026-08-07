//! Benchmark `max_work` chunk growth with a configurable factor on one position.
//!
//! This file is larger than 10 KiB because it contains argument parsing, timing,
//! and formatted output for a standalone diagnostic example.

use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use std::time::{Duration, Instant};

struct Run {
    elapsed: Duration,
    outcome: Outcome,
    nodes: u64,
    child_evals: u64,
}

struct ChunkResult {
    outcome: Outcome,
    nodes: u64,
    child_evals: u64,
    mean: Duration,
    min: Duration,
    max: Duration,
}

fn main() {
    let mut fen = "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22".to_string();
    let mut timeout = 60u64;
    let mut runs = 5usize;
    let mut factors: Vec<f64> = Vec::new();
    let mut linear = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fen" => {
                fen = args.get(i + 1).expect("--fen requires a value").to_string();
                i += 2;
            }
            "--timeout" => {
                timeout = args
                    .get(i + 1)
                    .expect("--timeout requires a value")
                    .parse()
                    .expect("--timeout needs a positive integer");
                i += 2;
            }
            "--runs" => {
                runs = args
                    .get(i + 1)
                    .expect("--runs requires a value")
                    .parse()
                    .expect("--runs needs a positive integer");
                i += 2;
            }
            "--factor" => {
                let v: f64 = args
                    .get(i + 1)
                    .expect("--factor requires a value")
                    .parse()
                    .expect("--factor needs a number >= 1.0");
                assert!(v >= 1.0, "--factor must be >= 1.0");
                factors.push(v);
                i += 2;
            }
            "--linear" => {
                linear = true;
                i += 1;
            }
            other => panic!("unknown option {other}"),
        }
    }

    if factors.is_empty() {
        factors.push(2.0);
    }

    println!("fen={fen}");
    println!("timeout={timeout}s runs={runs}");
    println!();

    let mut results: Vec<(String, ChunkResult)> = Vec::new();

    if linear {
        let (label, r) = bench_mode(&fen, timeout, runs, None);
        results.push((label, r));
    }

    for factor in factors {
        let (label, r) = bench_mode(&fen, timeout, runs, Some(factor));
        results.push((label, r));
    }

    print_results_table(&results);
}

fn bench_mode(fen: &str, timeout: u64, runs: usize, factor: Option<f64>) -> (String, ChunkResult) {
    let (_, warmup_frac) = run_once(fen, timeout, factor);

    let mut times = Vec::with_capacity(runs);
    let mut first: Option<Run> = None;

    let label = if let Some(f) = factor {
        if let Some((num, den)) = warmup_frac {
            format!("factor {num}/{den}")
        } else {
            format!("factor {f}")
        }
    } else {
        "linear".to_string()
    };
    println!("benchmarking {label} chunks ...");

    for _ in 0..runs {
        let (run, _) = run_once(fen, timeout, factor);
        times.push(run.elapsed);
        if first.is_none() {
            first = Some(run);
        }
    }

    let first = first.unwrap();
    let total: u128 = times.iter().map(Duration::as_nanos).sum();
    let mean = Duration::from_nanos((total / runs as u128) as u64);
    let min = *times.iter().min_by_key(|d| d.as_nanos()).unwrap();
    let max = *times.iter().max_by_key(|d| d.as_nanos()).unwrap();

    let result = ChunkResult {
        outcome: first.outcome,
        nodes: first.nodes,
        child_evals: first.child_evals,
        mean,
        min,
        max,
    };
    (label, result)
}

fn run_once(fen: &str, timeout: u64, factor: Option<f64>) -> (Run, Option<(u64, u64)>) {
    let mut pos = Position::from_fen(fen).expect("valid FEN");
    let mut search = Search::new(64);
    search.set_timeout(timeout);
    search.set_epsilon(0.125);

    let fraction = match factor {
        None => {
            search.set_linear_chunks(true);
            None
        }
        Some(f) => Some(search.set_chunk_multiplier_from_factor(f)),
    };

    let start = Instant::now();
    let (outcome, _pv, _nodes) = search.search_depth(&mut pos, u32::MAX);
    let elapsed = start.elapsed();

    let run = Run {
        elapsed,
        outcome,
        nodes: search.nodes(),
        child_evals: search.child_evaluations(),
    };
    (run, fraction)
}

fn print_results_table(results: &[(String, ChunkResult)]) {
    const GROWTH_W: usize = 14;
    const OUTCOME_W: usize = 7;
    const MEAN_W: usize = 8;
    const MIN_W: usize = 7;
    const MAX_W: usize = 7;
    const NODES_W: usize = 13;
    const CHILD_W: usize = 11;

    println!();
    println!(
        "| {:<GROWTH_W$} | {:<OUTCOME_W$} | {:>MEAN_W$} | {:>MIN_W$} | {:>MAX_W$} | {:>NODES_W$} | {:>CHILD_W$} |",
        "growth", "outcome", "mean (s)", "min (s)", "max (s)", "nodes", "child evals"
    );
    println!(
        "| {} | {} | {} | {} | {} | {} | {} |",
        sep(GROWTH_W, false),
        sep(OUTCOME_W, false),
        sep(MEAN_W, true),
        sep(MIN_W, true),
        sep(MAX_W, true),
        sep(NODES_W, true),
        sep(CHILD_W, true),
    );
    for (name, r) in results {
        println!(
            "| {:<GROWTH_W$} | {:<OUTCOME_W$} | {:>MEAN_W$.3} | {:>MIN_W$.3} | {:>MAX_W$.3} | {:>NODES_W$} | {:>CHILD_W$} |",
            name,
            r.outcome.as_str(),
            r.mean.as_secs_f64(),
            r.min.as_secs_f64(),
            r.max.as_secs_f64(),
            r.nodes,
            r.child_evals
        );
    }
}

fn sep(width: usize, right: bool) -> String {
    if right {
        format!("{:->width$}", ":")
    } else {
        format!("{:-<width$}", ":")
    }
}
