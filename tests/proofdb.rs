//! Integration harness for the proofdb merger (examples-side tooling).
//!
//! `cargo test` does not run unit tests inside example targets, so the merger
//! modules are included here verbatim via `#[path]` (their inline unit tests
//! then run as part of this target). This file additionally covers the
//! end-to-end pipeline over a synthetic fixture shard.

#[path = "../examples/proofdb/mod.rs"]
mod proofdb;

use atomic_movegen::types::Move;

use atomic_solver::notation::uci_to_move;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::{ProofNode, ProofTree, validate_proof_tree};

use proofdb::merge::PathTree;
use proofdb::read_manifest;
use proofdb::schema::{ShardRow, canonical_dump, write_db};

const ROOT_FEN: &str = Position::STARTPOS_FEN;
/// After 1.f3 e6 2.g4: Qd8-h4 explodes the White king (win in 1).
const TACTIC_FEN: &str = "rnbqkbnr/pppp1ppp/4p3/8/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2";
const TACTIC_PATH: &[&str] = &["f2f3", "e7e6", "g2g4"];

/// Build the win-in-1 fixture tree (root + terminal child) and serialize it.
fn fixture_bin() -> Vec<u8> {
    let mut pos = Position::from_fen(TACTIC_FEN).unwrap();
    let mv = uci_to_move("d8h4", &pos).expect("d8h4 is legal and wins");
    pos.do_move(mv);
    let child = ProofNode {
        parent: Some(std::num::NonZeroU32::new(1).expect("nonzero")),
        first_child: None,
        next_sibling: None,
        mv,
        hash: 0,
        outcome: Some(Outcome::Loss),
        depth: 0,
    };
    let root = ProofNode {
        parent: None,
        first_child: Some(std::num::NonZeroU32::new(1).expect("nonzero")),
        next_sibling: None,
        mv: atomic_movegen::types::Move::NONE,
        hash: 0,
        outcome: Some(Outcome::Win),
        depth: 1,
    };
    let tree = ProofTree {
        root_fen: TACTIC_FEN.to_string(),
        nodes: vec![root, child],
    };
    assert!(
        validate_proof_tree(&tree).is_ok(),
        "fixture must be a valid proof"
    );
    let mut buf = Vec::new();
    tree.to_bin(&mut buf).expect("fixture serializes");
    buf
}

fn moves_from_path(ucis: &[impl AsRef<str>]) -> Vec<Move> {
    let mut pos = Position::from_fen(ROOT_FEN).unwrap();
    ucis.iter()
        .map(|u| {
            let mv = uci_to_move(u.as_ref(), &pos).expect("legal");
            pos.do_move(mv);
            mv
        })
        .collect()
}

#[test]
fn end_to_end_fixture_pipeline() {
    let dir = std::env::temp_dir().join(format!("proofdb_e2e_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("fixture.bin"), fixture_bin()).unwrap();
    let manifest_path = dir.join("manifest.json");
    std::fs::write(
        &manifest_path,
        format!(
            r#"{{"entries":[{{"tag":"fixture","file":"fixture.bin","fen":"{TACTIC_FEN}",
                "moves":["f2f3","e7e6","g2g4"],"outcome":"win","validate":"ok"}}]}}"#
        ),
    )
    .unwrap();
    let manifest = read_manifest(&manifest_path).unwrap();
    assert_eq!(manifest.entries.len(), 1);

    // The merger pipeline (as proofdb_merge drives it) over the fixture.
    let mut tree = PathTree::new();
    let mut rows = Vec::new();
    let mut graft_idx = 0usize;
    for entry in &manifest.entries {
        let bytes = std::fs::read(dir.join(&entry.file)).unwrap();
        let mut shard = ProofTree::from_bin(&mut bytes.as_slice()).unwrap();
        assert!(validate_proof_tree(&shard).is_ok());
        assert_eq!(shard.nodes[0].outcome, Some(entry.outcome));
        let moves = moves_from_path(&entry.moves);
        let graft = tree.graft_path(&moves, &entry.tag);
        graft_idx = graft;
        tree.overlay_subtree(graft, &shard, &entry.tag).unwrap();
        rows.push(ShardRow {
            tag: entry.tag.clone(),
            file: entry.file.clone(),
            sha256: proofdb::digest_hex(&bytes),
            root_fen: std::mem::take(&mut shard.root_fen),
            path: entry.moves.join(" "),
            outcome: entry.outcome,
            depth_bound: 1,
            n_nodes: 2,
        });
    }
    tree.finalize().unwrap();
    let skeleton = tree.reassemble_skeleton(graft_idx, TACTIC_FEN).unwrap();
    assert!(validate_proof_tree(&skeleton).is_ok());

    let db_path = dir.join("proofdb.db");
    write_db(&db_path, ROOT_FEN, &manifest.sha256_hex, &rows, &tree).unwrap();
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let schema_version: String = conn
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "1");
    let node_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(node_count, 5); // root + 3 graft ancestors + shard root + terminal
    let (outcome, bound, status, prov): (Option<String>, Option<i64>, Option<String>, String) =
        conn.query_row(
            "SELECT outcome, depth_bound, depth_status, provenance FROM nodes
             WHERE ply=3 AND move_uci='g2g4'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(outcome.as_deref(), Some("win"));
    assert_eq!(bound, Some(1));
    assert_eq!(status.as_deref(), Some("bound"));
    assert_eq!(prov, "fixture");

    // Determinism: rebuilding from the same shard set is byte-identical.
    let dump1 = canonical_dump(ROOT_FEN, &manifest.sha256_hex, &tree);
    let mut tree2 = PathTree::new();
    tree2.graft_path(&moves_from_path(TACTIC_PATH), "fixture");
    let bytes = std::fs::read(dir.join("fixture.bin")).unwrap();
    let shard = ProofTree::from_bin(&mut bytes.as_slice()).unwrap();
    let graft2 = tree2.graft_path(&moves_from_path(TACTIC_PATH), "fixture");
    tree2.overlay_subtree(graft2, &shard, "fixture").unwrap();
    tree2.finalize().unwrap();
    assert_eq!(
        dump1,
        canonical_dump(ROOT_FEN, &manifest.sha256_hex, &tree2)
    );

    std::fs::remove_dir_all(&dir).ok();
}

