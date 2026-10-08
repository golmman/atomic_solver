//! The `proofdb_harvest` CLI arguments (plan2/3/4/6): `Args`, `usage`,
//! `parse_args`. Split from the example target root for the 10 KB
//! file-size convention; `main` remains there. `--pns-config` (plan6 D1)
//! loads the selection-mechanism knobs; it is validated here and effective
//! only under the `breadth-pns` policy.
//!
//! Generated state defaults wholly under `data/proofdb/` (plan14): DB,
//! manifest, shard dir, and ledger — a production harvest needs no path
//! flags. The committed fixture at `docs/plans/proofdb/shards/` is a
//! development/validation fixture, used only via explicit flags.

use std::path::PathBuf;

use super::and_close::AndCloseOrder;
use super::descend::{DEFAULT_PLIES, plies_in_range};
use super::policy::Policy;
use super::{DEFAULT_DB, DEFAULT_LEDGER, DEFAULT_MANIFEST, DEFAULT_SHARD_DIR};

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
    /// The `and-close` per-job budget cap (plan10 D1; 0 = unlimited).
    pub and_close_max_budget: u64,
    /// The `descend` enumeration depth (plan15 D1; effective only under
    /// `descend`, accepted range 1..=3).
    pub descend_plies: usize,
    pub out_db: Option<PathBuf>,
    pub dump: Option<PathBuf>,
}

fn usage() -> ! {
    eprintln!(
        "usage: proofdb_harvest [--db <proofdb.db>] [--manifest <manifest.json>] \
         [--shard-dir <dir>] [--policy <name>] [--ledger <path>] [--budget-evals <n>] \
         [--max-total-evals <n>] [--heavy-budget-evals <n>] [--heavy-sample <n>] \
         [--tt-mb <mb>] [--max-jobs <n>] [--max-runtime <s>] [--stop-file <path>] \
         [--pns-config <file>] [--and-close-order <completion|fresh>] \
         [--and-close-max-budget <evals>] [--descend-plies <n>] \
         [--out-db <grown.db>] [--dump <nodes.txt>]  \
         (defaults: --db data/proofdb/proofdb.db --manifest data/proofdb/shards/manifest.json \
         --shard-dir data/proofdb/shards --ledger data/proofdb/work.json; \
         policies: breadth-pns | and-close | descend | open-deepest | sharp-siblings | \
         sharp-heavy-tail)"
    );
    std::process::exit(1);
}

pub fn parse_args() -> Args {
    let mut a = Args {
        db: PathBuf::from(DEFAULT_DB),
        manifest: PathBuf::from(DEFAULT_MANIFEST),
        shard_dir: PathBuf::from(DEFAULT_SHARD_DIR),
        policy: Policy::BreadthPns,
        ledger: PathBuf::from(DEFAULT_LEDGER),
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
        and_close_max_budget: 0,
        descend_plies: DEFAULT_PLIES,
        out_db: None,
        dump: None,
    };
    let mut descend_plies_given: Option<usize> = None;
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
            "--and-close-max-budget" => {
                a.and_close_max_budget = next().parse().unwrap_or_else(|_| usage())
            }
            "--descend-plies" => {
                descend_plies_given = Some(next().parse().unwrap_or_else(|_| usage()))
            }
            "--out-db" => a.out_db = Some(PathBuf::from(next())),
            "--dump" => a.dump = Some(PathBuf::from(next())),
            _ => usage(),
        }
    }
    // `--descend-plies` (plan15 D1): effective only under `descend` (the
    // heavy-option convention) and only in the accepted range 1..=3.
    if let Some(n) = descend_plies_given {
        if a.policy != Policy::Descend {
            eprintln!("proofdb_harvest: --descend-plies is effective only under --policy descend");
            usage();
        }
        if !plies_in_range(n) {
            eprintln!("proofdb_harvest: --descend-plies must be in 1..=3 (got {n})");
            usage();
        }
        a.descend_plies = n;
    }
    a
}
