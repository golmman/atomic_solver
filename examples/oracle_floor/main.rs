//! Oracle-floor measurement for the move-ordering headroom question
//! (`docs/plans/nn/plan8.md`, Step 3).
//!
//! Two modes over the baseline proof-tree dumps generated in Step 2:
//!
//! - `decompose` (static analysis, no search): split the recorded `work` of
//!   each oracle tree at OR and AND nodes to bound what perfect ordering
//!   could have saved. See `decompose.rs`.
//! - `solve` (measured): solve each case twice with identical settings
//!   (`--timeout 60 --epsilon 0.125 --tt-size 64`, first-outcome) — once
//!   with the baseline ordering and once with the oracle ordering injected
//!   through `Search::set_ordering_scorer`. The **oracle eval ratio**
//!   (oracle evals / baseline evals on cases decisive under both) is the
//!   floor number the pinned 0.5x decision bar consumes.
//!
//! Usage:
//!     cargo run --release --example oracle_floor -- decompose
//!     cargo run --release --example oracle_floor -- solve

#[path = "../common.rs"]
mod common;

mod decompose;
mod oracle;

use std::time::Instant;

use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::ProofTree;
use atomic_solver::search::dfpn::Search;
use atomic_solver::search::ordering::MoveScorer;

use crate::oracle::OracleScorer;

struct Cli {
    mode: String,
    timeout: u64,
    epsilon: f64,
    tt_size: usize,
    trees_dir: String,
    case_filter: Option<String>,
    fen: Option<String>,
}

fn print_help(program: &str) {
    println!("oracle-floor measurement (nn plan8, Step 3)");
    println!();
    println!("Usage:");
    println!("  {program} <decompose|solve> [OPTIONS]");
    println!();
    println!("Modes:");
    println!("  decompose    Static work decomposition of the oracle trees (no search)");
    println!("  solve        Baseline vs oracle-ordered searches, eval ratio + coverage");
    println!();
    println!("Options:");
    println!("  -h, --help            Print help and exit");
    println!("  --timeout <S>         Search budget in seconds (default: 60)");
    println!("  --epsilon <F>         DF-PN+ threshold (default: 0.125)");
    println!("  --tt-size <MB>        TT size (default: 64)");
    println!("  --trees-dir <DIR>     Oracle tree dumps (default: data/oracle/trees)");
    println!("  --case <NAME>         Only run this case name");
    println!("  --fen <FEN>           With --case <NAME>: use this FEN instead of the fixture");
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    let mut cli = Cli {
        mode: String::new(),
        timeout: 60,
        epsilon: 0.125,
        tt_size: 64,
        trees_dir: "data/oracle/trees".to_string(),
        case_filter: None,
        fen: None,
    };
    let mut positional = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => return Err("help".to_string()),
            "--timeout" => {
                cli.timeout = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .ok_or("--timeout needs a positive integer")?;
                i += 2;
            }
            "--epsilon" => {
                cli.epsilon = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .ok_or("--epsilon needs a number in [0,1]")?;
                i += 2;
            }
            "--tt-size" => {
                cli.tt_size = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .ok_or("--tt-size needs a positive integer")?;
                i += 2;
            }
            "--trees-dir" => {
                cli.trees_dir = args
                    .get(i + 1)
                    .ok_or("--trees-dir needs a directory")?
                    .clone();
                i += 2;
            }
            "--case" => {
                cli.case_filter = Some(args.get(i + 1).ok_or("--case needs a name")?.clone());
                i += 2;
            }
            "--fen" => {
                cli.fen = Some(args.get(i + 1).ok_or("--fen needs a FEN")?.clone());
                i += 2;
            }
            other if other.starts_with('-') => return Err(format!("unknown option '{other}'")),
            other => {
                positional.push(other.to_string());
                i += 1;
            }
        }
    }
    if positional.len() != 1 || !matches!(positional[0].as_str(), "decompose" | "solve") {
        return Err("exactly one mode argument 'decompose' or 'solve' is required".to_string());
    }
    cli.mode = positional.remove(0);
    Ok(cli)
}

