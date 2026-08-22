//! Gate-1 corpus generation for the learned move-ordering concept
//! (`docs/plans/nn/concept.md`, Gate 1; plan `docs/plans/nn/plan2.md`).
//!
//! Two subcommands:
//!
//! - `solve` — solves the concept's quick + decisive suites at fixed
//!   deterministic settings and writes one compact `proof_tree.bin` dump per
//!   case into `--dump-dir` (via `ProofTreeWorkerHandle::dump_to_bin`) plus a
//!   `manifest.json` recording per-case solve metadata.
//! - `load` — replays every `.bin` (root FEN + move paths) using
//!   `ProofTree::from_bin`, materializes one row per expanded, non-leaf tree
//!   node with `{hash, source, fen, stm, outcome, depth, subtree_size,
//!   legal_moves, static_scores, children, first_decisive_rank, partial}`,
//!   deduplicates rows by Zobrist hash, and serializes them as NDJSON for the
//!   external (Gate 2) trainer.  The move-order suite is held out for
//!   evaluation (concept.md Gate 4).
//!
//! Since design B (`docs/plans/nn/plan4.md`), every `children[]` entry carries
//! the recorded real `work` (`child_evals` spent proving that child's subtree)
//! from the v2 dump; the corpus version is `atomic-corpus/2` and the AND label
//! is "rank the children by `work`".
//!
//! This file is larger than 10 KiB because the solve driver, the load replay
//! (DFS with a single mutable `Position`, static scoring, subtree sizes), and
//! the NDJSON emitter are one pipeline; splitting them would fragment the row
//! schema that is pinned here for Gate 2.
//!
//! Usage:
//!     cargo run --release --example corpus_gen -- solve --fen "<fen>" \
//!         --timeout 2 --dump-dir /tmp/pt1
//!     cargo run --release --example corpus_gen -- solve --suite quick \
//!         --timeout 10 --dump-dir data/corpus/trees
//!     cargo run --release --example corpus_gen -- load --dump-dir data/corpus/trees \
//!         --output data/corpus/train.ndjson

mod common;

use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Color, Move, MoveList};
use atomic_solver::notation::{move_to_uci, moves_to_uci_path};
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_event::{NodeProven, ProofEvent};
use atomic_solver::proof_tree::{ProofTree, ProofTreeWorkerHandle};
use atomic_solver::search::dfpn::{ExitReason, Search};
use atomic_solver::search::ordering::{StaticAtomicScorer, nearest_commoner_map};
use serde::{Deserialize, Serialize};

const CORPUS_VERSION: &str = "atomic-corpus/2";

enum Command {
    Solve(SolveCli),
    Load(LoadCli),
}

struct SolveCli {
    fen: Option<String>,
    suite: String,
    timeout: u64,
    epsilon: f64,
    tt_size: usize,
    pt_size: usize,
    dump_dir: PathBuf,
}

struct LoadCli {
    dump_dir: PathBuf,
    output: Option<String>,
    include_leaves: bool,
}

fn print_help(program: &str) {
    println!(
        "Gate-1 corpus generation: solve reference suites and emit a deduplicated NDJSON corpus"
    );
    println!();
    println!("Usage:");
    println!("  {program} solve [OPTIONS]");
    println!("    --fen <FEN>       Solve a single position; case name \"fen\"");
    println!("    --suite <NAME>    quick | decisive          (default: quick)");
    println!("    --timeout <S>     Search budget in seconds   (default: 10)");
    println!("    --epsilon <F>     DF-PN+ threshold            (default: 0.125)");
    println!("    --tt-size <MB>    TT size                     (default: 64)");
    println!("    --pt-size <MB>    Proof-tree memory budget    (default: 256)");
    println!("    --dump-dir <DIR>  Output dir for .bin dumps + manifest");
    println!("                     (default: data/trees; created if missing)");
    println!("  {program} load [OPTIONS]");
    println!("    --dump-dir <DIR>  Dir with *.bin (default: data/trees; reads manifest");
    println!("                     if present; warns if not)");
    println!("    --output <FILE>   NDJSON output file (default: stdout)");
    println!("    --include-leaves  Emit depth-0 rows too");
    println!("  -h, --help           Print help and exit");
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let Some(sub) = args.first() else {
        return Err("missing subcommand; try 'solve' or 'load'".to_string());
    };
    match sub.as_str() {
        "solve" => parse_solve(&args[1..]).map(Command::Solve),
        "load" => parse_load(&args[1..]).map(Command::Load),
        "-h" | "--help" => Err("help".to_string()),
        other => Err(format!(
            "unknown subcommand '{other}'; try 'solve' or 'load'"
        )),
    }
}

