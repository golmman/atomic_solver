//! Binary-level tests for the `proofdb_harvest` scoped harvest (plan19):
//! the `--moves` fail-fast validation (usage exit 1 before any manifest/
//! DB state is touched), a scoped session's in-scope-only job paths and
//! preserved out-of-scope censor records, the proven-scope exhausted note,
//! and the absent-flag global shape (bit-identity at the CLI surface).
//! Since plan20 the censor state lives in the DB's `censor` table, so every
//! test works on its own copy of the fixture DB (the harvest writes censor
//! rows there) and seeds censor state with plain SQL. The example binaries
//! are driven through `cargo run --example` (the `test_benchmark_json`
//! harness style — `CARGO_BIN_EXE_*` is not set for example targets);
//! invocations are serialized by a lock because each one goes through
//! cargo's build lock.
//!
//! The fixture DB is built once per test process (lazily, shared) by a
//! real `proofdb_merge` over a one-shard manifest: root (open) + the spine
//! `f2f3` / `f2f3 e7e6` (open) + `f2f3 e7e6 g2g4` (win, bound 1) +
//! `f2f3 e7e6 g2g4 d8h4` (loss terminal).

use std::process::Command;
use std::sync::{Mutex, OnceLock};

use atomic_movegen::types::Move;

use atomic_solver::notation::uci_to_move;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofNode, ProofTree};

/// After 1.f3 e6 2.g4: Qd8-h4 explodes the White king (win in 1).
const TACTIC_FEN: &str = "rnbqkbnr/pppp1ppp/4p3/8/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2";

static RUN_LOCK: Mutex<()> = Mutex::new(());
static FIXTURE: OnceLock<String> = OnceLock::new();

fn cargo() -> Command {
    let mut c = Command::new("cargo");
    c.current_dir(env!("CARGO_MANIFEST_DIR"));
    c
}