/// The measured cases: move-order suite entries that have an oracle tree in
/// `--trees-dir` (the Step-2 generation only kept converging cases).
fn load_cases(cli: &Cli) -> Vec<(String, String)> {
    if let (Some(name), Some(fen)) = (&cli.case_filter, &cli.fen) {
        return vec![(name.clone(), fen.clone())];
    }
    let suite = common::load_move_order_suite();
    let mut cases = Vec::new();
    let entries = std::fs::read_dir(&cli.trees_dir)
        .unwrap_or_else(|e| {
            eprintln!(
                "cannot read trees dir {} ({e}); generate dumps first (plan8 Step 2)",
                cli.trees_dir
            );
            std::process::exit(1);
        })
        .collect::<Vec<_>>();
    let mut names: Vec<String> = entries
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    for name in names {
        if let Some(filter) = &cli.case_filter
            && *filter != name
        {
            continue;
        }
        if let Some(c) = suite.iter().find(|c| c.name == name) {
            cases.push((name, c.fen.clone()));
        } else {
            eprintln!("skipping {name}: not a move-order suite case");
        }
    }
    if cases.is_empty() {
        eprintln!(
            "no cases found in {} (generate dumps first, plan8 Step 2)",
            cli.trees_dir
        );
        std::process::exit(1);
    }
    cases
}

fn load_tree(cli: &Cli, name: &str) -> ProofTree {
    let path = format!("{}/{name}.bin", cli.trees_dir);
    let mut file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open oracle tree {path}: {e}"));
    let tree = ProofTree::from_bin(&mut file)
        .unwrap_or_else(|e| panic!("cannot parse oracle tree {path}: {e}"));
    assert!(
        tree.nodes.len() > 1,
        "{name}: oracle tree has no proven children"
    );
    tree
}

struct RunResult {
    outcome: Outcome,
    child_evals: u64,
    wall: f64,
    timed_out: bool,
}

fn run_search(cli: &Cli, fen: &str, oracle: Option<std::sync::Arc<OracleScorer>>) -> RunResult {
    let mut pos = Position::from_fen(fen).expect("valid case FEN");
    let mut search = Search::new(cli.tt_size);
    search.set_timeout(cli.timeout);
    search.set_epsilon(cli.epsilon);
    search.set_first_outcome_only(true);
    if let Some(scorer) = oracle {
        search.set_ordering_scorer(Some(scorer as std::sync::Arc<dyn MoveScorer>));
    }
    let start = Instant::now();
    let (outcome, _pv, _) = search.solve(&mut pos);
    let wall = start.elapsed().as_secs_f64();
    RunResult {
        outcome,
        child_evals: search.child_evaluations(),
        wall,
        timed_out: search.time_exceeded(),
    }
}

