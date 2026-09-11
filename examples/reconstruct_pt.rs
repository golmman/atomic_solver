//! Reconstruct a proof tree from the root FEN plus a TT snapshot.
//!
//! This file is larger than the 10 KiB guideline because it contains the CLI
//! argument parsing, the single-snapshot reconstruction path, the isomorphism
//! oracle reporting, and the go/no-go experiment harness (dual build + oracle
//! per suite case) in one example binary; splitting a runnable example into
//! modules would hide the experiment's data flow.
//!
//! Usage:
//!     cargo run --release --example reconstruct_pt -- --snapshot proof.bin.tt
//!     cargo run --release --example reconstruct_pt -- --snapshot s.tt \
//!         --oracle proof_tree.bin --out recon.bin
//!     cargo run --release --example reconstruct_pt -- --experiment --json

mod common;

use atomic_solver::notation::{bits_to_move, moves_to_uci_path};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofTree, ProofTreeWorkerHandle};
use atomic_solver::reconstruct::{ReconstructConfig, reconstruct, tree_signature};
use atomic_solver::search::dfpn::Search;
use atomic_solver::tt_snapshot::{read_tt_snapshot, write_tt_snapshot};
use serde::Serialize;
use std::collections::HashSet;
use std::io::{BufReader, BufWriter, Cursor};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct Options {
    snapshot: Option<String>,
    fen: Option<String>,
    out: String,
    tt_size: Option<usize>,
    pt_size: usize,
    fill_base: u32,
    fill_depth_cap: u32,
    fill_attempt_budget: u64,
    fill_total_budget: u64,
    oracle: Option<String>,
    json: bool,
    experiment: bool,
    timeout: u64,
}

fn parse_args() -> Options {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut o = Options {
        snapshot: None,
        fen: None,
        out: "reconstruct_pt.bin".to_string(),
        tt_size: None,
        pt_size: 256,
        fill_base: 8,
        fill_depth_cap: 32,
        fill_attempt_budget: 10_000_000,
        fill_total_budget: 0,
        oracle: None,
        json: false,
        experiment: false,
        timeout: 5,
    };
    let mut i = 0;
    while i < args.len() {
        let need = |i: &mut usize, name: &str| -> String {
            let v = args
                .get(*i + 1)
                .unwrap_or_else(|| panic!("{name} needs an argument"))
                .clone();
            *i += 2;
            v
        };
        match args[i].as_str() {
            "--snapshot" => o.snapshot = Some(need(&mut i, "--snapshot")),
            "--fen" => o.fen = Some(need(&mut i, "--fen")),
            "--out" => o.out = need(&mut i, "--out"),
            "--tt-size" => {
                o.tt_size = Some(
                    need(&mut i, "--tt-size")
                        .parse()
                        .expect("--tt-size needs a number"),
                )
            }
            "--pt-size" => {
                o.pt_size = need(&mut i, "--pt-size")
                    .parse()
                    .expect("--pt-size needs a number")
            }
            "--fill-base" => {
                o.fill_base = need(&mut i, "--fill-base")
                    .parse()
                    .expect("--fill-base needs a number")
            }
            "--fill-depth-cap" => {
                o.fill_depth_cap = need(&mut i, "--fill-depth-cap")
                    .parse()
                    .expect("--fill-depth-cap needs a number")
            }
            "--fill-attempt-budget" => {
                o.fill_attempt_budget = need(&mut i, "--fill-attempt-budget")
                    .parse()
                    .expect("--fill-attempt-budget needs a number")
            }
            "--fill-total-budget" => {
                o.fill_total_budget = need(&mut i, "--fill-total-budget")
                    .parse()
                    .expect("--fill-total-budget needs a number")
            }
            "--oracle" => o.oracle = Some(need(&mut i, "--oracle")),
            "--timeout" => {
                o.timeout = need(&mut i, "--timeout")
                    .parse()
                    .expect("--timeout needs a number")
            }
            "--json" => {
                o.json = true;
                i += 1;
            }
            "--experiment" => {
                o.experiment = true;
                i += 1;
            }
            other => panic!("unknown option '{other}' (see the module docs)"),
        }
    }
    o
}