// --- plan7 D1: the N-way ledger union (examples/proofdb/ledger_union.rs) ---

use proofdb::ledger::{Ledger, LedgerEntry};
use proofdb::ledger_union::union;

/// Build a ledger from `(space-joined path key, work_done, passes_failed)`
/// records (the key "" is the root, exactly as `Ledger::load` produces).
fn ledger_of(records: &[(&str, u64, u32)]) -> Ledger {
    let mut m = std::collections::BTreeMap::new();
    for (key, work, passes) in records {
        let inserted = m.insert(
            (*key).to_string(),
            LedgerEntry {
                work_done: *work,
                passes_failed: *passes,
            },
        );
        assert!(inserted.is_none(), "duplicate test key {key:?}");
    }
    Ledger::from_entries(m)
}

fn entries(led: &Ledger) -> Vec<(String, u64, u32)> {
    led.iter()
        .map(|(k, e)| (k.clone(), e.work_done, e.passes_failed))
        .collect()
}

/// §2.2 coverage: for every input and every path in it, the union's record
/// has `passes_failed ≥` that input's — and at equal passes,
/// `work_done ≥` too (the same-pass max).
fn assert_coverage(union_led: &Ledger, inputs: &[Ledger]) {
    for input in inputs {
        for (k, e) in input.iter() {
            let u = union_led
                .get(k)
                .unwrap_or_else(|| panic!("coverage: missing {k:?}"));
            assert!(
                u.passes_failed > e.passes_failed
                    || (u.passes_failed == e.passes_failed && u.work_done >= e.work_done),
                "coverage violated at {k:?}: input {e:?}, union {u:?}"
            );
        }
    }
}

#[test]
fn union_rule_edges_pass_upgrade_and_work_tiebreak() {
    // Pass upgrade beats any work_done at a lower pass; the winning pass's
    // work is kept (a lower pass's higher work does not leak through).
    let a = ledger_of(&[("", 0, 0), ("a2a3", 100_000_000, 0)]);
    let b = ledger_of(&[("", 4_000_000, 1), ("a2a3", 4_000_000, 1)]);
    let (u, _) = union(&[a, b]);
    assert_eq!(
        u.get(""),
        Some(&LedgerEntry {
            work_done: 4_000_000,
            passes_failed: 1
        })
    );
    assert_eq!(
        u.get("a2a3"),
        Some(&LedgerEntry {
            work_done: 4_000_000,
            passes_failed: 1
        })
    );

    // Pass tie → the higher work_done wins.
    let a = ledger_of(&[("b1c3", 8_000_000, 1)]);
    let b = ledger_of(&[("b1c3", 4_000_000, 1)]);
    let (u, _) = union(&[a, b]);
    assert_eq!(
        u.get("b1c3"),
        Some(&LedgerEntry {
            work_done: 8_000_000,
            passes_failed: 1
        })
    );

    // Full pass+work tie → either record (identical) stands.
    let a = ledger_of(&[("b1c3", 8_000_000, 1)]);
    let (u, st) = union(&[a.clone(), a]);
    assert_eq!(
        u.get("b1c3"),
        Some(&LedgerEntry {
            work_done: 8_000_000,
            passes_failed: 1
        })
    );
    assert_eq!(st.union_records, 1);
    assert_eq!(st.new_paths, [0, 0]); // known in both inputs
    assert_eq!(st.sole_pass_upgrades, [0, 0]); // no strict pass upgrade anywhere
}