fn parse_solve(args: &[String]) -> Result<SolveCli, String> {
    let mut cli = SolveCli {
        fen: None,
        suite: "quick".to_string(),
        timeout: 10,
        epsilon: 0.125,
        tt_size: 64,
        pt_size: 256,
        dump_dir: PathBuf::from("data/trees"),
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
                match value.as_str() {
                    "quick" | "decisive" => cli.suite = value.clone(),
                    other => {
                        return Err(format!(
                            "unknown suite '{other}'; try 'quick' or 'decisive'"
                        ));
                    }
                }
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
            "--dump-dir" => {
                cli.dump_dir =
                    PathBuf::from(args.get(i + 1).ok_or("--dump-dir needs a directory")?);
                i += 2;
            }
            other => return Err(format!("unknown option '{other}'")),
        }
    }
    Ok(cli)
}

fn parse_load(args: &[String]) -> Result<LoadCli, String> {
    let mut cli = LoadCli {
        dump_dir: PathBuf::from("data/trees"),
        output: None,
        include_leaves: false,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => return Err("help".to_string()),
            "--dump-dir" => {
                cli.dump_dir =
                    PathBuf::from(args.get(i + 1).ok_or("--dump-dir needs a directory")?);
                i += 2;
            }
            "--output" => {
                cli.output = Some(args.get(i + 1).ok_or("--output needs a file")?.clone());
                i += 2;
            }
            "--include-leaves" => {
                cli.include_leaves = true;
                i += 1;
            }
            other => return Err(format!("unknown option '{other}'")),
        }
    }
    Ok(cli)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or("corpus_gen");
    let cmd = match parse_args(&args[1..]) {
        Ok(cmd) => cmd,
        Err(e) if e == "help" => {
            print_help(program);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    match cmd {
        Command::Solve(cli) => run_solve(&cli),
        Command::Load(cli) => run_load(&cli),
    }
}

// ---------------------------------------------------------------------------
// solve
// ---------------------------------------------------------------------------

/// Extract the case number from a move-order case name such as `m23_white`.
fn move_order_number(name: &str) -> Option<usize> {
    name.split('_')
        .next()
        .and_then(|prefix| prefix.strip_prefix('m'))
        .and_then(|s| s.parse().ok())
}

fn load_cases(cli: &SolveCli) -> Vec<(String, String)> {
    if let Some(fen) = &cli.fen {
        return vec![("fen".to_string(), fen.clone())];
    }
    let cases = match cli.suite.as_str() {
        "decisive" => common::load_decisive_suite(),
        // The concept's "quick + decisive" equals the benchmark `quick` suite:
        // `decisive` plus the move-order cases with number >= 23.
        "quick" => {
            let mut cases = common::load_decisive_suite();
            cases.extend(
                common::load_move_order_suite()
                    .into_iter()
                    .filter(|c| move_order_number(&c.name).is_some_and(|n| n >= 23)),
            );
            cases
        }
        _ => unreachable!("suite validated by the CLI parser"),
    };
    cases.into_iter().map(|c| (c.name, c.fen)).collect()
}

fn run_solve(cli: &SolveCli) {
    if let Err(e) = std::fs::create_dir_all(&cli.dump_dir) {
        eprintln!("failed to create dump dir {}: {e}", cli.dump_dir.display());
        std::process::exit(1);
    }

    let cases = load_cases(cli);
    if cases.is_empty() {
        eprintln!("no cases to solve");
        std::process::exit(1);
    }

    let mut metas = Vec::with_capacity(cases.len());
    for (name, fen) in &cases {
        eprintln!("solving {name} ...");
        metas.push(solve_case(name, fen, cli));
    }

    let manifest = Manifest {
        suite: if cli.fen.is_some() {
            "fen".to_string()
        } else {
            cli.suite.clone()
        },
        timeout: cli.timeout,
        epsilon: cli.epsilon,
        tt_size: cli.tt_size,
        pt_size: cli.pt_size,
        cases: metas,
    };
    let path = cli.dump_dir.join("manifest.json");
    let file = std::fs::File::create(&path).unwrap_or_else(|e| {
        eprintln!("failed to create {path:?}: {e}");
        std::process::exit(1);
    });
    let mut writer = BufWriter::new(file);
    serde_json::to_writer(&mut writer, &manifest).unwrap_or_else(|e| {
        eprintln!("failed to write manifest: {e}");
        std::process::exit(1);
    });
    let _ = writeln!(writer);
    drop(writer);
    eprintln!("wrote {} ({} cases)", path.display(), manifest.cases.len());
}

fn solve_case(name: &str, fen: &str, cli: &SolveCli) -> CaseMeta {
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
            moves_to_uci_path(line)
        );
    });
    let timed_out = search.time_exceeded();
    let mem_limited = search.exit_reason() == ExitReason::MemoryLimit;

    let synthesized_root = outcome == Outcome::Draw;
    if synthesized_root {
        // A Draw root is never realized (no proof events arrive), so
        // `finalize()` would abort. Synthesize a Loss root, keeping the
        // realized Win children (refuted lines) with their proven OR subtrees —
        // the same workaround as `move_order_fractions`.
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
    let stats = handle.stats();

    let bin = if stats.nodes == 0 {
        eprintln!("  warning: {name}: empty proof tree, skipping dump");
        None
    } else {
        let bin_name = format!("{name}.bin");
        handle
            .dump_to_bin(cli.dump_dir.join(&bin_name))
            .unwrap_or_else(|e| {
                eprintln!("failed to write dump for {name}: {e}");
                std::process::exit(1);
            });
        Some(bin_name)
    };

    drop(search);
    drop(handle);
    let _ = join.join();

    eprintln!(
        "  {name}: outcome={} tree_nodes={} root_depth={} timed_out={} mem_limited={} synthesized={} bin={}",
        outcome.as_str(),
        stats.nodes,
        stats.root_depth,
        timed_out,
        mem_limited,
        synthesized_root,
        bin.as_deref().unwrap_or("-"),
    );

    CaseMeta {
        name: name.to_string(),
        fen: fen.to_string(),
        outcome: outcome.as_str().to_string(),
        timeout: timed_out,
        mem_limited,
        synthesized_root,
        tree_nodes: stats.nodes,
        root_depth: stats.root_depth,
        bin,
    }
}