/// Run `proofdb_merge` over the fixture manifest + shard → fixture DB.
fn build_fixture(dir: &std::path::Path) {
    // The win-in-1 shard (root + terminal child), rooted at TACTIC_FEN.
    let mut pos = Position::from_fen(TACTIC_FEN).unwrap();
    let mv = uci_to_move("d8h4", &pos).unwrap();
    pos.do_move(mv);
    let child = ProofNode {
        parent: Some(std::num::NonZeroU32::new(1).unwrap()),
        first_child: None,
        next_sibling: None,
        mv,
        hash: 0,
        outcome: Some(Outcome::Loss),
        depth: 0,
    };
    let root = ProofNode {
        parent: None,
        first_child: Some(std::num::NonZeroU32::new(1).unwrap()),
        next_sibling: None,
        mv: Move::NONE,
        hash: 0,
        outcome: Some(Outcome::Win),
        depth: 1,
    };
    let tree = ProofTree {
        root_fen: TACTIC_FEN.to_string(),
        nodes: vec![root, child],
    };
    let mut bytes = Vec::new();
    tree.to_bin(&mut bytes).expect("fixture serializes");
    std::fs::write(dir.join("clifix.bin"), bytes).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        format!(
            r#"{{"entries":[{{"tag":"clifix","file":"clifix.bin","fen":"{TACTIC_FEN}",
                "moves":["f2f3","e7e6","g2g4"],"outcome":"win","validate":"ok"}}]}}"#
        ),
    )
    .unwrap();
    let out = cargo()
        .args([
            "run",
            "--release",
            "--quiet",
            "--example",
            "proofdb_merge",
            "--",
            "--manifest",
        ])
        .arg(dir.join("manifest.json"))
        .args(["--shard-dir"])
        .arg(dir)
        .args(["--db"])
        .arg(dir.join("fixture.db"))
        .output()
        .expect("failed to run proofdb_merge");
    assert!(
        out.status.success(),
        "fixture merge failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The shared per-process fixture: `<dir>/fixture.db` + `<dir>/manifest.json`.
fn fixture() -> &'static str {
    FIXTURE.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!(
            "proofdb_harvest_cli_fixture_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        build_fixture(&dir);
        dir.to_str().unwrap().to_string()
    })
}

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Run `proofdb_harvest` with explicit paths (no defaults touched).
fn run_harvest(tmp: &std::path::Path, extra: &[&str]) -> Run {
    let _guard = RUN_LOCK.lock().unwrap();
    let mut args: Vec<String> = vec![
        "run".into(),
        "--release".into(),
        "--quiet".into(),
        "--example".into(),
        "proofdb_harvest".into(),
        "--".into(),
        "--db".into(),
        tmp.join("fixture.db").to_str().unwrap().into(),
        "--manifest".into(),
        tmp.join("shards/manifest.json").to_str().unwrap().into(),
        "--shard-dir".into(),
        tmp.join("shards").to_str().unwrap().into(),
        "--budget-evals".into(),
        "1000".into(),
        "--tt-mb".into(),
        "4".into(),
        "--stop-file".into(),
        tmp.join("STOP").to_str().unwrap().into(),
    ];
    args.extend(extra.iter().map(|s| (*s).to_string()));
    let out = cargo().args(&args).output().expect("failed to run harvest");
    Run {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("proofdb_harvest_cli_{name}_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("shards")).unwrap();
    std::fs::write(
        dir.join("shards/manifest.json"),
        std::fs::read(format!("{}/manifest.json", fixture())).unwrap(),
    )
    .unwrap();
    // The session's own DB copy: the harvest commits censor rows to it
    // (plan20), so the shared fixture DB must stay untouched.
    std::fs::copy(format!("{}/fixture.db", fixture()), dir.join("fixture.db")).unwrap();
    dir
}

/// A fresh directory with nothing in it — the fail-fast tests prove the
/// solver touches no state, so it must start truly empty.
fn empty_dir(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("proofdb_harvest_cli_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The censor state of the test's DB copy, as `(path, work_done,
/// passes_failed)` rows in path order.
fn censor_rows(dir: &std::path::Path) -> Vec<(String, u64, u32)> {
    let conn = rusqlite::Connection::open(dir.join("fixture.db")).unwrap();
    let mut stmt = conn
        .prepare("SELECT path, work_done, passes_failed FROM censor ORDER BY path")
        .unwrap();
    stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)? as u64,
            r.get::<_, i64>(2)? as u32,
        ))
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

/// Seed one censor row into the test's DB copy (the standing-layer shape:
/// censor state lives in the DB, plan20).
fn seed_censor(dir: &std::path::Path, path: &str, work: u64, passes: u32) {
    let conn = rusqlite::Connection::open(dir.join("fixture.db")).unwrap();
    conn.execute(
        "INSERT INTO censor (path, work_done, passes_failed) VALUES (?1, ?2, ?3)",
        rusqlite::params![path, work as i64, passes as i64],
    )
    .unwrap();
}

fn job_paths(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|l| l.starts_with("job: "))
        .filter_map(|l| {
            let v: serde_json::Value = serde_json::from_str(&l[5..]).ok()?;
            Some(v["path"].as_str().unwrap_or("").to_string())
        })
        .collect()
}

// --- S1: fail-fast validation (exit 1 before any state load) ---

#[test]
fn moves_nonsense_token_exits_1_before_any_state() {
    let dir = empty_dir("nonsense");
    let run = run_harvest(&dir, &["--moves", "e2e4 zz9x9", "--max-jobs", "1"]);
    assert_eq!(run.code, Some(1), "stderr: {}", run.stderr);
    assert!(run.stderr.contains("zz9x9"), "token named: {}", run.stderr);
    // Fail fast: nothing was created or read (the paths do not even exist).
    let created: Vec<_> = walk_files(&dir);
    assert!(created.is_empty(), "no state touched: {created:?}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn moves_illegal_prefix_exits_1() {
    let dir = empty_dir("badprefix");
    let run = run_harvest(&dir, &["--moves", "e2e4 e2e4", "--max-jobs", "1"]);
    assert_eq!(run.code, Some(1), "stderr: {}", run.stderr);
    assert!(
        run.stderr.contains("is not a legal move"),
        "unexpected: {}",
        run.stderr
    );
    let created: Vec<_> = walk_files(&dir);
    assert!(created.is_empty(), "no state touched: {created:?}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn moves_empty_value_exits_1() {
    let dir = empty_dir("empty");
    let run = run_harvest(&dir, &["--moves", "", "--max-jobs", "1"]);
    assert_eq!(run.code, Some(1), "stderr: {}", run.stderr);
    assert!(
        run.stderr.contains("non-empty"),
        "unexpected: {}",
        run.stderr
    );
    std::fs::remove_dir_all(&dir).ok();
}

fn walk_files(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p.to_str().unwrap().to_string());
            }
        }
    }
    out.sort();
    out
}

// --- S2/S3: scoped session behavior at the CLI surface ---

