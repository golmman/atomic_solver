//! Reconstruct a proof tree from the root FEN plus a TT snapshot.
//!
//! This file is larger than the 10 KiB guideline because the CLI argument
//! parsing, the single-snapshot reconstruction path, and the isomorphism
//! oracle reporting share one data flow; splitting a runnable example into
//! modules would hide that flow without removing it.
//!
//! Usage:
//!     cargo run --release --example `reconstruct_pt` -- --snapshot proof.bin.tt
//!     cargo run --release --example `reconstruct_pt` -- --snapshot s.tt \
//!         --oracle `proof_tree.bin` --out recon.bin

use atomic_solver::notation::{bits_to_move, moves_to_uci_path};
use atomic_solver::position::Outcome;
use atomic_solver::proof_tree::{ProofTree, validate_proof_tree};
use atomic_solver::reconstruct::{ReconstructConfig, reconstruct, tree_signature};
use atomic_solver::tt_snapshot::read_tt_snapshot;
use serde::Serialize;
use std::collections::HashSet;
use std::io::{BufReader, BufWriter};

struct Options {
    snapshot: Option<String>,
    fen: Option<String>,
    out: String,
    tt_size: Option<usize>,
    pt_size: usize,
    fill_base: u32,
    /// `--fill-depth-cap`: the depth ceiling for **root** holes only. A
    /// non-root hole is bounded by its parent's proven depth and is never
    /// capped below it.
    fill_depth_cap: u32,
    fill_attempt_budget: u64,
    fill_total_budget: u64,
    oracle: Option<String>,
    json: bool,
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
                );
            }
            "--pt-size" => {
                o.pt_size = need(&mut i, "--pt-size")
                    .parse()
                    .expect("--pt-size needs a number");
            }
            "--fill-base" => {
                o.fill_base = need(&mut i, "--fill-base")
                    .parse()
                    .expect("--fill-base needs a number");
            }
            "--fill-depth-cap" => {
                o.fill_depth_cap = need(&mut i, "--fill-depth-cap")
                    .parse()
                    .expect("--fill-depth-cap needs a number");
            }
            "--fill-attempt-budget" => {
                o.fill_attempt_budget = need(&mut i, "--fill-attempt-budget")
                    .parse()
                    .expect("--fill-attempt-budget needs a number");
            }
            "--fill-total-budget" => {
                o.fill_total_budget = need(&mut i, "--fill-total-budget")
                    .parse()
                    .expect("--fill-total-budget needs a number");
            }
            "--oracle" => o.oracle = Some(need(&mut i, "--oracle")),
            "--timeout" => {
                o.timeout = need(&mut i, "--timeout")
                    .parse()
                    .expect("--timeout needs a number");
            }
            "--json" => {
                o.json = true;
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

/// Run the replay-based validator on a finished tree and return the
/// `validate:` status string ("ok" or "FAILED n").
fn validate_status(tree: &ProofTree) -> String {
    match validate_proof_tree(tree) {
        Ok(()) => "ok".to_string(),
        Err(defects) => format!("FAILED {}", defects.len()),
    }
}

fn main() {
    let opts = parse_args();
    run_single(&opts);
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
        .unwrap_or_else(|| panic!("--snapshot is required"));
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

    // Replay-based structural validation of the reconstructed tree.
    let validate = output.tree.as_ref().map(validate_status);

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
            validate: Option<String>,
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
            validate: validate.clone(),
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
        if let Some(status) = &validate {
            println!("validate: {status}");
        }
        if let Some(error) = &output.error {
            eprintln!("error: {error}");
        }
    }
    if output.tree.is_none() {
        std::process::exit(1);
    }
    if validate.as_deref().is_some_and(|v| v != "ok") {
        // The per-defect `pt_validate: FAILED ...` lines went to stderr from
        // the worker's finalize pass inside `reconstruct`.
        eprintln!("reconstruction failed proof-tree validation");
        std::process::exit(1);
    }
}
