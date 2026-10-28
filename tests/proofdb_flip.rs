//! Binary-level tests for `proofdb_flip`'s OR-witness semantics (plan26).
//!
//! **The fixture DBs are synthetic and not fold-producible**: a shard never
//! carries an open parent with a proven child (the merge only grafts proven
//! subtrees), and neither live layer has an open row with a proven loss
//! child — so the two shapes are hand-built here: `schema::ensure_fold_schema`
//! plus plain `nodes`/`meta` SQL, an empty `shards` table, an empty shard
//! dir. Do not "fix" these into merge tests.
//!
//! Fixture 1 is the cited production shape: an open ply-8 row with one
//! proven loss child `b5c7` (bound 22) and censored siblings (censors are
//! scheduling state, not rows) — an OR-witness claim that used to abort with
//! `reply a2a3 has no DB row`. Fixture 2 is the derive-once bound divergence:
//! an and-complete root whose tightest loss witness is *implied*, so the
//! converged claim is 4 while a derive-once pass says 11.
//!
//! The example binary is driven through `cargo run --example` (the harness
//! style of `tests/proofdb_harvest_cli.rs`); invocations are serialized by a
//! lock because each goes through cargo's build lock.

#[path = "../examples/proofdb/mod.rs"]
mod proofdb;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::Position;

use proofdb::schema::ensure_fold_schema;

static RUN_LOCK: Mutex<()> = Mutex::new(());

/// One synthetic `nodes` row: (space-joined path, ply, outcome, depth bound,
/// depth status).
type Row = (
    String,
    usize,
    Option<&'static str>,
    Option<u32>,
    Option<&'static str>,
);

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("proofdb_flip_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("shards")).unwrap();
    dir
}

/// Write the synthetic schema-v2 DB: ids by listing order, `parent_id`
/// resolved from the path map (so rows may be listed in any order as long as
/// every parent is present). The `shards` table stays empty and the shard dir
/// stays empty (the unmerged-shard gate passes).
fn build_db(dir: &Path, rows: &[Row]) -> PathBuf {
    let db = dir.join("flip.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    ensure_fold_schema(&conn).unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO meta VALUES ('built_from', ?1)",
        ["0".repeat(64)],
    )
    .unwrap();
    let ids: std::collections::HashMap<&str, i64> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (r.0.as_str(), (i + 1) as i64))
        .collect();
    let mut stmt = conn
        .prepare(
            "INSERT INTO nodes (id, parent_id, ply, move_uci, outcome,
                                depth_bound, depth_status, provenance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'synthetic')",
        )
        .unwrap();
    for (i, (path, ply, outcome, bound, status)) in rows.iter().enumerate() {
        let parent = match path.rfind(' ') {
            Some(cut) => Some(ids[&path[..cut]]),
            None if path.is_empty() => None,
            None => Some(ids[""]),
        };
        let mv = path.rsplit(' ').next().filter(|_| !path.is_empty());
        stmt.execute(rusqlite::params![
            (i + 1) as i64,
            parent,
            *ply as i64,
            mv,
            outcome,
            bound,
            status
        ])
        .unwrap();
    }
    db
}

fn run_flip(db: &Path, dir: &Path) -> std::process::Output {
    let out = dir.join("flip.json");
    Command::new("cargo")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "run",
            "--quiet",
            "--release",
            "--example",
            "proofdb_flip",
            "--",
        ])
        .arg("--db")
        .arg(db)
        .arg("--shard-dir")
        .arg(dir.join("shards"))
        .arg("--out")
        .arg(&out)
        .output()
        .expect("cargo run proofdb_flip")
}

fn legal_ucis(fen: &str) -> Vec<String> {
    let pos = Position::from_fen(fen).unwrap();
    pos.legal_moves_vec()
        .iter()
        .map(|&m| move_to_uci(m))
        .collect()
}

