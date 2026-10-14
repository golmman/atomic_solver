//! The `proofdb_harvest` CLI arguments (plan2/3/4/6; slimmed by the plan18
//! policy consolidation): `Args`, `usage`, `parse_args`. Split from the
//! example target root for the 10 KB file-size convention; `main` remains
//! there.
//!
//! Generated state defaults wholly under `data/proofdb/` (plan14): DB,
//! manifest, shard dir, and ledger — a production harvest needs no path
//! flags. The committed fixture at `docs/plans/proofdb/shards/` is a
//! development/validation fixture, used only via explicit flags.

use std::path::PathBuf;

use super::{DEFAULT_DB, DEFAULT_LEDGER, DEFAULT_MANIFEST, DEFAULT_SHARD_DIR};

/// The only harvest policy (plan18 policy consolidation): the plan16
/// shallowest-first ledger walk. The `--policy` flag is kept so existing
/// command lines keep working; it accepts only this name.
pub const POLICY: &str = "frontier-walk";

pub struct Args {
    pub db: PathBuf,
    pub manifest: PathBuf,
    pub shard_dir: PathBuf,
    pub ledger: PathBuf,
    pub budget_evals: u64,
    pub max_total_evals: u64,
    pub tt_mb: usize,
    pub max_jobs: usize,
    pub max_runtime: u64,
    pub stop_file: PathBuf,
    pub out_db: Option<PathBuf>,
    pub dump: Option<PathBuf>,
}

fn usage() -> ! {
    eprintln!(
        "usage: proofdb_harvest [--db <proofdb.db>] [--manifest <manifest.json>] \
         [--shard-dir <dir>] [--policy frontier-walk] [--ledger <path>] \
         [--budget-evals <n>] [--max-total-evals <n>] [--tt-mb <mb>] \
         [--max-jobs <n>] [--max-runtime <s>] [--stop-file <path>] \
         [--out-db <grown.db>] [--dump <nodes.txt>]  \
         (defaults: --db data/proofdb/proofdb.db --manifest data/proofdb/shards/manifest.json \
         --shard-dir data/proofdb/shards --ledger data/proofdb/work.json; \
         policy: frontier-walk — the only harvest policy; --budget-evals 0 \
         selects the standing base 4000000)"
    );
    std::process::exit(1);
}

pub fn parse_args() -> Args {
    let mut a = Args {
        db: PathBuf::from(DEFAULT_DB),
        manifest: PathBuf::from(DEFAULT_MANIFEST),
        shard_dir: PathBuf::from(DEFAULT_SHARD_DIR),
        ledger: PathBuf::from(DEFAULT_LEDGER),
        budget_evals: 0,
        max_total_evals: 0,
        tt_mb: 128,
        max_jobs: 0,
        max_runtime: 0,
        stop_file: PathBuf::from("STOP"),
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
                if next() != POLICY {
                    eprintln!(
                        "proofdb_harvest: unknown policy (expected {POLICY:?} — the only \
                         harvest policy since the plan18 policy consolidation)"
                    );
                    usage();
                }
            }
            "--ledger" => a.ledger = PathBuf::from(next()),
            "--budget-evals" => a.budget_evals = next().parse().unwrap_or_else(|_| usage()),
            "--max-total-evals" => a.max_total_evals = next().parse().unwrap_or_else(|_| usage()),
            "--tt-mb" => a.tt_mb = next().parse().unwrap_or_else(|_| usage()),
            "--max-jobs" => a.max_jobs = next().parse().unwrap_or_else(|_| usage()),
            "--max-runtime" => a.max_runtime = next().parse().unwrap_or_else(|_| usage()),
            "--stop-file" => a.stop_file = PathBuf::from(next()),
            "--out-db" => a.out_db = Some(PathBuf::from(next())),
            "--dump" => a.dump = Some(PathBuf::from(next())),
            _ => usage(),
        }
    }
    a
}