/// A scoped session: the census describes the scoped world, the jobs run
/// only inside the scope (virtual root at the open scope row, then its
/// first unhandled child), and the shared censor state's out-of-scope
/// record is preserved untouched.
#[test]
fn scoped_session_jobs_and_census_stay_in_scope() {
    let dir = temp_dir("scoped");
    // An out-of-scope censor record (work ≥ B — an expansion candidate in a
    // global session, invisible here), seeded into the DB (plan20).
    seed_censor(&dir, "b1c3", 50_000, 1);
    let run = run_harvest(&dir, &["--moves", "f2f3 e7e6", "--max-jobs", "2"]);
    assert_eq!(run.code, Some(0), "stderr: {}", run.stderr);
    // The session-start census: scoped world, out-of-scope count, root
    // state of the scope row (open).
    assert!(
        run.stderr.contains(
            "records 1 (lineage 0, ancestor 0, out_of_scope 1, complete 0) \
             — root open; base budget 1000; scope f2f3 e7e6",
        ),
        "census: {}",
        run.stderr
    );
    assert!(
        run.stderr.contains("; scope f2f3 e7e6"),
        "echo: {}",
        run.stderr
    );
    // Jobs: the virtual root at the scope key, then its first unhandled
    // child — in scope by construction (after 1.f3 e6 White is to move
    // again; the lex-first legal child is a2a3).
    assert_eq!(job_paths(&run.stdout), vec!["f2f3 e7e6", "f2f3 e7e6 a2a3"]);
    assert!(run.stdout.contains(r#""class":"C1""#));
    assert!(run.stdout.contains(r#""class":"C3""#));
    assert!(run.stdout.contains(r#""kind":"expand""#));
    // The censor state (read back from the DB): two in-scope records
    // joined at ≥ the budget, the out-of-scope record preserved value-wise.
    let rows = censor_rows(&dir);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], ("b1c3".to_string(), 50_000, 1));
    assert_eq!(rows[1].0, "f2f3 e7e6");
    assert!(rows[1].1 >= 1_000 && rows[1].2 == 1, "{:?}", rows[1]);
    assert_eq!(rows[2].0, "f2f3 e7e6 a2a3");
    assert!(rows[2].1 >= 1_000 && rows[2].2 == 1, "{:?}", rows[2]);
    std::fs::remove_dir_all(&dir).ok();
}

/// A proven-stored scope root (a proven DB row) ends the session
/// `exhausted` before any job, with the explanatory stderr note (S2).
#[test]
fn proven_scope_root_exhausts_with_note() {
    let dir = temp_dir("proven");
    let run = run_harvest(&dir, &["--moves", "f2f3 e7e6 g2g4"]);
    assert_eq!(run.code, Some(0), "stderr: {}", run.stderr);
    assert!(
        run.stderr
            .contains("scope root \"f2f3 e7e6 g2g4\" is proven-stored — nothing to harvest",),
        "note: {}",
        run.stderr
    );
    assert!(
        run.stderr
            .contains("— root proven; base budget 1000; scope f2f3 e7e6 g2g4"),
        "census: {}",
        run.stderr
    );
    assert!(!run.stdout.contains("job: "), "no jobs ran: {}", run.stdout);
    assert!(
        run.stdout.contains("stop=exhausted jobs 0"),
        "summary: {}",
        run.stdout
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// The absent `--moves` flag keeps the global session's exact CLI shape:
/// `scope global` in the census, no `; scope` echo, and the manual's
/// pinned global smoke shape (`""` then `a2a3`, C1 then C3).
#[test]
fn absent_moves_flag_keeps_the_global_shape() {
    let dir = temp_dir("global");
    let run = run_harvest(&dir, &["--max-jobs", "2"]);
    assert_eq!(run.code, Some(0), "stderr: {}", run.stderr);
    assert!(
        run.stderr
            .contains("out_of_scope 0, complete 0) — root open; base budget 1000; scope global"),
        "census: {}",
        run.stderr
    );
    assert_eq!(job_paths(&run.stdout), vec!["", "a2a3"]);
    assert!(run.stdout.contains(r#""class":"C1""#));
    assert!(run.stdout.contains(r#""class":"C3""#));
    // The `harvest: db` echo (stderr's first line) carries no scope.
    let echo = run.stderr.lines().next().unwrap();
    assert!(!echo.contains("scope"), "no scope echo: {echo}");
    std::fs::remove_dir_all(&dir).ok();
}
