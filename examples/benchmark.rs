//! Reproducible benchmark harness for the atomic-chess solver.
//!
//! This file is larger than the 10 KiB guideline because it contains the
//! benchmark harness, suite selection, fixture parsing, JSON output, and
//! expected-outcome reporting in one place; splitting it would fragment the
//! argument-parsing and result-formatting logic.
//!
//! Run with:
//!     cargo run --release --example benchmark -- --runs 10
//!     cargo run --release --example benchmark -- --suite move-order --timeout 10 --runs 1
//!     cargo run --release --example benchmark -- --suite move-order --first-outcome --timeout 5
//!     cargo run --release --example benchmark -- --suite decisive --timeout 5 --runs 1
//!     cargo run --release --example benchmark -- --suite quick --first-outcome --timeout 3 --runs 1 --json
//!     cargo run --release --example benchmark -- --suite thorough --first-outcome --timeout 5 --runs 3 --json
//!     cargo run --release --example benchmark -- --suite move-order --first-outcome --nn-weights data/corpus/weights.v1.bin --json

mod common;

use atomic_solver::config;
use atomic_solver::nn::{NnMoveScorer, NnWeights};
use atomic_solver::notation::move_to_uci;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::search::dfpn::Search;
use atomic_solver::search::ordering::StaticAtomicScorer;
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct Case {
    name: String,
    fen: String,
    expected: Option<Outcome>,
    note: Option<String>,
}

#[derive(Clone, Copy, Debug)]
enum Suite {
    Default,
    MoveOrder,
    Decisive,
    Quick,
    Thorough,
    All,
}

struct RunConfig {
    timeout: u64,
    epsilon: f64,
    tt_mb: usize,
    first_outcome: bool,
    nn_weights: Option<Arc<NnWeights>>,
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

#[derive(Serialize)]
struct JsonOutput {
    suite: String,
    mode: String,
    timeout: u64,
    runs: usize,
    epsilon: f64,
    tt_size: usize,
    config_path: Option<String>,
    nn_weights: Option<String>,
    results: Vec<JsonResult>,
    aggregates: JsonAggregate,
}

#[derive(Serialize)]
struct JsonResult {
    name: String,
    status: String,
    outcome: Outcome,
    expected: Option<Outcome>,
    nodes: u64,
    child_evals: u64,
    time_mean: f64,
    time_min: f64,
    time_max: f64,
    pv_len: usize,
    timeout: bool,
    wrong: bool,
}

#[derive(Serialize)]
struct JsonAggregate {
    total_nodes: u64,
    total_child_evals: u64,
    total_time: f64,
    solved: usize,
    timeouts: usize,
    wrong: usize,
    mean_pv_len: f64,
}

fn main() {
    let mut runs = 10usize;
    let mut timeout = 5u64;
    let mut epsilon = 0.125f64;
    let mut tt_size = 64usize;
    let mut suite = Suite::Default;
    let mut first_outcome = false;
    let mut json = false;
    let mut output_file: Option<String> = None;
    let mut filter: Option<String> = None;
    let mut config_path: Option<String> = None;
    let mut nn_weights: Option<String> = None;

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
            "--tt-size" => {
                tt_size = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .expect("--tt-size needs a positive integer");
                i += 2;
            }
            "--suite" => {
                let value = args.get(i + 1).expect("--suite needs an argument");
                suite = match value.as_str() {
                    "default" => Suite::Default,
                    "move-order" => Suite::MoveOrder,
                    "decisive" => Suite::Decisive,
                    "quick" => Suite::Quick,
                    "thorough" => Suite::Thorough,
                    "all" => Suite::All,
                    other => {
                        panic!(
                            "unknown suite '{other}'; try 'default', 'move-order', 'decisive', 'quick', 'thorough', or 'all'"
                        )
                    }
                };
                i += 2;
            }
            "--first-outcome" => {
                first_outcome = true;
                i += 1;
            }
            "--json" => {
                json = true;
                i += 1;
            }
            "--output-file" => {
                output_file = Some(
                    args.get(i + 1)
                        .expect("--output-file needs a file path")
                        .clone(),
                );
                i += 2;
            }
            "--config" => {
                config_path = Some(args.get(i + 1).expect("--config needs a file path").clone());
                i += 2;
            }
            "--nn-weights" => {
                nn_weights = Some(
                    args.get(i + 1)
                        .expect("--nn-weights needs a file path")
                        .clone(),
                );
                i += 2;
            }
            other => {
                filter = Some(other.to_string());
                i += 1;
            }
        }
    }

    assert!(tt_size > 0, "--tt-size must be positive");

    let config_path = config_path.or_else(|| std::env::var("SCORER_CONFIG").ok());

    let scorer = match config_path {
        Some(ref path) => {
            let params = config::load_scorer_config(path).expect("valid scorer config");
            StaticAtomicScorer::from_params(params)
        }
        None => StaticAtomicScorer::default(),
    };

    let nn_weights_loaded = nn_weights.as_ref().map(|path| {
        Arc::new(
            NnWeights::from_path(path)
                .unwrap_or_else(|e| panic!("failed to load NN weights from {path}: {e}")),
        )
    });

    let cases = load_suite(&suite);

    let mode = if first_outcome {
        "first-outcome"
    } else {
        "refined"
    };

    if !json {
        println!(
            "runs={runs} timeout={timeout}s epsilon={epsilon} tt_size={tt_size} suite={} mode={mode}",
            suite_name(&suite)
        );
        println!();
    }

    let mut results = Vec::new();
    let mut wrong = 0usize;
    for case in cases {
        if let Some(ref f) = filter
            && !case.name.contains(f)
        {
            continue;
        }
        if json {
            eprintln!("benchmarking {} ...", case.name);
        } else {
            println!("benchmarking {} ...", case.name);
        }
        let result = bench_case(
            &case,
            runs,
            &scorer,
            &RunConfig {
                timeout,
                epsilon,
                tt_mb: tt_size,
                first_outcome,
                nn_weights: nn_weights_loaded.clone(),
            },
        );
        if let Some(expected) = result.expected
            && result.outcome != Outcome::Draw
            && result.outcome != expected
        {
            wrong += 1;
        }
        results.push(result);
    }

    if json {
        let results_json: Vec<JsonResult> = results.iter().map(build_json_result).collect();
        let aggregates = build_aggregates(&results);
        let output = JsonOutput {
            suite: suite_name(&suite).to_string(),
            mode: mode.to_string(),
            timeout,
            runs,
            epsilon,
            tt_size,
            config_path,
            nn_weights,
            results: results_json,
            aggregates,
        };
        let json_str =
            serde_json::to_string_pretty(&output).expect("JSON serialization should not fail");
        println!("{json_str}");
        if let Some(path) = output_file {
            std::fs::write(&path, json_str).expect("failed to write JSON output file");
        }
    } else {
        print_table(&results, first_outcome);
    }

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
        Suite::Decisive => "decisive",
        Suite::Quick => "quick",
        Suite::Thorough => "thorough",
        Suite::All => "all",
    }
}