/// Per-case metadata recorded in `manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaseMeta {
    name: String,
    fen: String,
    outcome: String,
    timeout: bool,
    mem_limited: bool,
    synthesized_root: bool,
    tree_nodes: usize,
    root_depth: u32,
    bin: Option<String>,
}

/// Top-level manifest written by `solve` and read by `load`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    suite: String,
    timeout: u64,
    epsilon: f64,
    tt_size: usize,
    pt_size: usize,
    cases: Vec<CaseMeta>,
}

// ---------------------------------------------------------------------------
// load
// ---------------------------------------------------------------------------

/// One corpus row (deduplicated by Zobrist hash).
struct Row {
    hash: u64,
    source: String,
    fen: String,
    stm: String,
    outcome: Outcome,
    depth: u32,
    subtree_size: u64,
    legal_moves: Vec<String>,
    static_scores: Vec<(String, i32)>,
    children: Vec<ChildRow>,
    first_decisive_rank: Option<u32>,
    partial: bool,
}

struct ChildRow {
    mv: String,
    outcome: Outcome,
    subtree_size: u64,
    work: u64,
}

fn run_load(cli: &LoadCli) {
    let manifest = read_manifest(&cli.dump_dir);
    let manifest_by_name: HashMap<String, &CaseMeta> = manifest
        .as_ref()
        .map(|m| m.cases.iter().map(|c| (c.name.clone(), c)).collect())
        .unwrap_or_default();

    let bins = find_bins(&cli.dump_dir);
    let cases_expected = manifest
        .as_ref()
        .map(|m| m.cases.len())
        .unwrap_or(bins.len());
    let mut filtered: Vec<PathBuf> = Vec::new();
    for bin in bins {
        let stem = bin_stem(&bin);
        if !manifest_by_name.is_empty() {
            let registered = manifest_by_name
                .get(&stem)
                .is_some_and(|meta| meta.bin.is_some());
            if !registered {
                eprintln!("warning: {stem}: no manifest entry, skipping");
                continue;
            }
        }
        filtered.push(bin);
    }
    let bins = filtered;

    let mut order: Vec<u64> = Vec::new();
    let mut rows: HashMap<u64, Row> = HashMap::new();
    let mut raw_rows = 0usize;
    let mut failed = 0usize;

    for bin in &bins {
        let source = bin_stem(bin);
        let meta = manifest_by_name.get(&source).cloned();
        let result = case_rows_from_bin(bin, &source, meta, cli.include_leaves);
        let n = result.as_ref().map_or(0, Vec::len);
        match result {
            Ok(case_rows) => {
                raw_rows += n;
                for row in case_rows {
                    merge_row(&mut rows, &mut order, row);
                }
                eprintln!("  {source}: {n} rows");
            }
            Err(e) => {
                failed += 1;
                eprintln!("warning: {source}: {e}; skipping");
            }
        }
    }

    let bins_loaded = bins.len() - failed;
    let rows_emitted = emit_ndjson(
        cli,
        &rows,
        &order,
        manifest.as_ref(),
        cases_expected,
        bins_loaded,
    );

    let or_rows = rows.values().filter(|r| r.outcome == Outcome::Win).count();
    let and_rows = rows.values().filter(|r| r.outcome == Outcome::Loss).count();
    let partial_rows = rows.values().filter(|r| r.partial).count();
    let dedup_dropped = raw_rows.saturating_sub(rows.len());
    eprintln!(
        "summary: cases={} bins={} failed={} raw_rows={} rows={} or_rows={} and_rows={} \
         dedup_dropped={} partial_rows={}",
        cases_expected,
        bins_loaded,
        failed,
        raw_rows,
        rows_emitted,
        or_rows,
        and_rows,
        dedup_dropped,
        partial_rows
    );
}

