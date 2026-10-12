//! Salt-seeded racer portfolio driver (plan9; `parallel` backlog item #11).
//!
//! Launches K unmodified solver processes ("racers") at different `--salt`
//! values on one FEN — every racer at the measured plan8 configuration
//! (`--first-outcome --outcome-only`, fixed and non-negotiable) — and takes
//! the first decisive answer. Implements the report8 stop-when-proven kill
//! policy: at the first decisive `outcome:` line observed from any racer,
//! that racer runs to natural completion and all remaining racers are
//! killed immediately (no grace period — reclaims the grace-burned
//! core-seconds found in report8's stress cost accounting).
//!
//! Soundness: each racer *is* the unmodified solver, so per-process
//! soundness is inherited; the only nondeterminism is *which* valid proof
//! wins (the accepted plan8 premise). The driver is timing-dependent by
//! design (take-first-decisive) and must never be used where reproducible
//! single-run semantics are required — benchmark / optimizer runs do not
//! use it. Censoring discipline (plan8): a racer printing `budget
//! exhausted` is recorded censored, never as a proven draw; wall-killed
//! racers' evals are excluded from the summary decision.
//!
//! RAM: the driver holds no search state; RAM = K × TT of the spawned
//! racers (4 × 128 MB = 512 MB at defaults).
//!
//! Usage:
//!     cargo run --release --example portfolio -- --fen "<FEN>" [options]
//!
//! This file exceeds the 10 KiB guideline because the race state machine
//! (`RaceTracker`) and its unit tests share private types; splitting the
//! tests out would force those definitions to be public (same pattern as
//! `src/search/ordering.rs`).

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_SALTS: &str = "0,2,3,5";
const DEFAULT_TIMEOUT_S: u64 = 300;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stream {
    Stdout,
    Stderr,
}

struct Args {
    fen: String,
    salts: Vec<u64>,
    timeout: u64,
    budget: Option<u64>,
    tt_size: Option<u64>,
    solver: Option<String>,
}

fn print_usage(program: &str) {
    println!("Usage: {program} --fen \"<FEN>\" [options]");
    println!();
    println!("Options:");
    println!("  --fen <FEN>       Position raced by all racers (required)");
    println!("  --salts <LIST>    Comma-separated racer salt set (default: {DEFAULT_SALTS})");
    println!("  --timeout <S>     Per-race wall cap in seconds (default: {DEFAULT_TIMEOUT_S})");
    println!("                    Also passed to each racer as its solver timeout");
    println!("  --budget <EVALS>  Pass-through of the solver's child-eval cap");
    println!("  --tt-size <MB>    Pass-through of the solver TT size (MB)");
    println!("  --solver <PATH>   Solver binary path override (default: auto-discovery");
    println!("                    next to this driver, then the cargo release dir)");
    println!();
    println!("Every racer is the unmodified solver invoked with --first-outcome");
    println!("--outcome-only plus its --salt (the measured plan8 configuration).");
    println!("Exit code: 0 on a decisive answer, 1 otherwise.");
}

fn parse_salts(spec: &str) -> Result<Vec<u64>, String> {
    let mut salts = Vec::new();
    for token in spec.split(',') {
        let token = token.trim();
        if token.is_empty() {
            return Err(format!("malformed salt list {spec:?}: empty entry"));
        }
        let salt: u64 = token
            .parse()
            .map_err(|_| format!("malformed salt list {spec:?}: {token:?} is not a u64"))?;
        if salts.contains(&salt) {
            return Err(format!(
                "malformed salt list {spec:?}: duplicate salt {salt}"
            ));
        }
        salts.push(salt);
    }
    if salts.is_empty() {
        return Err("empty salt list".into());
    }
    Ok(salts)
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let program = argv.first().map_or("portfolio", String::as_str);
    let mut fen = None;
    let mut salts = None;
    let mut timeout = None;
    let mut budget = None;
    let mut tt_size = None;
    let mut solver = None;
    let mut it = argv.iter().skip(1);
    while let Some(arg) = it.next() {
        let mut value_for = |name: &str| -> Result<String, String> {
            it.next()
                .ok_or_else(|| format!("missing value for {name}"))
                .cloned()
        };
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage(program);
                std::process::exit(0);
            }
            "--fen" => fen = Some(value_for("--fen")?),
            "--salts" => salts = Some(value_for("--salts")?),
            "--timeout" => {
                timeout = Some(
                    value_for("--timeout")?
                        .parse::<u64>()
                        .map_err(|_| "--timeout must be a number of seconds".to_string())?,
                )
            }
            "--budget" => {
                budget = Some(
                    value_for("--budget")?
                        .parse::<u64>()
                        .map_err(|_| "--budget must be a number of evals".to_string())?,
                )
            }
            "--tt-size" => {
                tt_size = Some(
                    value_for("--tt-size")?
                        .parse::<u64>()
                        .map_err(|_| "--tt-size must be a number of MB".to_string())?,
                )
            }
            "--solver" => solver = Some(value_for("--solver")?),
            other => return Err(format!("unknown option {other:?} (see --help)")),
        }
    }
    Ok(Args {
        fen: fen.ok_or("missing required --fen <FEN>")?,
        salts: parse_salts(salts.as_deref().unwrap_or(DEFAULT_SALTS))?,
        timeout: timeout.unwrap_or(DEFAULT_TIMEOUT_S),
        budget,
        tt_size,
        solver,
    })
}