#[test]
fn union_disjoint_paths_and_nway_order_independence() {
    let a = ledger_of(&[("", 0, 0), ("a2a3", 4_000_000, 1)]);
    let b = ledger_of(&[("b1c3", 0, 0)]);
    let c = ledger_of(&[("a2a3 a7a5", 4_000_009, 1), ("c2c3", 4_000_000, 1)]);
    let base = union(&[a.clone(), b.clone(), c.clone()]).0;
    assert_eq!(base.len(), 5);
    // Every input order (and pairwise repetition) gives the same records.
    for perm in [
        vec![a.clone(), b.clone(), c.clone()],
        vec![c.clone(), b.clone(), a.clone()],
        vec![b.clone(), a.clone(), c.clone()],
        vec![c.clone(), a.clone(), b.clone()],
        vec![a.clone(), a.clone(), b.clone(), c.clone(), a.clone()],
    ] {
        let (u, _) = union(&perm);
        assert_eq!(entries(&u), entries(&base), "order dependence");
    }
    assert_coverage(&base, &[a.clone(), b.clone(), c.clone()]);
    // Attribution: a and c each solely know one path; c holds the sole
    // pass upgrade at "a2a3 a7a5"? No — no other input knows it, so it is
    // a new path, not an upgrade.
    let (_, st) = union(&[a, b, c.clone()]);
    assert_eq!(st.new_paths, [2, 1, 2]); // ""+a2a3 / b1c3 / a2a3 a7a5+c2c3
    assert_eq!(st.sole_pass_upgrades, [0, 0, 0]);
}

#[test]
fn union_idempotent_and_normalize_single_input() {
    let a = ledger_of(&[
        ("", 0, 0),
        ("a2a3", 4_000_000, 1),
        ("a2a3 a7a5", 4_000_009, 1),
    ]);
    let b = ledger_of(&[("a2a3", 8_000_000, 2), ("c2c3", 0, 0)]);
    let (u, _) = union(&[a.clone(), b.clone()]);
    // Idempotence: union(u, inputs) = u (record-set equality).
    let (u2, st2) = union(&[u.clone(), a.clone(), b.clone()]);
    assert_eq!(entries(&u2), entries(&u));
    assert_eq!(st2.union_records, 4);
    assert_eq!(st2.new_paths, [0, 0, 0]);
    assert_eq!(st2.sole_pass_upgrades, [0, 0, 0]);
    assert_coverage(&u2, &[u.clone(), a.clone(), b.clone()]);
    // N = 1 normalizes: the record set passes through unchanged.
    let (one, st) = union(std::slice::from_ref(&a));
    assert_eq!(entries(&one), entries(&a));
    assert_eq!(st.union_records, 3);
    assert_eq!(st.new_paths, [3]);
    // Empty input set: the empty ledger (the CLI requires N ≥ 1; the
    // function is total).
    let (empty, st) = union(&[]);
    assert_eq!(empty.len(), 0);
    assert_eq!(st.union_records, 0);
}

