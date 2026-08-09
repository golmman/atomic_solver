//! Reproducible benchmark harness for the atomic-chess solver.
//!
//! This file is larger than the 10 KiB guideline because it contains the
//! benchmark harness, suite selection, fixture parsing, and expected-outcome
//! reporting in one place; splitting it would fragment the argument-parsing and
//! result-formatting logic.
//!
//! Run with:
//!     cargo run --release --example benchmark -- --runs 10
//!     cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 1
//!     cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5

mod common;

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct Case {
    name: String,
    fen: String,
    expected: Option<Outcome>,
    note: Option<String>,
}

enum Suite {
    Default,
    MoveOrder,
    All,
}

struct Run {
    elapsed: Duration,
    outcome: Outcome,
    nodes: u64,
    child_evals: u64,
    pv: Vec<String>,
}

struct BenchResult {
    name: String,
    expected: Option<Outcome>,
    note: Option<String>,
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
    let mut suite = Suite::Default;
    let mut first_outcome = false;
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
            "--suite" => {
                let value = args.get(i + 1).expect("--suite needs an argument");
                suite = match value.as_str() {
                    "default" => Suite::Default,
                    "move-order" => Suite::MoveOrder,
                    "all" => Suite::All,
                    other => {
                        panic!("unknown suite '{other}'; try 'default', 'move-order', or 'all'")
                    }
                };
                i += 2;
            }
            "--first-outcome" => {
                first_outcome = true;
                i += 1;
            }
            other => {
                filter = Some(other.to_string());
                i += 1;
            }
        }
    }

    let cases = load_suite(&suite);

    let mode = if first_outcome {
        "first-outcome"
    } else {
        "refined"
    };
    println!(
        "runs={runs} timeout={timeout}s epsilon={epsilon} suite={} mode={mode}",
        suite_name(&suite)
    );
    println!();

    let mut results = Vec::new();
    let mut wrong = 0usize;
    for case in cases {
        if let Some(ref f) = filter
            && !case.name.contains(f)
        {
            continue;
        }
        println!("benchmarking {} ...", case.name);
        let result = bench_case(&case, runs, timeout, epsilon, first_outcome);
        if let Some(expected) = result.expected
            && result.outcome != Outcome::Draw
            && result.outcome != expected
        {
            wrong += 1;
        }
        results.push(result);
    }

    print_table(&results, first_outcome);

    if wrong > 0 {
        eprintln!(
            "\nerror: {wrong} position(s) returned a decisive outcome that does not match the expected value"
        );
        std::process::exit(1);
    }
}

fn suite_name(suite: &Suite) -> &'static str {
    match suite {
        Suite::Default => "default",
        Suite::MoveOrder => "move-order",
        Suite::All => "all",
    }
}

fn load_suite(suite: &Suite) -> Vec<Case> {
    match suite {
        Suite::Default => default_suite(),
        Suite::MoveOrder => move_order_suite(),
        Suite::All => {
            let mut cases = default_suite();
            cases.extend(move_order_suite());
            cases
        }
    }
}

fn default_suite() -> Vec<Case> {
    vec![
        Case {
            name: "two_rook_mate".to_string(),
            fen: "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1".to_string(),
            expected: None,
            note: None,
        },
        Case {
            name: "epsilon_mate".to_string(),
            fen: "rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3".to_string(),
            expected: None,
            note: None,
        },
        Case {
            name: "promotion_transposition".to_string(),
            fen: "4k3/PP6/8/8/8/8/8/4K3 w - - 0 1".to_string(),
            expected: None,
            note: None,
        },
        Case {
            name: "m26".to_string(),
            fen: "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26".to_string(),
            expected: None,
            note: None,
        },
        Case {
            name: "opening_f2".to_string(),
            fen: "rnbqkbnr/ppppp1pp/5p2/8/8/4P3/PPPP1PPP/RNBQKBNR w KQkq - 0 2".to_string(),
            expected: None,
            note: None,
        },
        Case {
            name: "rook_pawn_endgame".to_string(),
            fen: "8/4Pk2/8/8/8/8/PP2K1p1/6R1 w - - 1 28".to_string(),
            expected: None,
            note: None,
        },
        Case {
            name: "m19".to_string(),
            fen: "4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19".to_string(),
            expected: None,
            note: None,
        },
        Case {
            name: "startpos".to_string(),
            fen: Position::STARTPOS_FEN.to_string(),
            expected: None,
            note: None,
        },
    ]
}

