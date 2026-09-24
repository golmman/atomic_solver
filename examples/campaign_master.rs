//! Campaign master (plan5 D2 prototype): proof state v0 + job dispatch.
//!
//! Implements the master side of `docs/plans/solve/campaign_architecture.md`
//! (§6 master proof state, §3 merge contract, §7 checkpointing, §8 message
//! schemas; the implemented v0 subset is §10 of that document):
//!
//! - AND/OR proof state over replayed position keys; the master itself
//!   performs rule-derived expansion of AND nodes (opponent replies), so no
//!   worker ever solves a whole root child (isolation-subsidy
//!   countermeasure, architecture doc §9).
//! - Pseudo-MPN job selection: descend the OR root to the min-pn child, then
//!   the min-dn open leaf (static rank as the prior). Locked-leaf
//!   reservation (PPN₂): one in-flight job per leaf; re-queues prefer the
//!   same worker (TT affinity).
//! - Merge contract (§3): worker-decisive results are verified by
//!   global-path replay (`campaign::verify`) before their facts are
//!   re-emitted into the global proof tree; a failing result aborts the
//!   session (rejected, never patched). The artifact is dumped only after
//!   `validate_proof_tree` passes (the "always again at finalization" leg).
//! - Checkpointability (§7): `master_state.json` dumps on a timer; jobs and
//!   results are durable files.
//!
//! Usage:
//!     campaign_master --session <dir> --fen <FEN> [--mode campaign|seq]
//!         [--workers 2] [--tt-mb 128] [--pt-mb 512] [--slice 2000000]
//!         [--max-slice 64000000] [--nf] [--abandon] [--max-wall 1800]
//!         [--out proof_tree.bin] [--tree-json tree.json] [--state-every 30]
//!
//! `--mode seq` is the in-process sequential baseline (first-outcome solve;
//! reports wall/nodes/child_evals for the work-inflation metric).

mod campaign;

use atomic_solver::position::Position;
use atomic_solver::search::dfpn::Search;
use campaign::state::{Args, Summary, run_campaign};
use std::collections::HashMap;
use std::time::Instant;

// ------------------------------------------------------------------ main ---

fn parse_args() -> Args {
    let mut a = Args {
        session: ".".into(),
        fen: Position::STARTPOS_FEN.to_string(),
        workers: 2,
        tt_mb: 128,
        pt_mb: 512,
        slice: 2_000_000,
        max_slice: 64_000_000,
        nf: false,
        abandon: false,
        max_wall: 1800,
        mode: "campaign".into(),
        out: "proof_tree.bin".into(),
        tree_json: String::new(),
        state_every: 30,
        seq_timeout: 3600,
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let need = |i: &mut usize, name: &str| -> String {
            let v = argv
                .get(*i + 1)
                .unwrap_or_else(|| panic!("{name} needs an argument"))
                .clone();
            *i += 2;
            v
        };
        match argv[i].as_str() {
            "--session" => a.session = need(&mut i, "--session"),
            "--fen" => a.fen = need(&mut i, "--fen"),
            "--workers" => a.workers = need(&mut i, "--workers").parse().expect("number"),
            "--tt-mb" => a.tt_mb = need(&mut i, "--tt-mb").parse().expect("number"),
            "--pt-mb" => a.pt_mb = need(&mut i, "--pt-mb").parse().expect("number"),
            "--slice" => a.slice = need(&mut i, "--slice").parse().expect("number"),
            "--max-slice" => a.max_slice = need(&mut i, "--max-slice").parse().expect("number"),
            "--nf" => {
                a.nf = true;
                i += 1;
            }
            "--abandon" => {
                a.abandon = true;
                i += 1;
            }
            "--max-wall" => a.max_wall = need(&mut i, "--max-wall").parse().expect("number"),
            "--mode" => a.mode = need(&mut i, "--mode"),
            "--out" => a.out = need(&mut i, "--out"),
            "--tree-json" => a.tree_json = need(&mut i, "--tree-json"),
            "--state-every" => a.state_every = need(&mut i, "--state-every").parse().expect("n"),
            "--seq-timeout" => a.seq_timeout = need(&mut i, "--seq-timeout").parse().expect("n"),
            other => panic!("unknown option '{other}'"),
        }
    }
    a
}

/// In-process sequential baseline (`--mode seq`): first-outcome solve, JSON
/// summary with the work counters the CLI does not expose.
fn run_seq(args: &Args) -> i32 {
    let mut pos = Position::from_fen(&args.fen).expect("valid FEN");
    let mut search = Search::new(args.tt_mb);
    search.set_first_outcome_only(true);
    search.set_timeout(args.seq_timeout);
    let t0 = Instant::now();
    let (outcome, _pv, _nodes) = search.solve(&mut pos);
    let wall = t0.elapsed().as_secs_f64();
    let summary = Summary {
        mode: "seq".into(),
        fen: args.fen.clone(),
        workers: 1,
        feedback: false,
        exit: "done".into(),
        outcome: Some(outcome.as_str().into()),
        wall_s: wall,
        jobs_dispatched: 0,
        jobs_abandoned: 0,
        jobs_completed: 0,
        job_errors: 0,
        verify_failures: 0,
        worker_nodes: search.nodes(),
        worker_child_evals: search.child_evaluations(),
        children_total: 0,
        children_resolved: 0,
        children_refuted: 0,
        leaves_open: 0,
        leaves_won: 0,
        leaves_lost: 0,
        tree_nodes: None,
        tree_root_depth: None,
        validate: None,
        per_worker_child_evals: HashMap::new(),
    };
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    0
}

fn main() {
    let args = parse_args();
    if args.mode == "seq" {
        std::process::exit(run_seq(&args));
    }
    std::process::exit(run_campaign(&args));
}