fn load_oracle(path: &str) -> ProofTree {
    let file =
        std::fs::File::open(path).unwrap_or_else(|e| panic!("cannot open oracle {path}: {e}"));
    ProofTree::from_bin(&mut BufReader::new(file))
        .unwrap_or_else(|e| panic!("cannot parse oracle {path}: {e}"))
}

/// Compare two signatures and report the differing paths per side.
fn oracle_report(a: &ProofTree, b: &ProofTree) -> (bool, Vec<String>, Vec<String>) {
    let sig_a = tree_signature(a);
    let sig_b = tree_signature(b);
    if sig_a == sig_b {
        return (true, Vec::new(), Vec::new());
    }
    let keys_a: HashSet<_> = sig_a.keys().collect();
    let keys_b: HashSet<_> = sig_b.keys().collect();
    let mut only_a: Vec<String> = keys_a
        .difference(&keys_b)
        .map(|p| bits_path_string(p))
        .collect();
    let mut only_b: Vec<String> = keys_b
        .difference(&keys_a)
        .map(|p| bits_path_string(p))
        .collect();
    // Paths present in both but with different payload count as differing on
    // both sides.
    let shared_diff: Vec<String> = keys_a
        .intersection(&keys_b)
        .filter(|p| sig_a[p.as_slice()] != sig_b[p.as_slice()])
        .map(|p| bits_path_string(p))
        .collect();
    only_a.extend(shared_diff.iter().cloned());
    only_b.extend(shared_diff.iter().cloned());
    only_a.sort();
    only_b.sort();
    (false, only_a, only_b)
}

fn bits_path_string(path: &[u16]) -> String {
    let moves: Vec<_> = path.iter().filter_map(|&bits| bits_to_move(bits)).collect();
    moves_to_uci_path(&moves)
}

#[derive(Serialize, Clone, Copy, Default)]
struct HoleRow {
    hit: u64,
    terminal: u64,
    clock_hit: u64,
    clock_miss_draw: u64,
    repetition: u64,
    absent: u64,
    filled: u64,
    unfillable: u64,
    anomalies: u64,
}

fn hole_row(stats: &atomic_solver::reconstruct::ReconstructStats) -> HoleRow {
    HoleRow {
        hit: stats.hit,
        terminal: stats.terminal,
        clock_hit: stats.clock_hit,
        clock_miss_draw: stats.clock_miss_draw,
        repetition: stats.repetition,
        absent: stats.absent,
        filled: stats.filled,
        unfillable: stats.unfillable,
        anomalies: stats.anomalies,
    }
}

fn aggregate(mut acc: HoleRow, row: HoleRow) -> HoleRow {
    acc.hit += row.hit;
    acc.terminal += row.terminal;
    acc.clock_hit += row.clock_hit;
    acc.clock_miss_draw += row.clock_miss_draw;
    acc.repetition += row.repetition;
    acc.absent += row.absent;
    acc.filled += row.filled;
    acc.unfillable += row.unfillable;
    acc.anomalies += row.anomalies;
    acc
}

fn main() {
    let opts = parse_args();
    if opts.experiment {
        run_experiment(&opts);
    } else {
        run_single(&opts);
    }
}

fn config_from(opts: &Options, tt_mb: usize) -> ReconstructConfig {
    ReconstructConfig {
        tt_mb,
        pt_size_mb: opts.pt_size,
        fill_base: opts.fill_base,
        fill_depth_cap: opts.fill_depth_cap,
        fill_attempt_budget: opts.fill_attempt_budget,
        fill_total_budget: opts.fill_total_budget,
    }
}