fn load_suite(suite: &Suite) -> Vec<Case> {
    match suite {
        Suite::Default => default_suite(),
        Suite::MoveOrder => move_order_suite(),
        Suite::Decisive => decisive_suite(),
        Suite::Quick => quick_suite(),
        Suite::Thorough => thorough_suite(),
        Suite::All => {
            let mut cases = default_suite();
            cases.extend(move_order_suite());
            cases.extend(decisive_suite());
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

fn decisive_suite() -> Vec<Case> {
    common::load_decisive_suite()
        .into_iter()
        .map(|case| Case {
            name: case.name,
            fen: case.fen,
            expected: case.expected,
            note: case.note,
        })
        .collect()
}

fn quick_suite() -> Vec<Case> {
    let mut cases = decisive_suite();
    cases.extend(
        move_order_suite()
            .into_iter()
            .filter(|c| move_order_number(&c.name).is_some_and(|n| n >= 23)),
    );
    cases
}

fn thorough_suite() -> Vec<Case> {
    let mut cases = move_order_suite();
    cases.extend(decisive_suite());
    cases
}

fn move_order_number(name: &str) -> Option<usize> {
    name.split('_')
        .next()
        .and_then(|prefix| prefix.strip_prefix('m'))
        .and_then(|s| s.parse().ok())
}

fn bench_case(
    case: &Case,
    runs: usize,
    scorer: &StaticAtomicScorer,
    config: &RunConfig,
) -> BenchResult {
    // Warm-up run, excluded from statistics (matching the report style).
    let _ = run_once(&case.fen, scorer, config);

    let mut times = Vec::with_capacity(runs);
    let mut first: Option<Run> = None;

    for _ in 0..runs {
        let run = run_once(&case.fen, scorer, config);
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

fn run_once(fen: &str, scorer: &StaticAtomicScorer, config: &RunConfig) -> Run {
    let mut pos = Position::from_fen(fen).expect("valid FEN");
    let mut search = Search::new(config.tt_mb);
    search.set_scorer(scorer.clone());
    if let Some(weights) = &config.nn_weights {
        search.set_nn_scorer(Some(NnMoveScorer::new(Arc::clone(weights))));
    }
    search.set_timeout(config.timeout);
    search.set_epsilon(config.epsilon);
    search.set_first_outcome_only(config.first_outcome);

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

fn json_status(result: &BenchResult) -> &'static str {
    match result.expected {
        None => "unknown",
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

fn build_json_result(result: &BenchResult) -> JsonResult {
    let status = json_status(result);
    JsonResult {
        name: result.name.clone(),
        status: status.to_string(),
        outcome: result.outcome,
        expected: result.expected,
        nodes: result.nodes,
        child_evals: result.child_evals,
        time_mean: result.mean.as_secs_f64(),
        time_min: result.min.as_secs_f64(),
        time_max: result.max.as_secs_f64(),
        pv_len: result.pv.len(),
        timeout: status == "timeout",
        wrong: status == "wrong",
    }
}

fn build_aggregates(results: &[BenchResult]) -> JsonAggregate {
    let mut total_nodes = 0u64;
    let mut total_child_evals = 0u64;
    let mut total_time = 0.0f64;
    let mut solved = 0usize;
    let mut timeouts = 0usize;
    let mut wrong = 0usize;
    let mut total_pv_len = 0usize;

    for r in results {
        total_nodes += r.nodes;
        total_child_evals += r.child_evals;
        total_time += r.mean.as_secs_f64();
        total_pv_len += r.pv.len();
        match json_status(r) {
            "ok" => solved += 1,
            "timeout" => timeouts += 1,
            "wrong" => wrong += 1,
            _ => {}
        }
    }

    let mean_pv_len = if results.is_empty() {
        0.0
    } else {
        total_pv_len as f64 / results.len() as f64
    };

    JsonAggregate {
        total_nodes,
        total_child_evals,
        total_time,
        solved,
        timeouts,
        wrong,
        mean_pv_len,
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
