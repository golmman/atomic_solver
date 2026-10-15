//! Binary-level tests for `proofdb_merge`'s censor-state contract (plan20
//! D5/D6): the full rebuild preserves the DB's `censor` rows exactly, and
//! `--import-censor` merges a legacy `work.json` under the per-field max
//! rule (idempotent; never decreases a stored value). The example binary is
//! driven through `cargo run --example` (the `proofdb_harvest_cli` harness
//! style); the fixture is a one-shard manifest over a temp layer.

use std::process::Command;
use std::sync::{Mutex, OnceLock};

use atomic_movegen::types::Move;

use atomic_solver::notation::uci_to_move;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofNode, ProofTree};

static RUN_LOCK: Mutex<()> = Mutex::new(());
static FIXTURE: OnceLock<String> = OnceLock::new();

/// After 1.f3 e6 2.g4: Qd8-h4 explodes the White king (win in 1).
const TACTIC_FEN: &str = "rnbqkbnr/pppp1ppp/4p3/8/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2";

fn cargo() -> Command {
    let mut c = Command::new("cargo");
    c.current_dir(env!("CARGO_MANIFEST_DIR"));
    c
}

/// Build the one-shard fixture: root (open) + spine `f2f3` / `f2f3 e7e6` /
/// `f2f3 e7e6 g2g4` (win) + the `d8h4` loss terminal.
fn build_fixture(dir: &std::path::Path) {
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
}

/// The shared per-process fixture: `<dir>/manifest.json` + `<dir>/clifix.bin`.
fn fixture() -> &'static str {
    FIXTURE.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("proofdb_merge_cli_fixture_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        build_fixture(&dir);
        dir.to_str().unwrap().to_string()
    })
}

struct Run {
    code: Option<i32>,
    stdout: String,
}

/// Run `proofdb_merge` with explicit fixture paths and `extra` arguments.
fn run_merge(db: &std::path::Path, extra: &[&str]) -> Run {
    let _guard = RUN_LOCK.lock().unwrap();
    let fixture = fixture();
    let mut args: Vec<String> = vec![
        "run".into(),
        "--release".into(),
        "--quiet".into(),
        "--example".into(),
        "proofdb_merge".into(),
        "--".into(),
        "--manifest".into(),
        format!("{fixture}/manifest.json"),
        "--shard-dir".into(),
        fixture.to_string(),
        "--db".into(),
        db.to_str().unwrap().into(),
    ];
    args.extend(extra.iter().map(|s| (*s).to_string()));
    let out = cargo().args(&args).output().expect("failed to run merge");
    Run {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
    }
}

/// The DB's censor rows as `(path, work_done, passes_failed)` in path order.
fn censor_rows(db: &std::path::Path) -> Vec<(String, u64, u32)> {
    let conn =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
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

fn seed(db: &std::path::Path, path: &str, work: u64, passes: u32) {
    let conn = rusqlite::Connection::open(db).unwrap();
    conn.execute(
        "INSERT INTO censor (path, work_done, passes_failed) VALUES (?1, ?2, ?3)",
        rusqlite::params![path, work as i64, passes as i64],
    )
    .unwrap();
}

/// D5 (normative edge case, through the real binary): a bare re-merge over
/// a DB carrying censor rows preserves them row-for-row — including a key
/// with no DB row at all (never proven).
#[test]
fn merge_preserves_censor_rows_across_rebuild() {
    let dir = std::env::temp_dir().join(format!("proofdb_merge_cli_p20_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("proofdb.db");
    assert!(run_merge(&db, &[]).code == Some(0));
    seed(&dir.join("proofdb.db"), "g1f3 g8f6 f3g1 f6g8", 4, 2);
    seed(&dir.join("proofdb.db"), "", 4_000_000, 1);

    let run = run_merge(&db, &[]);
    assert_eq!(run.code, Some(0), "stdout: {}", run.stdout);
    assert!(
        run.stdout.contains("censor: 2 censor rows preserved"),
        "census line: {}",
        run.stdout
    );
    assert_eq!(
        censor_rows(&db),
        vec![
            ("".to_string(), 4_000_000, 1),
            ("g1f3 g8f6 f3g1 f6g8".to_string(), 4, 2),
        ]
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// D6 through the real binary: `--import-censor` merges a legacy work.json
/// (new keys inserted, stored values never decreased), is idempotent on a
/// repeated import, and is reported on stdout.
#[test]
fn merge_import_censor_is_max_rule_and_idempotent() {
    let dir = std::env::temp_dir().join(format!("proofdb_merge_cli_import_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("proofdb.db");
    let legacy = dir.join("work.json");
    std::fs::write(
        &legacy,
        r#"{"records":[
            {"path":["a2a3"],"work_done":100,"passes_failed":1},
            {"path":["f2f3"],"work_done":9000000,"passes_failed":3}]}"#,
    )
    .unwrap();

    // First merge imports both records…
    let run = run_merge(&db, &["--import-censor", legacy.to_str().unwrap()]);
    assert_eq!(run.code, Some(0), "stdout: {}", run.stdout);
    assert!(
        run.stdout
            .contains("censor: 0 censor rows preserved + 2 imported records -> 2 rows"),
        "census line: {}",
        run.stdout
    );
    // …but the stored (newer) row at "f2f3"-shaped paths is never decreased:
    // seed a newer value first to prove the max rule across the import.
    let dir2 =
        std::env::temp_dir().join(format!("proofdb_merge_cli_import2_{}", std::process::id()));
    std::fs::create_dir_all(&dir2).unwrap();
    let db2 = dir2.join("proofdb.db");
    assert!(run_merge(&db2, &[]).code == Some(0));
    seed(&db2, "f2f3", 50_000_000, 5);
    let run2 = run_merge(&db2, &["--import-censor", legacy.to_str().unwrap()]);
    assert_eq!(run2.code, Some(0));
    assert_eq!(
        censor_rows(&db2),
        vec![
            ("a2a3".to_string(), 100, 1),
            ("f2f3".to_string(), 50_000_000, 5),
        ],
        "stale import never decreases a stored row"
    );
    // Idempotence: a second identical import changes nothing.
    let run3 = run_merge(&db2, &["--import-censor", legacy.to_str().unwrap()]);
    assert_eq!(run3.code, Some(0));
    assert!(
        run3.stdout.contains("+ 2 imported records -> 2 rows"),
        "census line: {}",
        run3.stdout
    );
    assert_eq!(
        censor_rows(&db2),
        vec![
            ("a2a3".to_string(), 100, 1),
            ("f2f3".to_string(), 50_000_000, 5),
        ]
    );
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&dir2).ok();
}