fn run_single(opts: &Options) {
    let snapshot_path = opts
        .snapshot
        .as_ref()
        .unwrap_or_else(|| panic!("--snapshot is required (or use --experiment)"));
    let file = std::fs::File::open(snapshot_path)
        .unwrap_or_else(|e| panic!("cannot open snapshot {snapshot_path}: {e}"));
    let (header, solved, _unsolved) =
        read_tt_snapshot(&mut BufReader::new(file)).unwrap_or_else(|e| panic!("bad snapshot: {e}"));

    let fen = match &opts.fen {
        Some(fen) if fen.trim() != header.root_fen => {
            panic!(
                "--fen {fen:?} does not match the snapshot's root FEN {:?}",
                header.root_fen
            );
        }
        Some(_) | None => header.root_fen.clone(),
    };
    let tt_mb = opts.tt_size.unwrap_or(header.tt_size_mb as usize);

    let output = reconstruct(&fen, &solved, &config_from(opts, tt_mb));
    let stats = &output.stats;

    if let Some(tree) = &output.tree {
        let file = std::fs::File::create(&opts.out)
            .unwrap_or_else(|e| panic!("cannot create {}: {e}", opts.out));
        tree.to_bin(&mut BufWriter::new(file))
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", opts.out));
    }

    let oracle = opts.oracle.as_ref().map(|path| {
        let reference = load_oracle(path);
        match &output.tree {
            Some(tree) => {
                let (iso, a_only, b_only) = oracle_report(&reference, tree);
                if iso {
                    "oracle: isomorphic".to_string()
                } else {
                    format!(
                        "oracle: differing (a only-in-events: {} paths, b only-in-recon: {} paths)\n  a: {}\n  b: {}",
                        a_only.len(),
                        b_only.len(),
                        a_only.iter().take(3).cloned().collect::<Vec<_>>().join(", "),
                        b_only.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
                    )
                }
            }
            None => "oracle: reconstruction failed".to_string(),
        }
    });

    if opts.json {
        #[derive(Serialize)]
        struct SingleJson<'a> {
            status: &'a str,
            root_outcome: Option<Outcome>,
            nodes: u64,
            fill_evals: u64,
            duplicate_keys: u64,
            holes: HoleRow,
            error: Option<&'a str>,
            dump: Option<&'a str>,
            oracle: Option<String>,
        }
        let json = SingleJson {
            status: if output.tree.is_some() {
                "ok"
            } else {
                "failed"
            },
            root_outcome: output.root_outcome,
            nodes: stats.total(),
            fill_evals: output.fill_evals,
            duplicate_keys: stats.duplicate_keys,
            holes: hole_row(stats),
            error: output.error.as_deref(),
            dump: if output.tree.is_some() {
                Some(opts.out.as_str())
            } else {
                None
            },
            oracle,
        };
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    } else {
        match output.root_outcome {
            Some(outcome) => println!("outcome: {outcome}"),
            None => println!("outcome: none"),
        }
        println!("nodes: {}", stats.total());
        println!("{}", stats.holes_line());
        println!(
            "fill_evals: {} duplicate_keys: {}",
            output.fill_evals, stats.duplicate_keys
        );
        if output.tree.is_some() {
            println!("dump: {}", opts.out);
        }
        if let Some(line) = oracle {
            println!("{line}");
        }
        if let Some(error) = &output.error {
            eprintln!("error: {error}");
        }
    }
    if output.tree.is_none() {
        std::process::exit(1);
    }
}

// ------------------------------ experiment --------------------------------

#[derive(Serialize)]
struct CaseRow {
    name: String,
    status: String,
    outcome: Option<Outcome>,
    child_evals_c: u64,
    fill_evals_f: u64,
    fill_ratio: Option<f64>,
    recon_nodes: Option<u64>,
    holes: Option<HoleRow>,
    a_only_paths: Option<usize>,
    b_only_paths: Option<usize>,
    error: Option<String>,
}

#[derive(Serialize)]
struct Summary {
    cases: usize,
    live_ok: usize,
    skipped: usize,
    recon_ok: usize,
    oracle_isomorphic: usize,
    coverage_failures: usize,
    holes: HoleRow,
    total_child_evals_c: u64,
    total_fill_evals_f: u64,
    median_fill_ratio: Option<f64>,
    max_fill_ratio: Option<f64>,
    verdict: String,
}

struct LiveBuild {
    outcome: Outcome,
    child_evals: u64,
    snapshot: Vec<u8>,
    tree: Option<ProofTree>,
    memory_limited: bool,
}