/// Locate the solver binary: explicit `--solver` override, else next to this
/// driver's executable, else the cargo profile directory one level up (the
/// standard `target/<profile>/examples/portfolio` layout).
fn discover_solver(explicit: Option<&str>) -> Result<String, String> {
    if let Some(path) = explicit {
        return Ok(path.to_string());
    }
    let exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
    let mut candidates = Vec::new();
    if let Some(dir) = exe.parent() {
        candidates.push(dir.join("atomic_solver"));
        if let Some(parent) = dir.parent() {
            candidates.push(parent.join("atomic_solver"));
        }
    }
    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate.to_string_lossy().into_owned());
        }
    }
    Err(
        "atomic_solver binary not found next to the portfolio driver; \
         run `cargo build --release` or pass --solver <PATH>"
            .into(),
    )
}

/// One racer's observed facts (stream lines + lifecycle), testable without
/// any process.
#[derive(Default)]
struct RacerObs {
    decisive: Option<String>,
    final_outcome: Option<String>,
    budget_exhausted: bool,
    evals: Option<u64>,
    pv_lines: Vec<String>,
    exited: bool,
    killed: bool,
    spawn_failed: bool,
    wall_ms: u128,
}

/// Per-racer record as printed; `outcome` uses the driver grammar
/// (win|loss|draw|killed|error).
struct RacerRecord {
    salt: u64,
    wall_ms: u128,
    evals: Option<u64>,
    outcome: String,
    censored: bool,
}

/// Race state machine: consumes simulated or real racer events and yields
/// the kill decisions and final records. This is the skeleton that backlog
/// #6/#7 extend (shared-facts table, any-worker-can-finish).
#[derive(Default)]
struct RaceTracker {
    racers: BTreeMap<u64, RacerObs>,
    winner: Option<u64>,
}

impl RaceTracker {
    fn new(salts: &[u64]) -> Self {
        RaceTracker {
            racers: salts.iter().map(|&s| (s, RacerObs::default())).collect(),
            winner: None,
        }
    }

    /// Consume one output line; returns the salts to kill now (only ever on
    /// the first decisive outcome — stop-when-proven).
    fn on_line(&mut self, salt: u64, stream: Stream, line: &str) -> Vec<u64> {
        let line = line.trim_end();
        let Some(obs) = self.racers.get_mut(&salt) else {
            return Vec::new();
        };
        if let Some(rest) = line.strip_prefix("outcome: ") {
            let outcome = rest.split_whitespace().next().unwrap_or("");
            match outcome {
                "win" | "loss" => {
                    if obs.decisive.is_none() {
                        obs.decisive = Some(outcome.to_string());
                    }
                    if self.winner.is_none() {
                        self.winner = Some(salt);
                        return self.live_salts_except(salt);
                    }
                }
                "draw" if stream == Stream::Stdout => obs.final_outcome = Some("draw".to_string()),
                _ => {}
            }
        } else if line == "budget exhausted" {
            obs.budget_exhausted = true;
        } else if let Some(rest) = line.strip_prefix("evals: ") {
            if let Ok(n) = rest.trim().parse::<u64>() {
                obs.evals = Some(n);
            }
        } else if line.starts_with("pv: ") || line.starts_with("pv_status: ") {
            obs.pv_lines.push(line.to_string());
        }
        // Any other line is ignored (malformed input must never panic).
        Vec::new()
    }