#[test]
fn union_ledger_save_roundtrip_is_deterministic() {
    let dir = std::env::temp_dir().join(format!("proofdb_union_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = ledger_of(&[("", 0, 0), ("a2a3", 4_000_000, 1), ("a2a3 a7a5", 0, 0)]);
    let b = ledger_of(&[("a2a3", 8_000_000, 2)]);
    let (u, _) = union(&[a, b]);
    u.save(&dir.join("u1.json")).unwrap();
    // A rebuild from the written bytes + a second save are byte-identical
    // (the gate's determinism check, at fixture scale).
    let reloaded = Ledger::load(&dir.join("u1.json")).unwrap();
    reloaded.save(&dir.join("u2.json")).unwrap();
    let b1 = std::fs::read(dir.join("u1.json")).unwrap();
    let b2 = std::fs::read(dir.join("u2.json")).unwrap();
    assert_eq!(b1, b2);
    // The union's record: the pass-2 record wins, "a2a3 a7a5" stays fresh.
    let got: Vec<_> = reloaded
        .iter()
        .map(|(k, e)| (k.clone(), e.passes_failed))
        .collect();
    assert_eq!(
        got,
        vec![
            ("".to_string(), 0),
            ("a2a3".to_string(), 2),
            ("a2a3 a7a5".to_string(), 0),
        ]
    );
    std::fs::remove_dir_all(&dir).ok();
}

// --- plan8 D1: the and-close completion-gradient policy (and_close.rs) ---

use proofdb::and_close::driver::{
    AndCloseOptions, apply_budget_filter, on_censored, run_and_close_batch,
};
use proofdb::and_close::{AndCloseCensus, AndCloseOrder, build_sequence, ladder_budget};
use proofdb::pns::{Pns, PnsConfig};
use proofdb::policy::Policy;
use proofdb::session::Session;

/// A `DbContent` from hand-written rows (path, ply, outcome) — paths must
/// replay legally (the and-close extraction movegens at every active row).
/// Same shape as `pns::tests::crafted`, which is not reachable from here
/// (private `mod tests`).
fn ac_crafted(rows: &[(&str, usize, Option<Outcome>)]) -> proofdb::db::DbContent {
    let rows = rows
        .iter()
        .map(|&(p, ply, o)| proofdb::db::DbRow {
            path: if p.is_empty() {
                Vec::new()
            } else {
                p.split(' ').map(str::to_string).collect()
            },
            ply,
            outcome: o,
            depth_bound: o.map(|_| 1),
        })
        .collect();
    proofdb::db::DbContent {
        root_fen: Position::STARTPOS_FEN.to_string(),
        rows,
    }
}

fn ac_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("proofdb_ac_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn ac_build(db: &proofdb::db::DbContent, ledger: Ledger, dir: &std::path::Path, base: u64) -> Pns {
    Pns::build(
        db,
        ledger,
        dir.join("ledger.json"),
        base,
        PnsConfig::default(),
        0,
    )
    .unwrap()
}

fn ac_paths(jobs: &[proofdb::harvest::Job]) -> Vec<String> {
    jobs.iter().map(|j| j.path.join(" ")).collect()
}

#[test]
fn and_close_completion_order_ties_and_budgets() {
    let dir = ac_dir("order");
    // Root (18 missing: a2a3/b1c3 stored), then a tie at 20 between the two
    // stored rows (both ply 1, 20 unvisited replies each).
    let db = ac_crafted(&[("", 0, None), ("a2a3", 1, None), ("b1c3", 1, None)]);
    let mut sel = ac_build(&db, Ledger::default(), &dir, 1_000_000);
    let built = build_sequence(&mut sel, AndCloseOrder::Completion, 1_000_000).unwrap();
    let c = &built.census;
    assert_eq!(c.rows_open, 3);
    assert_eq!(c.active_rows, 3);
    assert_eq!(c.excluded, [0, 0, 0]);
    assert_eq!(c.replies(), 58);
    assert_eq!(c.replies_fresh, 58);
    assert_eq!(c.replies_censored, 0);
    // Gradient: root first (18), then the (missing, ply, path) tie broken
    // by path: a2a3 before b1c3.
    assert_eq!(
        built.gradient,
        vec![
            ("".to_string(), 18),
            ("a2a3".to_string(), 20),
            ("b1c3".to_string(), 20)
        ]
    );
    let paths = ac_paths(&built.jobs);
    // Root's 18 replies first, path-lex (a2a3/b1c3 are stored rows).
    assert!(paths[..18].iter().all(|p| !p.contains(' ')));
    let mut sorted = paths[..18].to_vec();
    sorted.sort();
    assert_eq!(paths[..18], sorted[..]);
    assert!(!paths[..18].contains(&"a2a3".to_string()));
    assert!(!paths[..18].contains(&"b1c3".to_string()));
    // Then a2a3's 20 replies (path-lex), then b1c3's.
    assert!(paths[18..38].iter().all(|p| p.starts_with("a2a3 ")));
    assert!(paths[38..].iter().all(|p| p.starts_with("b1c3 ")));
    let mut sorted = paths[18..38].to_vec();
    sorted.sort();
    assert_eq!(paths[18..38], sorted[..]);
    // Fresh ledger: every budget = the base.
    assert!(built.jobs.iter().all(|j| j.budget == 1_000_000));
    // Jobs are C3-class children (plan8 §2: the standard shard pipeline).
    assert!(built.jobs.iter().all(|j| j.ply == j.path.len()));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn and_close_ladder_budgets_and_strict_growth() {
    // The pure ladder: 2^(k-1) × base.
    assert_eq!(ladder_budget(0, 0, 4_000_000), 4_000_000);
    assert_eq!(ladder_budget(1, 0, 4_000_000), 8_000_000);
    assert_eq!(ladder_budget(2, 0, 4_000_000), 16_000_000);
    assert_eq!(ladder_budget(3, 0, 1_000_000), 8_000_000);
    // The strict-growth floor (plan9 §2): the floor doubles accumulated
    // work instead of matching it — a revisit at budget ≤ work_done is a
    // byte-identical deterministic repeat and must be impossible.
    assert_eq!(ladder_budget(1, 100_000_000, 4_000_000), 200_000_000);
    assert_eq!(ladder_budget(2, 20_000_000, 4_000_000), 40_000_000);
    // The equal-work regression case (plan9 §2): pass 1 at work =
    // 2^(k−1) × base must yield 2 × work, not work (the plan8 monotone
    // rule revisited a 1B-censored defense at exactly 1B — pure waste).
    assert_eq!(ladder_budget(1, 8_000_000, 4_000_000), 16_000_000);
    // The g1f3 deep-probe case with the standing base 4M: ≈ 2B (the
    // pre-registered plan9 rung, reached without a base change).
    assert_eq!(ladder_budget(1, 1_000_000_060, 4_000_000), 2_000_000_120);
    // Fresh (pass 0, work 0) stays at the base.
    assert_eq!(ladder_budget(0, 0, 4_000_000), 4_000_000);
    // Saturation guard.
    assert_eq!(ladder_budget(63, 0, u64::MAX), u64::MAX);

    // Integration: sequence budgets read from the ledger.
    let dir = ac_dir("ladder");
    let db = ac_crafted(&[("", 0, None)]);
    let ledger = ledger_of(&[
        ("c2c4", 100_000_000, 3),
        ("d2d4", 4_000_000, 1),
        ("e2e4", 0, 0),
    ]);
    let mut sel = ac_build(&db, ledger, &dir, 1_000_000);
    let built = build_sequence(&mut sel, AndCloseOrder::Completion, 1_000_000).unwrap();
    assert_eq!(built.census.replies(), 20);
    assert_eq!(built.census.replies_fresh, 18); // e2e4 (pass-0 record) + 17 bare
    assert_eq!(built.census.replies_censored, 2);
    let budget = |p: &str| {
        built
            .jobs
            .iter()
            .find(|j| j.path.join(" ") == p)
            .unwrap()
            .budget
    };
    // Strict growth: the 100M-deep-censored reply revisits at 2 × work,
    // never at ≤ work (the plan9 §2 floor); a pass-1 4M reply revisits at
    // max(8M rung, 8M floor) = 8M — plan8's behavior where the old floor
    // was strictly exceeded.
    assert_eq!(budget("c2c4"), 200_000_000);
    assert_eq!(budget("d2d4"), 8_000_000); // max(8M rung, 2 × 4M work)
    assert_eq!(budget("e2e4"), 1_000_000); // pass-0 record: fresh budget
    assert_eq!(budget("a2a3"), 1_000_000);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn and_close_fresh_order_excludes_ledger_censored() {
    let dir = ac_dir("fresh");
    let db = ac_crafted(&[("", 0, None)]);
    let ledger = ledger_of(&[
        ("c2c4", 0, 0),         // pass-0 record: still fresh (§2)
        ("d2d4", 4_000_000, 1), // censored once: excluded
    ]);
    let mut sel = ac_build(&db, ledger, &dir, 1_000_000);
    let built = build_sequence(&mut sel, AndCloseOrder::Fresh, 1_000_000).unwrap();
    assert_eq!(built.census.replies(), 20);
    assert_eq!(built.census.replies_fresh, 19);
    assert_eq!(built.census.replies_censored, 1);
    let paths = ac_paths(&built.jobs);
    assert_eq!(paths.len(), 19);
    assert!(!paths.iter().any(|p| p == "d2d4"));
    // (reply ply asc, path asc): all ply 1 here, so path-lex overall.
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted);
    // The completion order over the same state still has all 20.
    let ledger = ledger_of(&[("d2d4", 4_000_000, 1)]);
    let mut sel = ac_build(&db, ledger, &dir, 1_000_000);
    let built = build_sequence(&mut sel, AndCloseOrder::Completion, 1_000_000).unwrap();
    assert_eq!(built.jobs.len(), 20);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn and_close_exclusions_decisions_9_10() {
    let dir = ac_dir("excl");
    // Decision 9 (proven ancestor): e7e6 sits behind the proven f2f3, so
    // its replies are excluded; the root's replies (minus f2f3) remain.
    let db = ac_crafted(&[
        ("", 0, None),
        ("f2f3", 1, Some(Outcome::Win)),
        ("f2f3 e7e6", 2, None),
    ]);
    let mut sel = ac_build(&db, Ledger::default(), &dir, 1_000_000);
    let built = build_sequence(&mut sel, AndCloseOrder::Completion, 1_000_000).unwrap();
    assert_eq!(built.census.rows_open, 2);
    assert_eq!(built.census.active_rows, 1);
    assert_eq!(built.census.excluded, [1, 0, 0]);
    let paths = ac_paths(&built.jobs);
    assert_eq!(paths.len(), 19);
    assert!(paths.iter().all(|p| !p.starts_with("f2f3 ")));

    // Decision 10 (implied-decided): f2f3 with all 20 replies proven
    // root-player wins (even ply + win) is an implied AND-loss; its
    // (zero) missing replies drop out and the row is excluded.
    let pos_after_f3 = {
        let mut pos = Position::from_fen(Position::STARTPOS_FEN).unwrap();
        let mv = atomic_solver::notation::uci_to_move("f2f3", &pos).unwrap();
        pos.do_move(mv);
        pos
    };
    let mut rows: Vec<(&str, usize, Option<Outcome>)> = vec![("", 0, None), ("f2f3", 1, None)];
    for mv in pos_after_f3.legal_moves_vec() {
        let uci = atomic_solver::notation::move_to_uci(mv).leak() as &str;
        rows.push((format!("f2f3 {uci}").leak(), 2, Some(Outcome::Win)));
    }
    let db = ac_crafted(&rows);
    let mut sel = ac_build(&db, Ledger::default(), &dir, 1_000_000);
    let built = build_sequence(&mut sel, AndCloseOrder::Completion, 1_000_000).unwrap();
    assert_eq!(built.census.rows_open, 2);
    // The exclusion cascades (decision 10): f2f3 is an implied AND-loss and
    // the root (OR) is implied-win through it — both rows drop out, no jobs.
    assert_eq!(built.census.active_rows, 0);
    assert_eq!(built.census.excluded, [0, 2, 0], "implied-win cascade");
    assert_eq!(built.jobs.len(), 0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn and_close_censor_hook_bumps_only() {
    let dir = ac_dir("hook");
    let db = ac_crafted(&[("", 0, None)]);
    let mut sel = ac_build(&db, Ledger::default(), &dir, 1_000_000);
    let nodes_before = sel.nodes.len();
    let mut ledger = std::mem::take(&mut sel.ledger);
    on_censored(&mut ledger, &dir.join("ledger.json"), "a2a3", 4_000_000).unwrap();
    // Bump-only: exactly the censored reply's record — no child exposure,
    // no tree growth (the decision-11 deviation, plan8 §2).
    assert_eq!(ledger.len(), 1);
    assert_eq!(
        ledger.get("a2a3"),
        Some(&LedgerEntry {
            work_done: 4_000_000,
            passes_failed: 1
        })
    );
    assert_eq!(sel.nodes.len(), nodes_before);
    assert!(ledger.get("a2a3 a7a5").is_none());
    // A second censor compounds (pass 2, work added).
    on_censored(&mut ledger, &dir.join("ledger.json"), "a2a3", 8_000_000).unwrap();
    assert_eq!(
        ledger.get("a2a3"),
        Some(&LedgerEntry {
            work_done: 12_000_000,
            passes_failed: 2
        })
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn and_close_driver_censored_jobs_grow_ledger_by_censor_count() {
    let dir = ac_dir("driver");
    let (db_path, digest) = proofdb::fixture::fixture_db(&dir);
    let manifest = read_manifest(&dir.join("manifest.json")).unwrap();
    let shard_dir = dir.join("shards");
    std::fs::create_dir_all(&shard_dir).unwrap();
    let db = proofdb::db::load_db_rows(&db_path, &digest).unwrap();
    let ledger_path = dir.join("ledger.json");
    let mut sel = ac_build(&db, Ledger::default(), &dir, 1_000);
    let built = build_sequence(&mut sel, AndCloseOrder::Completion, 1_000).unwrap();
    assert!(
        built.jobs.len() > 5,
        "fixture has plenty of missing replies"
    );
    let start_records = sel.ledger.len();

    let run = |ledger_path: &std::path::Path| {
        let mut session =
            Session::new(4, dir.join("shards"), manifest.entries.clone(), "and-close");
        let opts = AndCloseOptions {
            max_total_evals: 0,
            max_jobs: 5,
            max_runtime: 0,
            stop_file: std::path::PathBuf::new(),
            max_budget: 0,
        };
        let mut sel = ac_build(&db, Ledger::default(), &dir, 1_000);
        let built = build_sequence(&mut sel, AndCloseOrder::Completion, 1_000).unwrap();
        let mut ledger = std::mem::take(&mut sel.ledger);
        let summary =
            run_and_close_batch(&mut session, &built.jobs, &mut ledger, ledger_path, &opts)
                .unwrap();
        (summary, ledger)
    };
    let (summary, ledger) = run(&ledger_path);
    // Tiny budgets on quiet fixture positions: every job censors.
    assert_eq!(summary.jobs, 5);
    assert_eq!(summary.stop_reason, "max-jobs");
    assert_eq!(summary.decisive, 0);
    assert_eq!(summary.censored, 5);
    // The H1 no-exposure signature: ledger growth == censor count.
    assert_eq!(ledger.len(), start_records + 5);
    // Determinism: a fresh session over the same inputs reproduces the
    // post-run ledger byte-for-byte.
    let ledger_path2 = dir.join("ledger2.json");
    let (summary2, ledger2) = run(&ledger_path2);
    assert_eq!(summary2.jobs, summary.jobs);
    assert_eq!(entries(&ledger), entries(&ledger2));
    let b1 = std::fs::read(&ledger_path).unwrap();
    let b2 = std::fs::read(&ledger_path2).unwrap();
    assert_eq!(b1, b2, "post-run ledger bytes identical");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn and_close_budget_filter_plan10_d1() {
    let dir = ac_dir("filter");
    // Fresh (pass-0, budget = base) and ledger-censored replies at pass 1
    // (budget = 2 × work) — a filter cap between the two classes must keep
    // exactly the fresh ones.
    let db = ac_crafted(&[("", 0, None)]);
    let mut ledger = Ledger::default();
    ledger.bump("a2a3", 1_000_000); // pass 1, work 1M → budget 2M
    let mut sel = ac_build(&db, ledger, &dir, 1_000_000);
    let built = build_sequence(&mut sel, AndCloseOrder::Completion, 1_000_000).unwrap();
    assert!(built.jobs.iter().any(|j| j.budget == 1_000_000));
    assert!(built.jobs.iter().any(|j| j.budget == 2_000_000));
    let total = built.jobs.len();

    // Unlimited (0): no-op, empty echo.
    let mut jobs = built.jobs.clone();
    assert_eq!(apply_budget_filter(&mut jobs, 0), "");
    assert_eq!(jobs.len(), total);

    // Cap between the classes: fresh kept, bumped dropped, echo exact.
    let mut jobs = built.jobs.clone();
    let line = apply_budget_filter(&mut jobs, 1_000_000);
    assert_eq!(
        line,
        format!(
            "and-close-filter: max-budget 1000000, jobs {} of {total}",
            jobs.len()
        )
    );
    assert!(jobs.iter().all(|j| j.budget == 1_000_000));
    assert_eq!(jobs.len(), total - 1);
    assert!(jobs.iter().all(|j| j.path != vec!["a2a3"]));

    // Cap above every budget: nothing dropped.
    let mut jobs = built.jobs.clone();
    let line = apply_budget_filter(&mut jobs, 1_000_000_000);
    assert_eq!(
        line,
        format!("and-close-filter: max-budget 1000000000, jobs {total} of {total}")
    );
    assert_eq!(jobs.len(), total);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn and_close_census_lines_and_parsing() {
    let c = AndCloseCensus {
        rows_open: 75,
        active_rows: 49,
        excluded: [26, 0, 0],
        replies_fresh: 1041,
        replies_censored: 101,
    };
    assert_eq!(c.replies(), 1142);
    let line = c.describe(AndCloseOrder::Completion, 1_000_000_000);
    assert_eq!(
        line,
        "and-close: active rows 49, replies 1142 (fresh 1041, ledger-censored 101); \
         order completion; base budget 1000000000"
    );
    let line = c.describe(AndCloseOrder::Fresh, 4_000_000);
    assert!(line.contains("order fresh; base budget 4000000"));
    let excl = c.describe_exclusions();
    assert_eq!(
        excl,
        "and-close-excluded: open rows 75 (active 49); proven-ancestor 26, \
         implied-win 0, implied-loss 0"
    );
    let grad = proofdb::and_close::describe_gradient(&[
        ("".to_string(), 13),
        ("g1f3".to_string(), 3),
        ("e2e3".to_string(), 7),
    ]);
    assert_eq!(grad, "and-close-gradient: root:13 g1f3:3 e2e3:7");

    // Parsing: policy name, order round-trip, unknown rejected.
    assert_eq!(Policy::parse("and-close"), Ok(Policy::AndClose));
    assert_eq!(Policy::AndClose.as_str(), "and-close");
    assert_eq!(
        AndCloseOrder::parse("completion"),
        Ok(AndCloseOrder::Completion)
    );
    assert_eq!(AndCloseOrder::parse("fresh"), Ok(AndCloseOrder::Fresh));
    assert!(AndCloseOrder::parse("deep").is_err());
    assert!(Policy::parse("andclose").is_err());
    // The legacy frontier sequence builder has no and-close jobs (the CLI
    // branch builds them over the classified PNS tree instead).
    let dir = ac_dir("jfp");
    let (db_path, digest) = proofdb::fixture::fixture_db(&dir);
    let f =
        proofdb::frontier::extract_frontier(&db_path, &digest, &std::collections::HashSet::new())
            .unwrap();
    assert!(proofdb::policy::jobs_for_policy(Policy::AndClose, &f).is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

// --- plan12 D3: the derived-DB gate as a repeatable test ---

/// The standing shard set (committed under `docs/plans/proofdb/shards/`)
/// must rebuild the standing DB byte-identically from a clean checkout —
/// the durable-layer gate (initiative constraint 3), pinned here against
/// drift. Digest and node counts are the plan12-audit pinned values
/// (report12 §1.1); a shard-set change is expected to turn this red.
#[test]
fn standing_layer_rebuilds_byte_identical() {
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let shards = crate_root.join("docs/plans/proofdb/shards");
    let manifest_path = shards.join("manifest.json");
    assert!(
        manifest_path.is_file(),
        "standing shard set not found at {} — this test runs inside the\n\
         atomic_solver checkout (it pins the committed durable layer);\n\
         see docs/proofdb_pipeline.md",
        manifest_path.display()
    );
    let manifest = proofdb::read_manifest(&manifest_path).unwrap();
    assert_eq!(manifest.entries.len(), 262, "standing manifest entry count");

    // The proofdb_merge pipeline, faithfully: per shard — validate=='ok',
    // parse + replay-validate, outcome cross-check, startpos path replay,
    // graft + overlay; then finalize, per-graft skeleton re-validation, DB.
    let mut tree = proofdb::merge::PathTree::new();
    let mut rows = Vec::new();
    let mut graft_roots = Vec::new();
    for entry in &manifest.entries {
        assert_eq!(entry.validate, "ok", "only 'ok' enters a merge");
        let bytes = std::fs::read(shards.join(&entry.file))
            .unwrap_or_else(|e| panic!("shard {}: {e}", entry.tag));
        let shard = atomic_solver::proof_tree::ProofTree::from_bin(&mut bytes.as_slice())
            .unwrap_or_else(|e| panic!("shard {}: {e}", entry.tag));
        atomic_solver::proof_tree::validate_proof_tree(&shard)
            .unwrap_or_else(|d| panic!("shard {}: {d:?}", entry.tag));
        assert_eq!(
            shard.nodes[0].outcome,
            Some(entry.outcome),
            "shard {}",
            entry.tag
        );
        let mut pos = Position::from_fen(Position::STARTPOS_FEN).unwrap();
        let moves: Vec<atomic_movegen::types::Move> = entry
            .moves
            .iter()
            .map(|u| {
                let mv = atomic_solver::notation::uci_to_move(u, &pos)
                    .unwrap_or_else(|| panic!("shard {}: illegal {u}", entry.tag));
                pos.do_move(mv);
                mv
            })
            .collect();
        let graft = tree.graft_path(&moves, &entry.tag);
        tree.overlay_subtree(graft, &shard, &entry.tag)
            .unwrap_or_else(|e| panic!("shard {}: {e}", entry.tag));
        graft_roots.push((graft, entry.fen.clone()));
        rows.push(proofdb::schema::ShardRow {
            tag: entry.tag.clone(),
            file: entry.file.clone(),
            sha256: proofdb::digest_hex(&bytes),
            root_fen: shard.root_fen,
            path: entry.moves.join(" "),
            outcome: entry.outcome,
            depth_bound: shard.nodes[0].depth,
            n_nodes: shard.nodes.len(),
        });
    }
    tree.finalize().unwrap();
    for (root_id, fen) in &graft_roots {
        let skeleton = tree.reassemble_skeleton(*root_id, fen).unwrap();
        assert!(
            atomic_solver::proof_tree::validate_proof_tree(&skeleton).is_ok(),
            "skeleton re-validation failed at graft {:?}",
            tree.path_uci(*root_id)
        );
    }

    let dir = std::env::temp_dir().join(format!("proofdb_rebuild_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("rebuilt.db");
    proofdb::schema::write_db(
        &db_path,
        Position::STARTPOS_FEN,
        &manifest.sha256_hex,
        &rows,
        &tree,
    )
    .unwrap();

    // The pinned standing DB: byte-identical rebuild + node census.
    assert_eq!(
        proofdb::digest_hex(&std::fs::read(&db_path).unwrap()),
        "0d929f4c3c62e4bbb87e9b043b582716297b23e2f7332a36e82763b1486070b8",
        "rebuilt DB digest (the standing data/proofdb.db)"
    );
    assert_eq!(tree.nodes.len(), 55_703, "merged node count");
    assert_eq!(
        tree.nodes.iter().filter(|n| n.outcome.is_some()).count(),
        55_628,
        "proven node count"
    );
    std::fs::remove_dir_all(&dir).ok();
}

// --- plan12 D1: the stale-DB guard is length-safe on a malformed DB ---

/// A foreign/corrupted DB with a short (or non-ASCII) `built_from` must
/// produce the clean stale-DB abort (an `Err` naming `built_from`), never a
/// slice panic (the pre-plan12 behavior: exit 134, core dump).
#[test]
fn stale_db_guard_is_length_safe_on_malformed_built_from() {
    let dir = std::env::temp_dir().join(format!("proofdb_g1_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (db_path, digest) = proofdb::fixture::fixture_db(&dir);
    for bogus in ["deadbeef", "", "méé-méé-méé-méé-beyond-16-bytes"] {
        let tampered = dir.join("tampered.db");
        std::fs::copy(&db_path, &tampered).unwrap();
        let conn = rusqlite::Connection::open(&tampered).unwrap();
        conn.execute("UPDATE meta SET value=?1 WHERE key='built_from'", [bogus])
            .unwrap();
        let err = proofdb::db::load_db_rows(&tampered, &digest).unwrap_err();
        assert!(
            err.contains("built_from"),
            "message names the meta row: {err}"
        );
        assert!(
            err.contains("stale DB"),
            "stale-DB abort, not a panic: {err}"
        );
        assert!(err.contains(bogus), "value printed in full: {err}");
        std::fs::remove_file(&tampered).ok();
    }
    std::fs::remove_dir_all(&dir).ok();
}

// --- plan14: the CLI defaults are centralized under data/proofdb/ ---

/// Every generated-proofdb default must live wholly under the gitignored
/// working layer `data/proofdb/` (plan14): the shard layer co-located under
/// `data/proofdb/shards/`, and every default path directory-qualified (a
/// bare file name in the cwd is exactly what this item removed).
#[test]
fn cli_defaults_are_centralized_under_data_proofdb() {
    use proofdb::{
        DEFAULT_DB, DEFAULT_FLIP_OUT, DEFAULT_LEDGER, DEFAULT_MANIFEST, DEFAULT_SHARD_DIR,
    };

    assert_eq!(DEFAULT_DB, "data/proofdb/proofdb.db");
    assert_eq!(DEFAULT_MANIFEST, "data/proofdb/shards/manifest.json");
    assert_eq!(DEFAULT_SHARD_DIR, "data/proofdb/shards");
    assert_eq!(DEFAULT_LEDGER, "data/proofdb/work.json");
    assert_eq!(DEFAULT_FLIP_OUT, "data/proofdb/flip.json");

    for path in [
        DEFAULT_DB,
        DEFAULT_MANIFEST,
        DEFAULT_SHARD_DIR,
        DEFAULT_LEDGER,
        DEFAULT_FLIP_OUT,
    ] {
        let p = std::path::Path::new(path);
        assert!(
            p.starts_with("data/proofdb"),
            "default {path} outside data/proofdb/"
        );
        assert!(
            p.parent().is_some_and(|d| !d.as_os_str().is_empty()),
            "default {path} is not directory-qualified"
        );
    }
}