fn live_build(fen: &str, tt_mb: usize, timeout: u64, pt_size: usize) -> LiveBuild {
    let memory_flag = Arc::new(AtomicBool::new(false));
    let (handle, join) =
        ProofTreeWorkerHandle::spawn(fen.to_string(), pt_size, Arc::clone(&memory_flag));
    let mut pos = Position::from_fen(fen).expect("valid suite FEN");
    let mut search = Search::new(tt_mb);
    search.set_timeout(timeout);
    search.set_memory_limited(Some(Arc::clone(&memory_flag)));
    search.set_proof_event_sender(Some(handle.event_sender()));
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    let child_evals = search.child_evaluations();
    let mut snapshot = Vec::new();
    write_tt_snapshot(
        search.tt(),
        fen,
        tt_mb.min(u32::MAX as usize) as u32,
        &mut snapshot,
    )
    .expect("snapshot write cannot fail into a Vec");
    let mut tree = None;
    if outcome != Outcome::Draw && !memory_flag.load(Ordering::Acquire) {
        handle.finalize();
        tree = Some(handle.tree());
    }
    drop(search);
    drop(handle);
    let _ = join.join();
    LiveBuild {
        outcome,
        child_evals,
        snapshot,
        tree,
        memory_limited: memory_flag.load(Ordering::Acquire),
    }
}

