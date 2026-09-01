//! Measure move-ordering quality over the finalized proof tree.
//!
//! For every OR (Win) node in the finalized proof tree with a proven Loss
//! child, this example reports the rank of the proven decisive child under
//! the move ordering. By default that is the **static** `StaticAtomicScorer`
//! ordering; with `--nn-weights` it is the learned `NnMoveScorer` ordering
//! (the runtime-only history/killer/TT state is not recorded anywhere in
//! either case). Ranks are reported flat (fraction of OR nodes) and
//! work-weighted (weighted by the subtree size of each OR node, a proxy for
//! the solver's node-work at that position).
//!
//! This is the Gate 0 measurement for the learned move-ordering concept
//! (`docs/plans/nn/concept.md`); re-run with `--nn-weights` it is the Gate 4
//! ordering-quality comparison.
//!
//! Usage:
//!     cargo run --release --example move_order_fractions -- --suite move-order
//!     cargo run --release --example move_order_fractions -- --fen "<fen>"
//!     cargo run --release --example move_order_fractions -- --suite move-order --nn-weights data/corpus/weights.v1.bin
//!
//! Options:
//!   -h, --help            Print help and exit
//!   --fen <FEN>           Solve a single position; case name "fen"
//!   --suite <NAME>        move-order | decisive | all (default: move-order)
//!   --timeout <S>         Search budget in seconds (default: 5)
//!   --epsilon <F>         DF-PN+ threshold (default: 0.125)
//!   --tt-size <MB>        TT size (default: 64)
//!   --pt-size <MB>        Proof-tree memory budget (default: 256)
//!   --nn-weights <FILE>   Rank by the learned network instead of the static scorer

mod common;

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};
use atomic_solver::nn::{NnMoveScorer, NnWeights};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_event::{NodeProven, ProofEvent};
use atomic_solver::proof_tree::{ProofTree, ProofTreeWorkerHandle};
use atomic_solver::search::dfpn::{ExitReason, Search};
use atomic_solver::search::ordering::{StaticAtomicScorer, nearest_commoner_map};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Suite {
    MoveOrder,
    Decisive,
    All,
}

struct Cli {
    fen: Option<String>,
    suite: Suite,
    timeout: u64,
    epsilon: f64,
    tt_size: usize,
    pt_size: usize,
    nn_weights: Option<String>,
}

#[derive(Default)]
struct Buckets {
    nodes: [u64; 4],
    work: [u64; 4],
}

struct CaseReport {
    name: String,
    outcome: Outcome,
    tree_nodes: usize,
    or_nodes: usize,
    timeout: bool,
    memory_limited: bool,
    buckets: Buckets,
}