    fn on_exit(&mut self, salt: u64, wall_ms: u128) {
        if let Some(obs) = self.racers.get_mut(&salt) {
            // A killed racer keeps its kill-time wall; the exit event only
            // confirms death a few milliseconds later.
            if !obs.killed {
                obs.wall_ms = wall_ms;
            }
            obs.exited = true;
        }
    }

    fn mark_killed(&mut self, salt: u64, wall_ms: u128) {
        if let Some(obs) = self.racers.get_mut(&salt) {
            obs.killed = true;
            obs.wall_ms = wall_ms;
        }
    }

    fn mark_spawn_failed(&mut self, salt: u64) {
        if let Some(obs) = self.racers.get_mut(&salt) {
            obs.spawn_failed = true;
            obs.exited = true;
        }
    }

    /// Salts of racers neither exited nor already killed (kill targets).
    fn live_salts_except(&self, salt: u64) -> Vec<u64> {
        self.racers
            .iter()
            .filter_map(|(s, o)| {
                if *s != salt && !o.exited && !o.killed && !o.spawn_failed {
                    Some(*s)
                } else {
                    None
                }
            })
            .collect()
    }

    fn records(&self) -> Vec<RacerRecord> {
        self.racers
            .iter()
            .map(|(&salt, o)| {
                let (outcome, censored) = if o.spawn_failed {
                    ("error".to_string(), false)
                } else if let Some(decisive) = &o.decisive {
                    (decisive.clone(), false)
                } else if o.killed && !o.budget_exhausted {
                    ("killed".to_string(), false)
                } else if o.budget_exhausted {
                    // plan8 censoring discipline: never a proven draw.
                    ("draw".to_string(), true)
                } else if let Some(final_outcome) = &o.final_outcome {
                    (final_outcome.clone(), false)
                } else {
                    // Exited naturally without a parseable outcome line.
                    ("error".to_string(), false)
                };
                // Wall-killed racers have their evals excluded (plan8
                // discipline): only a naturally completed racer's evals
                // count.
                let evals = if o.killed && o.decisive.is_none() {
                    None
                } else {
                    o.evals
                };
                RacerRecord {
                    salt,
                    wall_ms: o.wall_ms,
                    evals,
                    outcome,
                    censored,
                }
            })
            .collect()
    }

    fn winner_pv_lines(&self, salt: u64) -> &[String] {
        self.racers
            .get(&salt)
            .map(|o| o.pv_lines.as_slice())
            .unwrap_or(&[])
    }
}

fn print_records(records: &[RacerRecord]) {
    for r in records {
        let evals = r.evals.map_or("-".to_string(), |n| n.to_string());
        println!(
            "racer: salt={} wall={} evals={evals} outcome={} censored={}",
            r.salt,
            r.wall_ms,
            r.outcome,
            if r.censored { "budget" } else { "no" }
        );
    }
}

enum Event {
    Line {
        salt: u64,
        stream: Stream,
        line: String,
    },
    Exited {
        salt: u64,
    },
}

