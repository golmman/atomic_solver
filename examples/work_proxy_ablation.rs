//! Ablation: does the corpus's `subtree_size` label proxy the solver's real
//! per-child work? (`docs/plans/nn/plan3.md`, design A; re-measured with the
//! recorded-work ground truth of `docs/plans/nn/plan4.md`, design B.)
//!
//! For each case, this example solves the position, walks the finalized proof
//! tree, and at every AND (Loss) node with at least two children compares the
//! stored child `subtree_size` (post-order node count, the old corpus AND
//! ranking label) against the child's recorded real work — `ProofNode.work`,
//! the cumulative `child_evals` spent proving that child's subtree, recorded
//! at prove time (design B).
//!
//! Since design B, every finalized node carries `work`, so every AND node is
//! complete by construction. The TT probe (`Search::tt_work_for`) remains as a
//! cross-check: `coverage` is the fraction of AND children whose hash hit the
//! TT, and `tt_agree` is the fraction of probed children whose TT `work`
//! exactly equals the recorded tree work.
//!
//! Metrics (reported per case and pooled):
//!
//! 1. **Pair flip rate** — over all child pairs of complete AND nodes, the
//!    fraction where `sign(subtree_size_i - subtree_size_j) !=
//!    sign(work_i - work_j)`. This is the label-noise rate the trainer would
//!    see.
//! 2. **Kendall τ** — `(C - D) / (C + D)` over pairs that are strictly
//!    ordered in both dimensions (ties in either dimension are ignored),
//!    pooled over all complete nodes.
//! 3. **Top-child agreement** — the fraction of complete AND nodes where the
//!    max-`subtree_size` child is also a max-`work` child (a child that
//!    attains both maxima).
//! 4. **Work-weighted flip share** — mis-ordered pairs weighted by
//!    `min(work_i, work_j)` divided by the total `min` weight over all
//!    complete-node pairs.
//!
//! Per-case summaries go to stderr; stdout carries only the final aggregate
//! table. The move-order suite is not training data, so it is fine to
//! measure on it here.
//!
//! Usage:
//!     cargo run --release --example work_proxy_ablation -- --fen "<fen>"
//!     cargo run --release --example work_proxy_ablation -- --suite quick
//!
//! Options:
//!   -h, --help            Print help and exit
//!   --fen <FEN>           Solve a single position; case name "fen"
//!   --suite <NAME>        quick | decisive | all (default: quick)
//!   --timeout <S>         Search budget in seconds (default: 10)
//!   --epsilon <F>         DF-PN+ threshold (default: 0.125)
//!   --tt-size <MB>        TT size (default: 64)
//!   --pt-size <MB>        Proof-tree memory budget (default: 256)

mod common;

use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_event::{NodeProven, ProofEvent};
use atomic_solver::proof_tree::{ProofTree, ProofTreeWorkerHandle};
use atomic_solver::search::dfpn::{ExitReason, Search};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Suite {
    Quick,
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
}

/// Pair metrics accumulated over the complete AND nodes of one case.
#[derive(Default, Clone, Copy)]
struct PairMetrics {
    nodes: u64,
    pairs: u64,
    flips: u64,
    concordant: u64,
    discordant: u64,
    weight_total: u128,
    weight_flip: u128,
    top_agree: u64,
    kendall_sum: f64,
}

struct CaseReport {
    name: String,
    outcome: Outcome,
    timed_out: bool,
    memory_limited: bool,
    and_nodes: usize,
    complete: usize,
    children_probed: u64,
    children_total: u64,
    tt_agree: u64,
    metrics: PairMetrics,
}