fn print_help(program: &str) {
    println!("measure static move-ordering quality over the finalized proof tree");
    println!();
    println!("Usage:");
    println!("  {program} [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -h, --help            Print help and exit");
    println!("  --fen <FEN>           Solve a single position; case name \"fen\"");
    println!("  --suite <NAME>        move-order | decisive | all (default: move-order)");
    println!("  --timeout <S>         Search budget in seconds (default: 5)");
    println!("  --epsilon <F>         DF-PN+ threshold (default: 0.125)");
    println!("  --tt-size <MB>        TT size (default: 64)");
    println!("  --pt-size <MB>        Proof-tree memory budget (default: 256)");
    println!("  --nn-weights <FILE>   Rank by the learned network instead of the static scorer");
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    let mut cli = Cli {
        fen: None,
        suite: Suite::MoveOrder,
        timeout: 5,
        epsilon: 0.125,
        tt_size: 64,
        pt_size: 256,
        nn_weights: None,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => return Err("help".to_string()),
            "--fen" => {
                cli.fen = Some(args.get(i + 1).ok_or("--fen needs a FEN")?.clone());
                i += 2;
            }
            "--suite" => {
                let value = args.get(i + 1).ok_or("--suite needs an argument")?;
                cli.suite = match value.as_str() {
                    "move-order" => Suite::MoveOrder,
                    "decisive" => Suite::Decisive,
                    "all" => Suite::All,
                    other => {
                        return Err(format!(
                            "unknown suite '{other}'; try 'move-order', 'decisive', or 'all'"
                        ));
                    }
                };
                i += 2;
            }
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
            "--pt-size" => {
                cli.pt_size = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .ok_or("--pt-size needs a positive integer")?;
                i += 2;
            }
            "--nn-weights" => {
                cli.nn_weights = Some(
                    args.get(i + 1)
                        .ok_or("--nn-weights needs a file path")?
                        .clone(),
                );
                i += 2;
            }
            other => return Err(format!("unknown option '{other}'")),
        }
    }
    Ok(cli)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args
        .first()
        .map(String::as_str)
        .unwrap_or("move_order_fractions");
    let cli = match parse_args(&args[1..]) {
        Ok(cli) => cli,
        Err(e) if e == "help" => {
            print_help(program);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let cases = load_cases(&cli);
    let nn_weights = cli.nn_weights.as_ref().map(|path| {
        Arc::new(NnWeights::from_path(path).unwrap_or_else(|e| {
            eprintln!("failed to load NN weights from {path}: {e}");
            std::process::exit(1);
        }))
    });
    if let Some(path) = &cli.nn_weights {
        eprintln!("ordering: nn-weights={path}");
    }
    let mut reports = Vec::new();
    for (name, fen) in &cases {
        eprintln!("solving {name} ...");
        reports.push(solve_and_measure(name, fen, &cli, nn_weights.clone()));
    }

    for report in &reports {
        print_case(report);
    }

    let mut aggregate = Buckets::default();
    let mut tree_nodes = 0usize;
    let mut or_nodes = 0usize;
    for r in &reports {
        for i in 0..4 {
            aggregate.nodes[i] += r.buckets.nodes[i];
            aggregate.work[i] += r.buckets.work[i];
        }
        tree_nodes += r.tree_nodes;
        or_nodes += r.or_nodes;
    }
    println!(
        "=== aggregate  cases={}  tree_nodes={}  or_nodes={}",
        reports.len(),
        tree_nodes,
        or_nodes
    );
    print_bucket_rows(&aggregate);
}

fn load_cases(cli: &Cli) -> Vec<(String, String)> {
    if let Some(fen) = &cli.fen {
        return vec![("fen".to_string(), fen.clone())];
    }
    let mut cases = Vec::new();
    match cli.suite {
        Suite::MoveOrder => {
            cases.extend(
                common::load_move_order_suite()
                    .into_iter()
                    .map(|c| (c.name, c.fen)),
            );
        }
        Suite::Decisive => {
            cases.extend(
                common::load_decisive_suite()
                    .into_iter()
                    .map(|c| (c.name, c.fen)),
            );
        }
        Suite::All => {
            cases.extend(
                common::load_move_order_suite()
                    .into_iter()
                    .map(|c| (c.name, c.fen)),
            );
            cases.extend(
                common::load_decisive_suite()
                    .into_iter()
                    .map(|c| (c.name, c.fen)),
            );
        }
    }
    cases
}

fn solve_and_measure(
    name: &str,
    fen: &str,
    cli: &Cli,
    nn_weights: Option<Arc<NnWeights>>,
) -> CaseReport {
    let mut pos = Position::from_fen(fen).unwrap_or_else(|e| {
        eprintln!("failed to parse FEN for {name}: {e}");
        std::process::exit(1);
    });

    let mut search = Search::new(cli.tt_size);
    search.set_timeout(cli.timeout);
    search.set_epsilon(cli.epsilon);
    if let Some(weights) = &nn_weights {
        search.set_nn_scorer(Some(NnMoveScorer::new(Arc::clone(weights))));
    }

    let memory_limited = Arc::new(AtomicBool::new(false));
    let (handle, join) =
        ProofTreeWorkerHandle::spawn(fen.to_string(), cli.pt_size, Arc::clone(&memory_limited));
    search.set_proof_event_sender(Some(handle.event_sender()));

    let (outcome, _pv, _nodes) = search.solve_with_progress(&mut pos, |o, line| {
        eprintln!("  outcome: {} length: {}", o.as_str(), line.len());
    });
    let timed_out = search.time_exceeded();
    let mem_limited = search.exit_reason() == ExitReason::MemoryLimit;

    if outcome == Outcome::Draw {
        // A Draw root is never realized: draw outcomes are not emitted as proof
        // events, and a timeout leaves the root unproven, so `finalize()` would
        // abort. Synthesize a Loss root so finalize keeps the realized Win
        // children (refuted lines) with their proven OR subtrees.
        let _ = handle
            .event_sender()
            .send(ProofEvent::NodeProven(NodeProven::new(
                Vec::new(),
                pos.hash(),
                Outcome::Loss,
                0,
                0,
            )));
    }

    handle.finalize();
    let tree = handle.tree();
    drop(search);
    drop(handle);
    let _ = join.join();

    let report = analyze_tree(name, &tree, outcome, timed_out, mem_limited, nn_weights);
    eprintln!(
        "  {name}: outcome={} tree_nodes={} or_nodes={}",
        report.outcome.as_str(),
        report.tree_nodes,
        report.or_nodes
    );
    report
}

fn analyze_tree(
    name: &str,
    tree: &ProofTree,
    outcome: Outcome,
    timed_out: bool,
    memory_limited: bool,
    nn_weights: Option<Arc<NnWeights>>,
) -> CaseReport {
    let sizes = subtree_sizes(tree);
    let samples = rank_samples(tree, &sizes, nn_weights);

    let mut buckets = Buckets::default();
    for (rank, work) in &samples {
        let idx = (*rank - 1).min(3);
        buckets.nodes[idx] += 1;
        buckets.work[idx] += *work;
    }

    CaseReport {
        name: name.to_string(),
        outcome,
        tree_nodes: tree.nodes.len(),
        or_nodes: samples.len(),
        timeout: timed_out,
        memory_limited,
        buckets,
    }
}

fn subtree_sizes(tree: &ProofTree) -> Vec<u64> {
    let mut sizes = vec![0u64; tree.nodes.len()];
    let mut stack: Vec<(usize, bool)> = vec![(0, false)];
    while let Some((id, done)) = stack.pop() {
        if done {
            let mut total = 1u64;
            for c in tree.children(id) {
                total += sizes[c];
            }
            sizes[id] = total;
        } else {
            stack.push((id, true));
            for c in tree.children(id) {
                stack.push((c, false));
            }
        }
    }
    sizes
}

fn rank_samples(
    tree: &ProofTree,
    sizes: &[u64],
    nn_weights: Option<Arc<NnWeights>>,
) -> Vec<(usize, u64)> {
    let mut pos = Position::from_fen(&tree.root_fen).unwrap();
    let scorer = StaticAtomicScorer::default();
    let nn = nn_weights.map(NnMoveScorer::new);
    let mut samples = Vec::new();

    enum Op {
        Enter(usize),
        Descend(usize),
        Exit(usize),
    }
    let mut stack = vec![Op::Enter(0)];
    while let Some(op) = stack.pop() {
        match op {
            Op::Enter(id) => {
                let node = &tree.nodes[id];
                let children: Vec<usize> = tree.children(id).collect();
                if node.outcome == Some(Outcome::Win) {
                    let decisive: Vec<usize> = children
                        .iter()
                        .copied()
                        .filter(|&c| tree.nodes[c].outcome == Some(Outcome::Loss))
                        .collect();
                    if !decisive.is_empty() {
                        let mut moves = MoveList::new();
                        pos.legal_moves(&mut moves);
                        let mut state = StateInfo::new();
                        pos.populate_state(&mut state);
                        let slice = moves.as_slice();
                        let them = pos.side_to_move().flip();
                        let nearest = nearest_commoner_map(pos.board(), them);
                        let mut scored: Vec<(Move, i32)> = slice
                            .iter()
                            .copied()
                            .map(|m| {
                                (
                                    m,
                                    scorer.score_with_map(pos.board(), m, &state, &nearest, true),
                                )
                            })
                            .collect();
                        if let Some(nn) = &nn {
                            // Residual composition (nn.md §6 v2 recipe): the
                            // network adds to the static term, matching
                            // `sort_moves`; history/killer are runtime state
                            // and are not part of the measured ordering.
                            let scores = nn.move_scores(pos.board(), slice);
                            for ((_, score), nn_score) in scored.iter_mut().zip(scores) {
                                *score += nn_score;
                            }
                        }
                        scored.sort_by_key(|&(_, s)| std::cmp::Reverse(s));
                        let mut min_rank = usize::MAX;
                        for &c in &decisive {
                            let cmv = tree.nodes[c].mv;
                            if let Some(i) = scored.iter().position(|&(m, _)| m == cmv) {
                                min_rank = min_rank.min(i + 1);
                            }
                        }
                        if min_rank != usize::MAX {
                            samples.push((min_rank, sizes[id]));
                        }
                    }
                }
                stack.push(Op::Exit(id));
                for &c in children.iter().rev() {
                    stack.push(Op::Descend(c));
                }
            }
            Op::Descend(c) => {
                pos.do_move(tree.nodes[c].mv);
                stack.push(Op::Enter(c));
            }
            Op::Exit(id) => {
                if id != 0 {
                    pos.undo_move(tree.nodes[id].mv);
                }
            }
        }
    }
    samples
}

fn print_case(report: &CaseReport) {
    let mut line = format!(
        "=== {}  outcome={}  tree_nodes={}  or_nodes={}",
        report.name,
        report.outcome.as_str(),
        report.tree_nodes,
        report.or_nodes
    );
    if report.timeout {
        line.push_str("  timeout=yes");
    }
    if report.memory_limited {
        line.push_str("  memory_limited=yes");
    }
    println!("{line}");
    if report.or_nodes == 0 {
        return;
    }
    print_bucket_rows(&report.buckets);
}

fn print_bucket_rows(buckets: &Buckets) {
    let total_nodes: u64 = buckets.nodes.iter().sum();
    let total_work: u64 = buckets.work.iter().sum();
    println!("rank    nodes  pct      work    work_pct");
    for (i, label) in ["1", "2", "3", ">3"].iter().enumerate() {
        let pct = pct_of(buckets.nodes[i], total_nodes);
        let work_pct = pct_of(buckets.work[i], total_work);
        println!(
            "{:<8} {:>5} {:>7.1}%  {:>7}  {:>7.1}%",
            label, buckets.nodes[i], pct, buckets.work[i], work_pct
        );
    }
}

fn pct_of(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * part as f64 / total as f64
    }
}