fn read_manifest(dump_dir: &Path) -> Option<Manifest> {
    let path = dump_dir.join("manifest.json");
    if !path.exists() {
        eprintln!(
            "warning: no manifest.json in {}; treating every bin as partial-free",
            dump_dir.display()
        );
        return None;
    }
    let file = std::fs::File::open(&path).unwrap_or_else(|e| {
        eprintln!("failed to read {path:?}: {e}");
        std::process::exit(1);
    });
    match serde_json::from_reader(BufReader::new(file)) {
        Ok(m) => Some(m),
        Err(e) => {
            eprintln!("failed to parse manifest {path:?}: {e}");
            std::process::exit(1);
        }
    }
}

fn find_bins(dump_dir: &Path) -> Vec<PathBuf> {
    let mut bins = Vec::new();
    let entries = match std::fs::read_dir(dump_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("failed to read dump dir {}: {e}", dump_dir.display());
            std::process::exit(1);
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "bin") {
            bins.push(path);
        }
    }
    bins.sort();
    bins
}

fn bin_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn case_rows_from_bin(
    path: &Path,
    source: &str,
    meta: Option<&CaseMeta>,
    include_leaves: bool,
) -> Result<Vec<Row>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(file);
    let tree = ProofTree::from_bin(&mut reader).map_err(|e| format!("bad proof tree: {e}"))?;
    let partial = meta.is_some_and(|m| m.timeout || m.synthesized_root);
    case_rows(&tree, source, partial, include_leaves)
}