fn print_help(program: &str) {
    println!("ablate the subtree_size label against real TT work over the proof tree");
    println!();
    println!("Usage:");
    println!("  {program} [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -h, --help            Print help and exit");
    println!("  --fen <FEN>           Solve a single position; case name \"fen\"");
    println!("  --suite <NAME>        quick | decisive | all (default: quick)");
    println!("  --timeout <S>         Search budget in seconds (default: 10)");
    println!("  --epsilon <F>         DF-PN+ threshold (default: 0.125)");
    println!("  --tt-size <MB>        TT size (default: 64)");
    println!("  --pt-size <MB>        Proof-tree memory budget (default: 256)");
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    let mut cli = Cli {
        fen: None,
        suite: Suite::Quick,
        timeout: 10,
        epsilon: 0.125,
        tt_size: 64,
        pt_size: 256,
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
                    "quick" => Suite::Quick,
                    "decisive" => Suite::Decisive,
                    "all" => Suite::All,
                    other => {
                        return Err(format!(
                            "unknown suite '{other}'; try 'quick', 'decisive', or 'all'"
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
        .unwrap_or("work_proxy_ablation");
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
    if cases.is_empty() {
        eprintln!("no cases to solve");
        std::process::exit(1);
    }

    let mut reports = Vec::with_capacity(cases.len());
    for (name, fen) in &cases {
        eprintln!("solving {name} ...");
        reports.push(solve_and_measure(name, fen, &cli));
    }

    let mut aggregate = CaseReport {
        name: "aggregate".to_string(),
        outcome: Outcome::Draw,
        timed_out: false,
        memory_limited: false,
        and_nodes: 0,
        complete: 0,
        children_probed: 0,
        children_total: 0,
        tt_agree: 0,
        metrics: PairMetrics::default(),
    };
    for r in &reports {
        aggregate.and_nodes += r.and_nodes;
        aggregate.complete += r.complete;
        aggregate.children_probed += r.children_probed;
        aggregate.children_total += r.children_total;
        aggregate.tt_agree += r.tt_agree;
        aggregate.metrics.nodes += r.metrics.nodes;
        aggregate.metrics.pairs += r.metrics.pairs;
        aggregate.metrics.flips += r.metrics.flips;
        aggregate.metrics.concordant += r.metrics.concordant;
        aggregate.metrics.discordant += r.metrics.discordant;
        aggregate.metrics.weight_total += r.metrics.weight_total;
        aggregate.metrics.weight_flip += r.metrics.weight_flip;
        aggregate.metrics.top_agree += r.metrics.top_agree;
        aggregate.metrics.kendall_sum += r.metrics.kendall_sum;
    }

    println!(
        "case        and  comp   cov%    tag%     pairs  flip%  kendall  kmn%  top%  workflip%"
    );
    for r in &reports {
        println!("{}", table_row(r));
    }
    println!("{}", table_row(&aggregate));
}

/// Extract the case number from a move-order case name such as `m23_white`.
fn move_order_number(name: &str) -> Option<usize> {
    name.split('_')
        .next()
        .and_then(|prefix| prefix.strip_prefix('m'))
        .and_then(|s| s.parse().ok())
}

fn load_cases(cli: &Cli) -> Vec<(String, String)> {
    if let Some(fen) = &cli.fen {
        return vec![("fen".to_string(), fen.clone())];
    }
    let move_order = common::load_move_order_suite();
    let decisive = common::load_decisive_suite();
    let mut cases = Vec::new();
    match cli.suite {
        // Same as `corpus_gen --suite quick`: decisive + move-order cases m >= 23.
        Suite::Quick => {
            cases.extend(decisive.into_iter().map(|c| (c.name, c.fen)));
            cases.extend(
                move_order
                    .into_iter()
                    .filter(|c| move_order_number(&c.name).is_some_and(|n| n >= 23))
                    .map(|c| (c.name, c.fen)),
            );
        }
        Suite::Decisive => {
            cases.extend(decisive.into_iter().map(|c| (c.name, c.fen)));
        }
        Suite::All => {
            cases.extend(move_order.into_iter().map(|c| (c.name, c.fen)));
            cases.extend(decisive.into_iter().map(|c| (c.name, c.fen)));
        }
    }
    cases
}

fn solve_and_measure(name: &str, fen: &str, cli: &Cli) -> CaseReport {
    let mut pos = Position::from_fen(fen).unwrap_or_else(|e| {
        eprintln!("failed to parse FEN for {name}: {e}");
        std::process::exit(1);
    });

    let mut search = Search::new(cli.tt_size);
    search.set_timeout(cli.timeout);
    search.set_epsilon(cli.epsilon);

    let memory_limited = Arc::new(AtomicBool::new(false));
    let (handle, join) =
        ProofTreeWorkerHandle::spawn(fen.to_string(), cli.pt_size, Arc::clone(&memory_limited));
    search.set_proof_event_sender(Some(handle.event_sender()));

    let (outcome, _pv, _nodes) = search.solve_with_progress(&mut pos, |o, line| {
        eprintln!(
            "  outcome {} depth {} path {}",
            o.as_str(),
            line.len(),
            atomic_solver::notation::moves_to_uci_path(line)
        );
    });
    let timed_out = search.time_exceeded();
    let mem_limited = search.exit_reason() == ExitReason::MemoryLimit;

    if outcome == Outcome::Draw {
        // A Draw root is never realized (no proof events arrive), so
        // `finalize()` would abort. Synthesize a Loss root, keeping the
        // realized Win children (refuted lines) with their proven OR subtrees —
        // the same workaround as `corpus_gen` / `move_order_fractions`.
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

    // The TT must stay alive for probing; drop the search only after the walk.
    let report = analyze_tree(&search, &tree, name, outcome, timed_out, mem_limited);

    drop(search);
    drop(handle);
    let _ = join.join();

    eprintln!(
        "  {name}: outcome={} tree_nodes={} and_nodes={} complete={}",
        outcome.as_str(),
        tree.nodes.len(),
        report.and_nodes,
        report.complete
    );
    report
}

fn analyze_tree(
    search: &Search,
    tree: &ProofTree,
    name: &str,
    outcome: Outcome,
    timed_out: bool,
    memory_limited: bool,
) -> CaseReport {
    let sizes = subtree_sizes(tree);
    let mut pos = Position::from_fen(&tree.root_fen).unwrap_or_else(|e| {
        eprintln!("failed to parse root FEN for {name}: {e}");
        std::process::exit(1);
    });

    let mut metrics = PairMetrics::default();
    let mut and_nodes = 0usize;
    let mut complete = 0usize;
    let mut children_probed = 0u64;
    let mut children_total = 0u64;
    let mut tt_agree = 0u64;

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
                if node.outcome == Some(Outcome::Loss) && children.len() >= 2 {
                    and_nodes += 1;
                    let works: Vec<u64> = children
                        .iter()
                        .map(|&c| {
                            children_total += 1;
                            let recorded = tree.nodes[c].work;
                            // Cross-check against the design-A TT probe.
                            if let Some(tw) = search.tt_work_for(tree.nodes[c].hash) {
                                children_probed += 1;
                                if tw == recorded {
                                    tt_agree += 1;
                                }
                            }
                            recorded
                        })
                        .collect();
                    // Recorded work is always present post-finalize (design B),
                    // so every AND node with >= 2 children is complete.
                    complete += 1;
                    let child_sizes: Vec<u64> = children.iter().map(|&c| sizes[c]).collect();
                    accumulate(&mut metrics, &child_sizes, &works);
                }

                // Traversal must be independent of analysis: every `Enter`
                // pushes its `Exit` and children regardless of whether the
                // node contributed metrics, so the pending `Descend` `do_move`
                // is always undone.
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

    let report = CaseReport {
        name: name.to_string(),
        outcome,
        timed_out,
        memory_limited,
        and_nodes,
        complete,
        children_probed,
        children_total,
        tt_agree,
        metrics,
    };
    eprintln!("{}", case_summary(&report));
    report
}

/// Accumulate the pair metrics for one complete AND node.
fn accumulate(metrics: &mut PairMetrics, sizes: &[u64], works: &[u64]) {
    let n = sizes.len();
    let max_size = sizes.iter().copied().max().unwrap_or(0);
    let max_work = works.iter().copied().max().unwrap_or(0);
    if sizes
        .iter()
        .zip(works.iter())
        .any(|(&s, &w)| s == max_size && w == max_work)
    {
        metrics.top_agree += 1;
    }
    metrics.nodes += 1;

    let mut concordant = 0u64;
    let mut discordant = 0u64;
    for i in 0..n {
        for j in (i + 1)..n {
            metrics.pairs += 1;
            let min_weight = works[i].min(works[j]) as u128;
            metrics.weight_total += min_weight;
            let size_cmp = sizes[i].cmp(&sizes[j]);
            let work_cmp = works[i].cmp(&works[j]);
            if size_cmp == work_cmp {
                if size_cmp != Ordering::Equal {
                    concordant += 1;
                }
            } else {
                metrics.flips += 1;
                metrics.weight_flip += min_weight;
                if size_cmp != Ordering::Equal && work_cmp != Ordering::Equal {
                    discordant += 1;
                }
            }
        }
    }
    metrics.concordant += concordant;
    metrics.discordant += discordant;
    metrics.kendall_sum += if concordant + discordant > 0 {
        (concordant as f64 - discordant as f64) / (concordant + discordant) as f64
    } else {
        0.0
    };
}

fn pct(part: u128, total: u128) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * part as f64 / total as f64
    }
}

/// Two-line per-case summary in the plan's format (stderr).
fn case_summary(report: &CaseReport) -> String {
    let coverage = pct(
        report.children_probed as u128,
        report.children_total as u128,
    );
    let tt_agree = pct(report.tt_agree as u128, report.children_probed as u128);
    let m = &report.metrics;
    let mut line1 = format!(
        "=== {}  outcome={}  and_nodes={}  complete={}  coverage={coverage:.1}%  tt_agree={tt_agree:.1}%",
        report.name,
        report.outcome.as_str(),
        report.and_nodes,
        report.complete,
    );
    if report.timed_out {
        line1.push_str("  timeout=yes");
    }
    if report.memory_limited {
        line1.push_str("  memory_limited=yes");
    }
    let line2 = format!(
        "    pairs={}  pair_flip={:.1}%  kendall={:.2}  kendall_mean={:.2}  \
         top_agree={:.1}%  work_flip={:.1}%",
        m.pairs,
        pct(m.flips as u128, m.pairs as u128),
        kendall(m),
        kendall_mean(m),
        pct(m.top_agree as u128, m.nodes as u128),
        pct(m.weight_flip, m.weight_total),
    );
    format!("{line1}\n{line2}")
}

/// One row of the final stdout table.
fn table_row(report: &CaseReport) -> String {
    let coverage = pct(
        report.children_probed as u128,
        report.children_total as u128,
    );
    let tt_agree = pct(report.tt_agree as u128, report.children_probed as u128);
    let m = &report.metrics;
    format!(
        "{:<12} {:>4} {:>4}  {:>5.1}%  {:>5.1}%  {:>7}  {:>5.1}%  {:>7.2}  {:>5.1}%  {:>5.1}%  {:>8.1}%",
        report.name,
        report.and_nodes,
        report.complete,
        coverage,
        tt_agree,
        m.pairs,
        pct(m.flips as u128, m.pairs as u128),
        kendall(m),
        kendall_mean(m) * 100.0,
        pct(m.top_agree as u128, m.nodes as u128),
        pct(m.weight_flip, m.weight_total),
    )
}

fn kendall(m: &PairMetrics) -> f64 {
    if m.concordant + m.discordant > 0 {
        (m.concordant as f64 - m.discordant as f64) / (m.concordant + m.discordant) as f64
    } else {
        0.0
    }
}

/// Mean Kendall τ over complete nodes (unweighted by node size).
fn kendall_mean(m: &PairMetrics) -> f64 {
    if m.nodes > 0 {
        m.kendall_sum / m.nodes as f64
    } else {
        0.0
    }
}

/// Post-order subtree sizes (node counts), identical to
/// `corpus_gen::subtree_sizes`.
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
