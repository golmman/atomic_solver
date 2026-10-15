//! The `proofdb_harvest` CLI arguments (plan2/3/4/6; slimmed by the plan18
//! policy consolidation; censor state moved into the DB by plan20):
//! `Args`, `usage`, `parse_args`. Split from the example target root for
//! the 10 KB file-size convention; `main` remains there.
//!
//! Generated state defaults wholly under `data/proofdb/` (plan14): DB,
//! manifest, and shard dir — a production harvest needs no path flags.
//! The censor state lives in the DB's `censor` table (schema v2); the
//! pre-plan20 `--ledger <work.json>` flag is retired with the JSON
//! sidecar.

use std::path::PathBuf;

use atomic_solver::notation::uci_to_move;
use atomic_solver::position::Position;

use super::{DEFAULT_DB, DEFAULT_MANIFEST, DEFAULT_SHARD_DIR};

/// The only harvest policy (plan18 policy consolidation): the plan16
/// shallowest-first ledger walk. The `--policy` flag is kept so existing
/// command lines keep working; it accepts only this name.
pub const POLICY: &str = "frontier-walk";

pub struct Args {
    pub db: PathBuf,
    pub manifest: PathBuf,
    pub shard_dir: PathBuf,
    pub budget_evals: u64,
    pub max_total_evals: u64,
    pub tt_mb: usize,
    pub max_jobs: usize,
    pub max_runtime: u64,
    pub stop_file: PathBuf,
    pub out_db: Option<PathBuf>,
    pub dump: Option<PathBuf>,
    /// The scoped-harvest startpos path (plan19): whitespace-separated UCI
    /// moves, replay-validated; empty = the flag's absence = global session.
    pub moves: Vec<String>,
}

fn usage() -> ! {
    eprintln!(
        "usage: proofdb_harvest [--db <proofdb.db>] [--manifest <manifest.json>] \
         [--shard-dir <dir>] [--policy frontier-walk] \
         [--budget-evals <n>] [--max-total-evals <n>] [--tt-mb <mb>] \
         [--max-jobs <n>] [--max-runtime <s>] [--stop-file <path>] \
         [--moves '<uci path>'] \
         [--out-db <grown.db>] [--dump <nodes.txt>]  \
         (defaults: --db data/proofdb/proofdb.db --manifest data/proofdb/shards/manifest.json \
         --shard-dir data/proofdb/shards; \
         policy: frontier-walk — the only harvest policy; --budget-evals 0 \
         selects the standing base 4000000; --moves scopes the session to \
         the subtree of the startpos path it names; censor state lives in \
         the DB's censor table — the pre-plan20 --ledger flag is retired)"
    );
    std::process::exit(1);
}

pub fn parse_args() -> Args {
    let mut a = Args {
        db: PathBuf::from(DEFAULT_DB),
        manifest: PathBuf::from(DEFAULT_MANIFEST),
        shard_dir: PathBuf::from(DEFAULT_SHARD_DIR),
        budget_evals: 0,
        max_total_evals: 0,
        tt_mb: 128,
        max_jobs: 0,
        max_runtime: 0,
        stop_file: PathBuf::from("STOP"),
        out_db: None,
        dump: None,
        moves: Vec::new(),
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
            "--ledger" => {
                eprintln!(
                    "proofdb_harvest: --ledger is retired (plan20): censor state \
                     lives in the DB's censor table; migrate a legacy work.json \
                     with proofdb_merge --import-censor"
                );
                usage()
            }
            "--budget-evals" => a.budget_evals = next().parse().unwrap_or_else(|_| usage()),
            "--max-total-evals" => a.max_total_evals = next().parse().unwrap_or_else(|_| usage()),
            "--tt-mb" => a.tt_mb = next().parse().unwrap_or_else(|_| usage()),
            "--max-jobs" => a.max_jobs = next().parse().unwrap_or_else(|_| usage()),
            "--max-runtime" => a.max_runtime = next().parse().unwrap_or_else(|_| usage()),
            "--stop-file" => a.stop_file = PathBuf::from(next()),
            "--out-db" => a.out_db = Some(PathBuf::from(next())),
            "--dump" => a.dump = Some(PathBuf::from(next())),
            "--moves" => {
                let raw = next();
                a.moves = parse_moves(Some(&raw)).unwrap_or_else(|e| {
                    eprintln!("proofdb_harvest: {e}");
                    usage();
                });
            }
            _ => usage(),
        }
    }
    a
}

/// The `--moves` value (plan19 S1): the scoped harvest's startpos path.
/// `None` = the flag's absence = a global session (`Ok(vec![])`). The raw
/// string is split on whitespace (the path-key join; no comma dialect) and
/// replayed sequentially from the startpos — a malformed token or a line
/// that becomes illegal at any prefix is rejected with a message naming
/// the offending token. Called from [`parse_args`], before any manifest/
/// DB state is loaded (fail fast, usage-class exit 1).
///
/// # Errors
/// An empty value, or a token that is not a legal move at its path prefix.
pub(crate) fn parse_moves(raw: Option<&str>) -> Result<Vec<String>, String> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    if raw.trim().is_empty() {
        return Err(
            "--moves requires a non-empty space-separated UCI path (the flag's \
             absence already means a global session)"
                .to_string(),
        );
    }
    let tokens: Vec<String> = raw.split_whitespace().map(str::to_string).collect();
    let mut pos = Position::from_fen(Position::STARTPOS_FEN).expect("startpos FEN parses");
    for (i, t) in tokens.iter().enumerate() {
        match uci_to_move(t, &pos) {
            Some(mv) => pos.do_move(mv),
            None => {
                return Err(format!(
                    "--moves: token {t:?} at ply {i} is not a legal move — the whole \
                     path must replay legally from the startpos"
                ));
            }
        }
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::parse_moves;

    #[test]
    fn moves_good_path_and_whitespace_normalization() {
        assert_eq!(
            parse_moves(Some("a2a4 b7b5 b1a3")).unwrap(),
            vec!["a2a4", "b7b5", "b1a3"]
        );
        // Tokens are whitespace-separated exactly (the path-key join); any
        // whitespace run separates, so the joined scope key is normalized.
        assert_eq!(
            parse_moves(Some("  e2e4\te7e5\n  g1f3")).unwrap(),
            vec!["e2e4", "e7e5", "g1f3"]
        );
    }

    #[test]
    fn moves_absent_is_global() {
        assert!(parse_moves(None).unwrap().is_empty());
    }

    #[test]
    fn moves_empty_value_rejected() {
        // The flag's absence already means global; an empty value is a
        // mistake, not a global session.
        assert!(parse_moves(Some("")).is_err());
        assert!(parse_moves(Some("   ")).is_err());
    }

    #[test]
    fn moves_nonsense_token_rejected_with_the_token_named() {
        let e = parse_moves(Some("e2e4 zz9x9")).unwrap_err();
        assert!(e.contains("zz9x9"), "unexpected: {e}");
        let e = parse_moves(Some("e2e4,e7e5")).unwrap_err();
        assert!(e.contains("e2e4,e7e5"), "no comma dialect: {e}");
    }

    #[test]
    fn moves_illegal_prefix_rejected() {
        // A legal-looking token that is illegal at its prefix (after e2e4
        // the pawn is gone; e2e5 is never a pawn move anyway).
        let e = parse_moves(Some("e2e4 e2e4")).unwrap_err();
        assert!(e.contains("e2e4"), "unexpected: {e}");
        assert!(parse_moves(Some("d7d5 e7e5")).is_err(), "white to move");
    }
}