/// Keep the occurrence with the largest `subtree_size`; ties keep the first
/// seen. Insertion order (case order, then node id order) is preserved.
fn merge_row(rows: &mut HashMap<u64, Row>, order: &mut Vec<u64>, row: Row) {
    match rows.get(&row.hash) {
        Some(existing) if existing.subtree_size >= row.subtree_size => {}
        Some(_) => {
            rows.insert(row.hash, row);
        }
        None => {
            order.push(row.hash);
            rows.insert(row.hash, row);
        }
    }
}

fn emit_ndjson(
    cli: &LoadCli,
    rows: &HashMap<u64, Row>,
    order: &[u64],
    manifest: Option<&Manifest>,
    cases: usize,
    bins: usize,
) -> usize {
    let suite = manifest.map(|m| m.suite.as_str()).unwrap_or("unknown");
    let timeout = manifest.map(|m| m.timeout).unwrap_or(0);
    let epsilon = manifest.map(|m| m.epsilon).unwrap_or(0.0);
    let tt_size = manifest.map(|m| m.tt_size).unwrap_or(0);
    let pt_size = manifest.map(|m| m.pt_size).unwrap_or(0);
    let partial_rows = rows.values().filter(|r| r.partial).count();

    let mut out: Box<dyn Write> = match &cli.output {
        Some(path) => {
            let file = std::fs::File::create(path).unwrap_or_else(|e| {
                eprintln!("failed to create {path}: {e}");
                std::process::exit(1);
            });
            Box::new(BufWriter::new(file))
        }
        None => Box::new(BufWriter::new(std::io::stdout())),
    };

    let meta = serde_json::json!({
        "_meta": CORPUS_VERSION,
        "suite": suite,
        "timeout": timeout,
        "epsilon": epsilon,
        "tt_size": tt_size,
        "pt_size": pt_size,
        "cases": cases,
        "bins": bins,
        "rows": rows.len(),
        "partial_rows": partial_rows,
    });
    writeln!(out, "{meta}").unwrap_or_else(|e| {
        eprintln!("failed to write NDJSON: {e}");
        std::process::exit(1);
    });
    for hash in order {
        let row = &rows[hash];
        let line = serde_json::to_string(&row.to_json()).unwrap_or_else(|e| {
            eprintln!("failed to serialize row {}: {e}", row.hash);
            std::process::exit(1);
        });
        writeln!(out, "{line}").unwrap_or_else(|e| {
            eprintln!("failed to write NDJSON: {e}");
            std::process::exit(1);
        });
    }
    let _ = out.flush();
    rows.len()
}

impl Row {
    fn to_json(&self) -> serde_json::Value {
        let mut static_scores = serde_json::Map::new();
        for (uci, score) in &self.static_scores {
            static_scores.insert(uci.clone(), serde_json::json!(*score));
        }
        let children: Vec<serde_json::Value> = self
            .children
            .iter()
            .map(|c| {
                serde_json::json!({
                    "mv": c.mv,
                    "outcome": c.outcome.as_str(),
                    "subtree_size": c.subtree_size,
                    "work": c.work,
                })
            })
            .collect();

        let mut obj = serde_json::Map::new();
        obj.insert("hash".to_string(), serde_json::json!(self.hash));
        obj.insert("source".to_string(), serde_json::json!(self.source));
        obj.insert("fen".to_string(), serde_json::json!(self.fen));
        obj.insert("stm".to_string(), serde_json::json!(self.stm));
        obj.insert(
            "outcome".to_string(),
            serde_json::json!(self.outcome.as_str()),
        );
        obj.insert("depth".to_string(), serde_json::json!(self.depth));
        obj.insert(
            "subtree_size".to_string(),
            serde_json::json!(self.subtree_size),
        );
        obj.insert(
            "legal_moves".to_string(),
            serde_json::json!(self.legal_moves),
        );
        obj.insert(
            "static_scores".to_string(),
            serde_json::Value::Object(static_scores),
        );
        obj.insert("children".to_string(), serde_json::Value::Array(children));
        if let Some(rank) = self.first_decisive_rank {
            obj.insert("first_decisive_rank".to_string(), serde_json::json!(rank));
        }
        obj.insert("partial".to_string(), serde_json::json!(self.partial));
        serde_json::Value::Object(obj)
    }
}