fn spawn_racer(
    solver: &str,
    args: &Args,
    salt: u64,
    tx: &Sender<Event>,
) -> std::io::Result<Arc<Mutex<Child>>> {
    let mut cmd = Command::new(solver);
    cmd.arg("--fen")
        .arg(&args.fen)
        .args(["--first-outcome", "--outcome-only"])
        .arg("--salt")
        .arg(salt.to_string())
        .arg("--timeout")
        .arg(args.timeout.to_string());
    if let Some(budget) = args.budget {
        cmd.arg("--budget").arg(budget.to_string());
    }
    if let Some(tt_size) = args.tt_size {
        cmd.arg("--tt-size").arg(tt_size.to_string());
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let child = Arc::new(Mutex::new(child));
    for (stream, file) in [
        (
            Stream::Stdout,
            Box::new(stdout) as Box<dyn std::io::Read + Send>,
        ),
        (
            Stream::Stderr,
            Box::new(stderr) as Box<dyn std::io::Read + Send>,
        ),
    ] {
        let tx = tx.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(file);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if tx
                            .send(Event::Line {
                                salt,
                                stream,
                                line: line.clone(),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
    }
    let watch_child = Arc::clone(&child);
    let watch_tx = tx.clone();
    thread::spawn(move || {
        loop {
            let done = {
                let Ok(mut guard) = watch_child.lock() else {
                    break;
                };
                matches!(guard.try_wait(), Ok(Some(_)) | Err(_))
            };
            if done {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        // The watcher loop broke (child exited, or its lock was poisoned):
        // report the exit so the event loop can settle.
        let _ = watch_tx.send(Event::Exited { salt });
    });
    Ok(child)
}

fn kill_racer(child: &Mutex<Child>) {
    if let Ok(mut guard) = child.lock() {
        let _ = guard.kill();
        let _ = guard.wait();
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let args = match parse_args(&argv) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let solver = match discover_solver(args.solver.as_deref()) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    let (tx, rx) = mpsc::channel::<Event>();
    let mut children: BTreeMap<u64, Arc<Mutex<Child>>> = BTreeMap::new();
    let mut tracker = RaceTracker::new(&args.salts);
    for &salt in &args.salts {
        match spawn_racer(&solver, &args, salt, &tx) {
            Ok(child) => {
                children.insert(salt, child);
            }
            Err(e) => {
                eprintln!("failed to spawn racer salt={salt}: {e}");
                tracker.mark_spawn_failed(salt);
            }
        }
    }
    // The race ends when the channel disconnects (all reader/watcher threads
    // done), not when the children settle: a child can exit while its pipes
    // still hold unflushed lines, and breaking at `all_settled()` drops those
    // trailing lines (misreporting e.g. a budget-censored racer as `error`).
    // Dropping the last sender makes `Disconnected` reachable.
    drop(tx);

    let start = Instant::now();
    let wall_cap = Duration::from_secs(args.timeout);
    loop {
        let now = Instant::now();
        if now.duration_since(start) >= wall_cap {
            for salt in tracker.live_salts_except(0) {
                if let Some(child) = children.get(&salt) {
                    kill_racer(child);
                    tracker.mark_killed(salt, now.duration_since(start).as_millis());
                }
            }
        }
        let wait = wall_cap
            .saturating_sub(now.duration_since(start))
            .min(Duration::from_millis(20))
            .max(Duration::from_millis(2));
        match rx.recv_timeout(wait) {
            Ok(Event::Line { salt, stream, line }) => {
                for target in tracker.on_line(salt, stream, &line) {
                    if let Some(child) = children.get(&target) {
                        kill_racer(child);
                        tracker.mark_killed(target, start.elapsed().as_millis());
                    }
                }
            }
            Ok(Event::Exited { salt }) => {
                tracker.on_exit(salt, start.elapsed().as_millis());
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    let records = tracker.records();
    print_records(&records);
    match tracker.winner {
        Some(salt) => {
            let rec = records
                .iter()
                .find(|r| r.salt == salt)
                .expect("winner salt has a record");
            let evals = rec.evals.map_or("-".to_string(), |n| n.to_string());
            println!(
                "portfolio: outcome={} salt={salt} evals={evals} wall={} racers={}",
                rec.outcome,
                rec.wall_ms,
                records.len(),
            );
            for line in tracker.winner_pv_lines(salt) {
                println!("{line}");
            }
        }
        None => {
            println!("portfolio: outcome=none racers={}", records.len());
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker(salts: &[u64]) -> RaceTracker {
        RaceTracker::new(salts)
    }

    #[test]
    fn salt_list_parsing_defaults_and_errors() {
        assert_eq!(parse_salts(DEFAULT_SALTS).unwrap(), vec![0, 2, 3, 5]);
        assert_eq!(parse_salts("7").unwrap(), vec![7]);
        assert_eq!(parse_salts(" 1 , 3 ").unwrap(), vec![1, 3]);
        assert!(parse_salts("").is_err());
        assert!(parse_salts("1,,2").is_err());
        assert!(parse_salts("a,2").is_err());
        assert!(parse_salts("2,2").is_err());
    }

    #[test]
    fn missing_fen_and_unknown_option_rejected() {
        let argv = vec!["portfolio".to_string()];
        assert!(parse_args(&argv).is_err());
        let argv = vec![
            "portfolio".to_string(),
            "--fenn".to_string(),
            "x".to_string(),
        ];
        assert!(parse_args(&argv).is_err());
        let argv = vec![
            "portfolio".to_string(),
            "--fen".to_string(),
            "8/8/8/8/8/8/8/4K2k w - - 0 1".to_string(),
        ];
        let args = parse_args(&argv).unwrap();
        assert_eq!(args.salts, vec![0, 2, 3, 5]);
        assert_eq!(args.timeout, DEFAULT_TIMEOUT_S);
        assert!(args.budget.is_none());
    }

    #[test]
    fn first_decisive_kills_remaining_racers() {
        let mut t = tracker(&[0, 2, 3, 5]);
        // A stderr progress `outcome:` line is the earliest decisive signal.
        let kill = t.on_line(2, Stream::Stderr, "outcome: win length: 15\n");
        assert_eq!(kill, vec![0, 3, 5]);
        assert_eq!(t.winner, Some(2));
        // A later stdout decisive line kills nobody.
        let kill = t.on_line(2, Stream::Stdout, "outcome: win length: 15\n");
        assert!(kill.is_empty());
        t.mark_killed(0, 100);
        t.on_exit(0, 101);
        t.on_exit(2, 900);
        let records = t.records();
        let r0 = records.iter().find(|r| r.salt == 0).unwrap();
        let r2 = records.iter().find(|r| r.salt == 2).unwrap();
        assert_eq!(r0.outcome, "killed");
        assert_eq!(r0.wall_ms, 100); // kill time, not exit-confirmation time
        assert!(!r0.censored);
        assert_eq!(r2.outcome, "win");
        assert_eq!(r2.evals, None);
        assert_eq!(r2.wall_ms, 900);
    }

    #[test]
    fn budget_censored_never_reported_as_proven_draw() {
        let mut t = tracker(&[0]);
        t.on_line(0, Stream::Stdout, "outcome: draw length: 5\n");
        t.on_line(0, Stream::Stdout, "budget exhausted\n");
        t.on_line(0, Stream::Stderr, "evals: 12345\n");
        t.on_exit(0, 50);
        let rec = &t.records()[0];
        assert_eq!(rec.outcome, "draw");
        assert!(rec.censored);
        assert_eq!(rec.evals, Some(12345));
        assert!(t.winner.is_none());
    }

    #[test]
    fn wall_killed_racer_excludes_evals_from_decision() {
        let mut t = tracker(&[0]);
        t.on_line(0, Stream::Stderr, "evals: 999\n");
        t.mark_killed(0, 400);
        t.on_exit(0, 401);
        let rec = &t.records()[0];
        assert_eq!(rec.outcome, "killed");
        // Killed mid-run: the solver never prints its final evals line, so
        // the summary decision sees no evals for this racer.
        assert_eq!(rec.evals, None);
    }

    #[test]
    fn natural_proven_draw_is_uncensored() {
        let mut t = tracker(&[3]);
        t.on_line(3, Stream::Stdout, "outcome: draw length: 8\n");
        t.on_exit(3, 20);
        let rec = &t.records()[0];
        assert_eq!(rec.outcome, "draw");
        assert!(!rec.censored);
    }

    #[test]
    fn malformed_exit_is_error_record_not_panic() {
        let mut t = tracker(&[1]);
        t.on_line(1, Stream::Stdout, "some garbage\n");
        t.on_line(1, Stream::Stderr, "evals: not-a-number\n");
        t.on_exit(1, 5);
        let rec = &t.records()[0];
        assert_eq!(rec.outcome, "error");
        assert_eq!(rec.evals, None);
    }

    #[test]
    fn later_decisive_racer_recorded_as_also_decisive() {
        let mut t = tracker(&[0, 5]);
        assert_eq!(
            t.on_line(5, Stream::Stderr, "outcome: loss length: 3\n"),
            vec![0]
        );
        // Racer 0 becomes decisive before the kill lands.
        let kill = t.on_line(0, Stream::Stderr, "outcome: win length: 3\n");
        assert!(kill.is_empty());
        assert_eq!(t.winner, Some(5));
        t.on_exit(0, 30);
        t.on_exit(5, 31);
        let records = t.records();
        assert_eq!(records.iter().find(|r| r.salt == 0).unwrap().outcome, "win");
        assert_eq!(
            records.iter().find(|r| r.salt == 5).unwrap().outcome,
            "loss"
        );
    }

    #[test]
    fn winner_pv_lines_are_kept_verbatim() {
        let mut t = tracker(&[0, 2]);
        assert_eq!(
            t.on_line(2, Stream::Stderr, "outcome: win length: 5\n"),
            vec![0]
        );
        t.on_line(2, Stream::Stdout, "pv: e2e4 d7d5\n");
        t.on_line(2, Stream::Stdout, "pv_status: first-outcome\n");
        t.on_line(2, Stream::Stdout, "pv: mangled\n");
        let pv = t.winner_pv_lines(2);
        assert_eq!(
            pv,
            &["pv: e2e4 d7d5", "pv_status: first-outcome", "pv: mangled"]
        );
    }
}