fn move_order_suite() -> Vec<Case> {
    common::load_move_order_suite()
        .into_iter()
        .map(|case| Case {
            name: case.name,
            fen: case.fen,
            expected: case.expected,
            note: case.note,
        })
        .collect()
}

fn bench_case(
    case: &Case,
    runs: usize,
    timeout: u64,
    epsilon: f64,
    first_outcome: bool,
) -> BenchResult {
    // Warm-up run, excluded from statistics (matching the report style).
    let _ = run_once(&case.fen, timeout, epsilon, first_outcome);

    let mut times = Vec::with_capacity(runs);
    let mut first: Option<Run> = None;

    for _ in 0..runs {
        let run = run_once(&case.fen, timeout, epsilon, first_outcome);
        times.push(run.elapsed);
        if first.is_none() {
            first = Some(run);
        }
    }

    let first = first.unwrap();
    let nanos: Vec<u128> = times.iter().map(Duration::as_nanos).collect();
    let total: u128 = nanos.iter().sum();
    let mean = Duration::from_nanos((total / runs as u128) as u64);
    let min = *times.iter().min_by_key(|d| d.as_nanos()).unwrap();
    let max = *times.iter().max_by_key(|d| d.as_nanos()).unwrap();

    BenchResult {
        name: case.name.clone(),
        expected: case.expected,
        note: case.note.clone(),
        outcome: first.outcome,
        nodes: first.nodes,
        child_evals: first.child_evals,
        pv: first.pv,
        mean,
        min,
        max,
    }
}

fn run_once(fen: &str, timeout: u64, epsilon: f64, first_outcome: bool) -> Run {
    let mut pos = Position::from_fen(fen).expect("valid FEN");
    let mut search = Search::new(64);
    search.set_timeout(timeout);
    search.set_epsilon(epsilon);
    search.set_first_outcome_only(first_outcome);

    let start = Instant::now();
    let (outcome, pv, nodes) = search.solve(&mut pos);
    let elapsed = start.elapsed();

    let child_evals = search.child_evaluations();
    let pv = pv.iter().copied().map(move_to_uci).collect();

    Run {
        elapsed,
        outcome,
        nodes,
        child_evals,
        pv,
    }
}

fn print_table(results: &[BenchResult], first_outcome: bool) {
    let mode_label = if first_outcome {
        " (first-outcome)"
    } else {
        ""
    };
    println!(
        "| name | status | outcome | expected | nodes | child_evals | mean (s) | min (s) | max (s) | pv_len | note |{mode_label}"
    );
    println!(
        "|------|--------|---------|----------|------:|------------:|---------:|--------:|--------:|-------:|------|"
    );

    for r in results {
        let status = status_text(r);
        let outcome = r.outcome.as_str();
        let expected = r.expected.map(|o| o.as_str()).unwrap_or("");
        let mean = r.mean.as_secs_f64();
        let min = r.min.as_secs_f64();
        let max = r.max.as_secs_f64();
        let pv_len = r.pv.len();
        let note = r.note.as_deref().unwrap_or("");
        println!(
            "| {} | {} | {} | {} | {} | {} | {:.3} | {:.3} | {:.3} | {} | {} |",
            r.name, status, outcome, expected, r.nodes, r.child_evals, mean, min, max, pv_len, note
        );
    }
}

fn status_text(result: &BenchResult) -> &'static str {
    match result.expected {
        None => "",
        Some(expected) => {
            if result.outcome == expected {
                "ok"
            } else if result.outcome == Outcome::Draw {
                "timeout"
            } else {
                "wrong"
            }
        }
    }
}