fn run_experiment(opts: &Options) {
    let tt_mb = opts.tt_size.unwrap_or(128);
    let config = config_from(opts, tt_mb);
    let cases = common::load_decisive_suite();
    if !opts.json {
        println!(
            "experiment: suite=decisive cases={} tt_size={tt_mb} pt_size={} timeout={}s fill_base={} fill_depth_cap={} fill_attempt_budget={}",
            cases.len(),
            opts.pt_size,
            opts.timeout,
            opts.fill_base,
            opts.fill_depth_cap,
            opts.fill_attempt_budget
        );
    }

    let mut rows = Vec::new();
    for case in &cases {
        if opts.json {
            eprintln!("experiment: {} ...", case.name);
        }
        let live = live_build(&case.fen, tt_mb, opts.timeout, opts.pt_size);
        let status = if live.tree.is_none() {
            if live.memory_limited {
                "skipped_memory"
            } else {
                "skipped_live_draw"
            }
        } else {
            "live_ok"
        };
        let mut row = CaseRow {
            name: case.name.clone(),
            status: status.to_string(),
            outcome: Some(live.outcome),
            child_evals_c: live.child_evals,
            fill_evals_f: 0,
            fill_ratio: None,
            recon_nodes: None,
            holes: None,
            a_only_paths: None,
            b_only_paths: None,
            error: None,
        };
        if status == "live_ok" {
            let (header, solved, _unsolved) =
                read_tt_snapshot(&mut Cursor::new(&live.snapshot)).expect("fresh snapshot parses");
            let output = reconstruct(&header.root_fen, &solved, &config);
            row.fill_evals_f = output.fill_evals;
            row.holes = Some(hole_row(&output.stats));
            row.recon_nodes = Some(output.stats.total());
            match (&output.tree, &live.tree) {
                (Some(t2), Some(t1)) => {
                    let (iso, a_only, b_only) = oracle_report(t1, t2);
                    row.a_only_paths = Some(a_only.len());
                    row.b_only_paths = Some(b_only.len());
                    if iso {
                        row.status = "ok".to_string();
                    } else {
                        row.status = "oracle_differing".to_string();
                        row.error = Some(format!(
                            "a only-in-events: {}; b only-in-recon: {}; first a: {}; first b: {}",
                            a_only.len(),
                            b_only.len(),
                            a_only.first().cloned().unwrap_or_default(),
                            b_only.first().cloned().unwrap_or_default()
                        ));
                    }
                }
                _ => {
                    row.status = "recon_failed".to_string();
                    row.error = output.error.clone();
                }
            }
            if row.status == "ok" && row.child_evals_c > 0 {
                row.fill_ratio = Some(row.fill_evals_f as f64 / row.child_evals_c as f64);
            }
        }
        rows.push(row);
    }

    let ratios: Vec<f64> = rows.iter().filter_map(|r| r.fill_ratio).collect();
    let mut sorted = ratios.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = if sorted.is_empty() {
        None
    } else {
        Some(sorted[sorted.len() / 2])
    };
    let max = ratios.iter().copied().fold(None, |acc: Option<f64>, r| {
        Some(acc.map_or(r, |m: f64| m.max(r)))
    });
    let holes = rows
        .iter()
        .filter_map(|r| r.holes)
        .fold(HoleRow::default(), aggregate);
    let coverage_failures = rows
        .iter()
        .filter(|r| matches!(r.status.as_str(), "recon_failed" | "oracle_differing"))
        .count();
    let oracle_isomorphic = rows.iter().filter(|r| r.status == "ok").count();
    let live_ok = rows
        .iter()
        .filter(|r| r.status != "skipped_live_draw" && r.status != "skipped_memory")
        .count();
    let skipped = rows.len() - live_ok;

    let verdict = if coverage_failures > 0 {
        "NO-GO (coverage): reconstruction failed where the live worker succeeded"
    } else if median.is_some_and(|m| m > 0.50) {
        "NO-GO (amplification): median fill ratio exceeds 50%"
    } else if median.is_some_and(|m| m <= 0.10) {
        "GO: zero coverage failures, all oracles isomorphic, median fill ratio <= 10%"
    } else {
        "WATCH: no coverage failures, median fill ratio between 10% and 50%"
    };
    let summary = Summary {
        cases: rows.len(),
        live_ok,
        skipped,
        recon_ok: oracle_isomorphic,
        oracle_isomorphic,
        coverage_failures,
        holes,
        total_child_evals_c: rows.iter().map(|r| r.child_evals_c).sum(),
        total_fill_evals_f: rows.iter().map(|r| r.fill_evals_f).sum(),
        median_fill_ratio: median,
        max_fill_ratio: max,
        verdict: verdict.to_string(),
    };

    if opts.json {
        #[derive(Serialize)]
        struct ExperimentJson {
            summary: Summary,
            cases: Vec<CaseRow>,
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&ExperimentJson {
                summary,
                cases: rows
            })
            .unwrap()
        );
    } else {
        println!("| name | status | outcome | C | F | F/C | recon_nodes | holes |");
        println!("|------|--------|---------|---:|---:|----:|----:|-------|");
        for r in &rows {
            println!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                r.name,
                r.status,
                r.outcome.map(|o| o.as_str()).unwrap_or("-"),
                r.child_evals_c,
                r.fill_evals_f,
                r.fill_ratio.map_or("-".to_string(), |v| format!("{v:.3}")),
                r.recon_nodes.map_or("-".to_string(), |v| v.to_string()),
                r.holes
                    .map(|h| format!(
                        "hit={} term={} clock_hit={} miss_draw={} rep={} absent={} filled={} unfill={} anom={}",
                        h.hit, h.terminal, h.clock_hit, h.clock_miss_draw, h.repetition,
                        h.absent, h.filled, h.unfillable, h.anomalies
                    ))
                    .unwrap_or_else(|| "-".to_string()),
            );
        }
        println!();
        println!(
            "live_ok={live_ok} skipped={skipped} recon_ok={recon_ok} coverage_failures={coverage_failures}",
            recon_ok = summary.recon_ok,
        );
        println!(
            "holes: hit={} terminal={} clock_hit={} clock_miss_draw={} repetition={} absent={} filled={} unfillable={} anomalies={}",
            holes.hit,
            holes.terminal,
            holes.clock_hit,
            holes.clock_miss_draw,
            holes.repetition,
            holes.absent,
            holes.filled,
            holes.unfillable,
            holes.anomalies
        );
        println!(
            "total C={} total F={} median F/C={} max F/C={}",
            summary.total_child_evals_c,
            summary.total_fill_evals_f,
            median.map_or("-".to_string(), |v| format!("{v:.4}")),
            max.map_or("-".to_string(), |v| format!("{v:.4}")),
        );
        println!("verdict: {verdict}");
    }
    if coverage_failures > 0 {
        eprintln!("experiment: {coverage_failures} coverage failure(s)");
        std::process::exit(1);
    }
}