/// Post-order subtree sizes (node counts), identical to
/// `move_order_fractions::subtree_sizes`.
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

/// Materialize one row per expanded, non-leaf node by DFS-replaying the tree
/// with a single mutable `Position`.
fn case_rows(
    tree: &ProofTree,
    source: &str,
    partial: bool,
    include_leaves: bool,
) -> Result<Vec<Row>, String> {
    let sizes = subtree_sizes(tree);
    let mut pos = Position::from_fen(&tree.root_fen).map_err(|e| format!("bad root FEN: {e}"))?;
    let scorer = StaticAtomicScorer::default();
    let mut rows = Vec::with_capacity(tree.nodes.len());

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
                let outcome = node.outcome.filter(|_| node.depth > 0 || include_leaves);
                if let Some(outcome) = outcome {
                    let mut moves = MoveList::new();
                    pos.legal_moves(&mut moves);
                    let mut state = StateInfo::new();
                    pos.populate_state(&mut state);
                    let them = pos.side_to_move().flip();
                    let nearest = nearest_commoner_map(pos.board(), them);
                    let legal_moves: Vec<Move> = moves.as_slice().to_vec();
                    let is_or = outcome == Outcome::Win;

                    let stats: Vec<i32> = legal_moves
                        .iter()
                        .copied()
                        .map(|m| scorer.score_with_map(pos.board(), m, &state, &nearest, is_or))
                        .collect();
                    let mut scored: Vec<(Move, i32)> = legal_moves
                        .iter()
                        .copied()
                        .zip(stats.iter().copied())
                        .collect();
                    scored.sort_by_key(|&(_, s)| std::cmp::Reverse(s));

                    let legal_uci: Vec<String> =
                        legal_moves.iter().map(|&m| move_to_uci(m)).collect();
                    let static_scores: Vec<(String, i32)> = legal_uci
                        .iter()
                        .cloned()
                        .zip(stats.iter().copied())
                        .collect();

                    let children: Vec<ChildRow> = tree
                        .children(id)
                        .filter_map(|c| {
                            let outcome = tree.nodes[c].outcome?;
                            Some(ChildRow {
                                mv: move_to_uci(tree.nodes[c].mv),
                                outcome,
                                subtree_size: sizes[c],
                                work: tree.nodes[c].work,
                            })
                        })
                        .collect();

                    let first_decisive_rank = if is_or {
                        let mut min_rank: Option<usize> = None;
                        for c in tree.children(id) {
                            if tree.nodes[c].outcome != Some(Outcome::Loss) {
                                continue;
                            }
                            let cmv = tree.nodes[c].mv;
                            if let Some(rank) = scored.iter().position(|&(m, _)| m == cmv) {
                                min_rank =
                                    Some(min_rank.map_or(rank + 1, |r: usize| r.min(rank + 1)));
                            }
                        }
                        min_rank.map(|r| r as u32)
                    } else {
                        None
                    };

                    rows.push(Row {
                        hash: pos.hash(),
                        source: source.to_string(),
                        fen: pos.fen(),
                        stm: if pos.side_to_move() == Color::White {
                            "w".to_string()
                        } else {
                            "b".to_string()
                        },
                        outcome,
                        depth: node.depth,
                        subtree_size: sizes[id],
                        legal_moves: legal_uci,
                        static_scores,
                        children,
                        first_decisive_rank,
                        partial,
                    });
                }

                // Traversal must be independent of row emission: skipping an
                // `Enter` without pushing the matching `Exit` would leave the
                // pending `Descend` `do_move` un-undone and desync the
                // position.
                stack.push(Op::Exit(id));
                for &c in tree.children(id).collect::<Vec<_>>().iter().rev() {
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
    Ok(rows)
}