fn solve_mode(cli: &Cli) {
    #[derive(Default)]
    struct Agg {
        cases: usize,
        baseline_evals: u64,
        oracle_evals: u64,
        ratio_sum: f64,
        oracle_wall_sum: f64,
        baseline_wall_sum: f64,
        coverage_hit: u64,
        coverage_seen: u64,
    }
    let mut agg = Agg::default();
    for (name, fen) in load_cases(cli) {
        eprintln!("solving {name} ...");
        let baseline = run_search(cli, &fen, None);
        // Single-threaded measurement with the pinned `Arc<dyn MoveScorer>`
        // hook; the scorer's RefCell caches never cross threads.
        #[allow(clippy::arc_with_non_send_sync)]
        let scorer = std::sync::Arc::new(OracleScorer::new(load_tree(cli, &name)));
        let oracle = run_search(cli, &fen, Some(std::sync::Arc::clone(&scorer)));
        let (hit, seen, board_hit) = scorer.coverage();
        let decisive_both = baseline.outcome != Outcome::Draw && oracle.outcome != Outcome::Draw;
        let eval_ratio = if decisive_both && baseline.child_evals > 0 {
            oracle.child_evals as f64 / baseline.child_evals as f64
        } else {
            f64::NAN
        };
        println!(
            "=== {name}  baseline={}/{} evals={} wall={:.2}s{}",
            baseline.outcome.as_str(),
            oracle.outcome.as_str(),
            baseline.child_evals,
            baseline.wall,
            if baseline.timed_out { " TIMEOUT" } else { "" }
        );
        println!(
            "  oracle        evals={} wall={:.2}s{}  coverage={:.1}% ({hit}/{seen})  board_hash={:.1}%",
            oracle.child_evals,
            oracle.wall,
            if oracle.timed_out { " TIMEOUT" } else { "" },
            if seen > 0 {
                100.0 * hit as f64 / seen as f64
            } else {
                0.0
            },
            if seen > 0 {
                100.0 * board_hit as f64 / seen as f64
            } else {
                0.0
            },
        );
        if decisive_both {
            println!(
                "  ratio         evals={eval_ratio:.3}x wall={:.3}x",
                oracle.wall / baseline.wall.max(1e-9)
            );
            agg.cases += 1;
            agg.baseline_evals += baseline.child_evals;
            agg.oracle_evals += oracle.child_evals;
            agg.ratio_sum += eval_ratio;
            agg.baseline_wall_sum += baseline.wall;
            agg.oracle_wall_sum += oracle.wall;
            agg.coverage_hit += hit;
            agg.coverage_seen += seen;
        } else {
            println!("  ratio         n/a (not decisive under both; excluded from aggregate)");
        }
    }
    if agg.cases > 0 {
        let work_ratio = agg.oracle_evals as f64 / agg.baseline_evals as f64;
        println!(
            "=== aggregate  cases={}  evals: baseline={} oracle={} ratio={work_ratio:.3}x",
            agg.cases, agg.baseline_evals, agg.oracle_evals
        );
        println!(
            "  unweighted mean per-case ratio={:.3}x  wall: baseline={:.1}s oracle={:.1}s ({:.2}x)",
            agg.ratio_sum / agg.cases as f64,
            agg.baseline_wall_sum,
            agg.oracle_wall_sum,
            agg.oracle_wall_sum / agg.baseline_wall_sum
        );
        println!(
            "  oracle-node coverage={:.1}% ({}/{})",
            100.0 * agg.coverage_hit as f64 / agg.coverage_seen as f64,
            agg.coverage_hit,
            agg.coverage_seen
        );
    }
}

fn decompose_mode(cli: &Cli) {
    let mut agg_or_node_work = 0u64;
    let mut agg_or_decisive_clamped = 0u64;
    let mut agg_and_shares: Vec<f64> = Vec::new();
    for (name, _fen) in load_cases(cli) {
        let tree = load_tree(cli, &name);
        let d = decompose::decompose(&name, &tree);
        agg_or_node_work += d.or_node_work;
        agg_or_decisive_clamped += d.or_decisive_work_clamped;
        agg_and_shares.extend(d.and_child_shares.iter().copied());
        decompose::print_case(&d);
    }
    let mut shares = agg_and_shares;
    shares.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("=== aggregate  cases={}", load_cases(cli).len());
    println!(
        "  OR work: decisive-child share={:.1}%  recoverable (refutation+own)={:.1}%",
        100.0 * agg_or_decisive_clamped as f64 / agg_or_node_work as f64,
        100.0 * (agg_or_node_work - agg_or_decisive_clamped) as f64 / agg_or_node_work as f64,
    );
    if !shares.is_empty() {
        println!(
            "  AND child work share: min={:.1}%  median={:.1}%  max={:.1}%",
            100.0 * decompose::percentile(&shares, 0.0),
            100.0 * decompose::percentile(&shares, 0.5),
            100.0 * decompose::percentile(&shares, 1.0),
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or("oracle_floor");
    let cli = match parse_args(&args[1..]) {
        Ok(cli) => cli,
        Err(e) if e == "help" => {
            print_help(program);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            eprintln!("run with --help for usage");
            std::process::exit(1);
        }
    };
    match cli.mode.as_str() {
        "decompose" => decompose_mode(&cli),
        "solve" => solve_mode(&cli),
        _ => unreachable!(),
    }
}