/// The cited production shape: `g1f3 d7d6 b1c3 c7c6 f3g5 f7f6 c3b5 f6f5` is
/// open at ply 8 with one proven loss child `b5c7` (bound 22) and 29
/// unresolved replies (censored siblings, which have no rows).
#[test]
fn or_witness_flip_reports_coverage_and_bound() {
    let _guard = RUN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tmp_dir("or_witness");
    let spine = [
        "",
        "g1f3",
        "g1f3 d7d6",
        "g1f3 d7d6 b1c3",
        "g1f3 d7d6 b1c3 c7c6",
        "g1f3 d7d6 b1c3 c7c6 f3g5",
        "g1f3 d7d6 b1c3 c7c6 f3g5 f7f6",
        "g1f3 d7d6 b1c3 c7c6 f3g5 f7f6 c3b5",
        "g1f3 d7d6 b1c3 c7c6 f3g5 f7f6 c3b5 f6f5",
    ];
    let mut rows: Vec<Row> = spine
        .iter()
        .enumerate()
        .map(|(ply, p)| ((*p).to_string(), ply, None, None, None))
        .collect();
    rows.push((
        format!("{} b5c7", spine[8]),
        9,
        Some("loss"),
        Some(22),
        Some("bound"),
    ));
    let db = build_db(&dir, &rows);
    let o = run_flip(&db, &dir);
    let stdout = String::from_utf8_lossy(&o.stdout);
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(0), "stdout {stdout} stderr {stderr}");
    assert!(
        stdout.contains("or-witness 1, and-complete 0, exact 0"),
        "coverage counts missing: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("flip.json")).unwrap()).unwrap();
    assert_eq!(json["flips_total"], 1, "{json}");
    assert_eq!(json["flips_or_witness"], 1, "{json}");
    assert_eq!(json["flips_and_complete"], 0, "{json}");
    let f = &json["flips"][0];
    assert_eq!(f["ply"], 8, "{f}");
    assert_eq!(f["implied"], "root-win", "{f}");
    assert_eq!(f["side_to_move"], "win", "{f}");
    assert_eq!(f["bound"], 23, "{f}");
    assert_eq!(f["coverage"], "or-witness", "{f}");
    assert_eq!(f["bound_mode"], "upper", "{f}");
    assert_eq!(f["loss_witnesses"], 1, "{f}");
    assert_eq!(f["unresolved_replies"], 29, "{f}");
}

/// The derive-once bound divergence: 17 replies proven `win` (bound 2), one
/// proven loss witness (bound 10) at the root, and one open child whose 20
/// replies are all proven `win` (bound 2) — so the child claims `loss`
/// bound 3 and the root's converged claim is `min(10, 3) + 1 = 4`, not the
/// round-1 `11`.
#[test]
fn or_bound_is_recomputed_to_the_fixpoint() {
    let _guard = RUN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tmp_dir("or_bound");
    let root_moves = legal_ucis(Position::STARTPOS_FEN);
    assert_eq!(root_moves.len(), 20);
    let open_child = "a2a3";
    let loss_witness = "g1f3";
    let mut rows: Vec<Row> = vec![(String::new(), 0, None, None, None)];
    for uci in &root_moves {
        let (outcome, bound) = if uci == loss_witness {
            (Some("loss"), Some(10))
        } else if uci == open_child {
            (None, None)
        } else {
            (Some("win"), Some(2))
        };
        rows.push((uci.clone(), 1, outcome, bound, outcome.map(|_| "bound")));
    }
    let mut after = Position::from_fen(Position::STARTPOS_FEN).unwrap();
    let mv = uci_to_move(open_child, &after).unwrap();
    after.do_move(mv);
    let mut ucis = after
        .legal_moves_vec()
        .iter()
        .map(|&m| move_to_uci(m))
        .collect::<Vec<_>>();
    ucis.sort();
    assert_eq!(ucis.len(), 20);
    for uci in &ucis {
        rows.push((
            format!("{open_child} {uci}"),
            2,
            Some("win"),
            Some(2),
            Some("bound"),
        ));
    }
    let db = build_db(&dir, &rows);
    let o = run_flip(&db, &dir);
    let stdout = String::from_utf8_lossy(&o.stdout);
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(0), "stdout {stdout} stderr {stderr}");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("flip.json")).unwrap()).unwrap();
    assert_eq!(json["flips_total"], 2, "root + open child: {json}");
    assert_eq!(
        json["flips_verified_independently"], json["flips_total"],
        "{json}"
    );
    let root = json["flips"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "")
        .unwrap_or_else(|| panic!("no root flip: {json}"));
    assert_eq!(root["bound"], 4, "{root}");
    assert_eq!(root["coverage"], "and-complete", "{root}");
    assert_eq!(root["bound_mode"], "upper", "{root}");
    assert_eq!(root["loss_witnesses"], 2, "{root}");
    assert_eq!(root["unresolved_replies"], 0, "{root}");
    assert!(stdout.contains("flips 2 verified 2"), "{stdout}");
}
