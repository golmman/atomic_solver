//! The `proofdb_harvest` CLI arguments (plan2/3/4/6): `Args`, `usage`,
//! `parse_args`. Split from the example target root for the 10 KB
//! file-size convention; `main` remains there. `--pns-config` (plan6 D1)
//! loads the selection-mechanism knobs; it is validated here and effective
//! only under the `breadth-pns` policy.

use std::path::PathBuf;

use super::and_close::AndCloseOrder;
use super::policy::Policy;

pub struct Args {
    pub db: PathBuf,
    pub manifest: PathBuf,
    pub shard_dir: PathBuf,
    pub policy: Policy,
    pub ledger: PathBuf,
    pub budget_evals: u64,
    pub max_total_evals: u64,
    pub heavy_budget_evals: u64,
    pub heavy_sample: usize,
    pub tt_mb: usize,
    pub max_jobs: usize,
    pub max_runtime: u64,
    pub stop_file: PathBuf,
    pub pns_config: Option<PathBuf>,
    /// The `and-close` reply order (effective only under `and-close`).
    pub and_close_order: AndCloseOrder,
    pub out_db: Option<PathBuf>,
    pub dump: Option<PathBuf>,
}

fn usage() -> ! {
    eprintln!(
        "usage: proofdb_harvest --db <proofdb.db> --manifest <manifest.json> \
         --shard-dir <dir> [--policy <name>] [--ledger <path>] [--budget-evals <n>] \
         [--max-total-evals <n>] [--heavy-budget-evals <n>] [--heavy-sample <n>] \
         [--tt-mb <mb>] [--max-jobs <n>] [--max-runtime <s>] [--stop-file <path>] \
         [--pns-config <file>] [--and-close-order <completion|fresh>] \
         [--out-db <grown.db>] [--dump <nodes.txt>]  \
         (policies: breadth-pns | and-close | open-deepest | sharp-siblings | sharp-heavy-tail)"
    );
    std::process::exit(1);
}

pub fn parse_args() -> Args {
    let mut a = Args {
        db: PathBuf::new(),
        manifest: PathBuf::new(),
        shard_dir: PathBuf::new(),
        policy: Policy::BreadthPns,
        ledger: PathBuf::from("data/proofdb_work.json"),
        budget_evals: 0,
        max_total_evals: 0,
        heavy_budget_evals: 40_000_000,
        heavy_sample: 5,
        tt_mb: 128,
        max_jobs: 0,
        max_runtime: 0,
        stop_file: PathBuf::from("STOP"),
        pns_config: None,
        and_close_order: AndCloseOrder::Completion,
        out_db: None,
        dump: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut next = || it.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "--db" => a.db = PathBuf::from(next()),
            "--manifest" => a.manifest = PathBuf::from(next()),
            "--shard-dir" => a.shard_dir = PathBuf::from(next()),
            "--policy" => {
                a.policy = Policy::parse(&next()).unwrap_or_else(|e| {
                    eprintln!("proofdb_harvest: {e}");
                    usage()
                })
            }
            "--ledger" => a.ledger = PathBuf::from(next()),
            "--budget-evals" => a.budget_evals = next().parse().unwrap_or_else(|_| usage()),
            "--max-total-evals" => a.max_total_evals = next().parse().unwrap_or_else(|_| usage()),
            "--heavy-budget-evals" => {
                a.heavy_budget_evals = next().parse().unwrap_or_else(|_| usage())
            }
            "--heavy-sample" => a.heavy_sample = next().parse().unwrap_or_else(|_| usage()),
            "--tt-mb" => a.tt_mb = next().parse().unwrap_or_else(|_| usage()),
            "--max-jobs" => a.max_jobs = next().parse().unwrap_or_else(|_| usage()),
            "--max-runtime" => a.max_runtime = next().parse().unwrap_or_else(|_| usage()),
            "--stop-file" => a.stop_file = PathBuf::from(next()),
            "--pns-config" => a.pns_config = Some(PathBuf::from(next())),
            "--and-close-order" => {
                a.and_close_order = AndCloseOrder::parse(&next()).unwrap_or_else(|e| {
                    eprintln!("proofdb_harvest: {e}");
                    usage()
                })
            }
            "--out-db" => a.out_db = Some(PathBuf::from(next())),
            "--dump" => a.dump = Some(PathBuf::from(next())),
            _ => usage(),
        }
    }
    if a.db.as_os_str().is_empty()
        || a.manifest.as_os_str().is_empty()
        || a.shard_dir.as_os_str().is_empty()
    {
        usage();
    }
    a
}
