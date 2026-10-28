//! `proofdb_flip` — the implied-flip analysis (read-only): over a grown DB,
//! derive what every open row's stored facts already imply, converge the
//! claim map on values *and* metadata, and recompute every claim with an
//! independent recursive verifier. The semantics — the OR-witness /
//! AND-complete asymmetry, the coverage and bound-mode vocabulary, and why a
//! flip is a derived claim rather than a contradiction — are normative in
//! `examples/proofdb/flip/mod.rs`; the manual states them for operators in
//! `docs/proofdb_pipeline.md` §7.3.
//!
//! Output: one stdout line
//! (`flip_analysis: open_rows N flips F verified V root <Null|l> (fixpoint R
//! rounds; or-witness O, and-complete A, exact E)`) plus the JSON report at
//! `--out` (default `data/proofdb/flip.json`). Exit 0 normally — including
//! with flips — and 1 on a usage error, a DB load/gate failure, or any
//! verification defect. This tool materializes nothing: flips stay derived,
//! the DB ships the raw tree only. The `--shard-dir` gate is the
//! unmerged-shard check (plan22).

use std::path::PathBuf;

mod proofdb;

use proofdb::db::{check_unmerged_shards, load_db_rows};
use proofdb::flip::{Analysis, BoundMode, Coverage};
use proofdb::schema::read_shard_rows;
use proofdb::{DEFAULT_DB, DEFAULT_FLIP_OUT, DEFAULT_SHARD_DIR, create_parent_dir};

fn fail(msg: &str) -> ! {
    eprintln!("proofdb_flip: {msg}");
    std::process::exit(1);
}

fn usage() -> ! {
    eprintln!(
        "usage: proofdb_flip [--db <grown.db>] [--shard-dir <dir>] \
         [--out <flip_analysis.json>]  \
         (defaults: --db data/proofdb/proofdb.db \
         --shard-dir data/proofdb/shards --out data/proofdb/flip.json)"
    );
    std::process::exit(1);
}

fn main() {
    let mut db = PathBuf::from(DEFAULT_DB);
    let mut shard_dir = PathBuf::from(DEFAULT_SHARD_DIR);
    let mut out = PathBuf::from(DEFAULT_FLIP_OUT);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--db" => db = it.next().unwrap_or_else(|| usage()).into(),
            "--shard-dir" => shard_dir = it.next().unwrap_or_else(|| usage()).into(),
            "--out" => out = it.next().unwrap_or_else(|| usage()).into(),
            _ => usage(),
        }
    }
    // The unmerged-shard gate (plan22 D6 — replaces the retired built_from
    // manifest-digest check): an unmerged sidecar refuses the analysis.
    let ro = rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap_or_else(|e| fail(&format!("cannot open DB {}: {e}", db.display())));
    let shard_rows = read_shard_rows(&ro).unwrap_or_else(|e| fail(&e));
    if let Err(e) = check_unmerged_shards(&shard_dir, &shard_rows) {
        fail(&e);
    }
    drop(ro);
    let content = load_db_rows(&db).unwrap_or_else(|e| fail(&e));
    let mut an = Analysis::new(&content.rows, &content.root_fen).unwrap_or_else(|e| fail(&e));
    let rounds = an.solve().unwrap_or_else(|e| fail(&e));
    let verified = an
        .verify_all()
        .unwrap_or_else(|e| fail(&format!("FLIP VERIFICATION FAILED: {e}")));
    let root = an.root().map(|c| (c.value.root_label(0), c.value.bound));
    if let Some((label, b)) = root {
        eprintln!("proofdb_flip: ROOT implied {label} (bound {b})");
    }
    // `Null` for an undecided root (the pinned grammar), else the label.
    let root_word = root.map_or("Null", |(label, _)| label);
    let (mut or_witness, mut and_complete, mut exact) = (0usize, 0usize, 0usize);
    let mut claims: Vec<serde_json::Value> = Vec::with_capacity(an.order().len());
    for (path, ply, round) in an.order() {
        let c = an.claim(path).expect("every discovered flip has a claim");
        or_witness += usize::from(c.coverage == Coverage::OrWitness);
        and_complete += usize::from(c.coverage == Coverage::AndComplete);
        exact += usize::from(c.bound_mode == BoundMode::Exact);
        claims.push(serde_json::json!({
            "path": path, "ply": ply,
            "implied": c.value.root_label(*ply),
            "side_to_move": if c.value.stm_win { "win" } else { "loss" },
            "bound": c.value.bound, "fixpoint_round": round,
            "coverage": c.coverage.as_str(),
            "bound_mode": c.bound_mode.as_str(),
            "loss_witnesses": c.loss_witnesses,
            "unresolved_replies": c.unresolved_replies,
        }));
    }
    let json = serde_json::json!({
        "open_rows": an.open_rows(),
        "flips": claims,
        "flips_total": an.order().len(),
        "flips_verified_independently": verified,
        "flips_or_witness": or_witness,
        "flips_and_complete": and_complete,
        "flips_exact": exact,
        "root_implied": root.map(|(l, _)| l),
        "root_bound": root.map(|(_, b)| b),
    });
    create_parent_dir(&out).unwrap_or_else(|e| fail(&e));
    std::fs::write(&out, serde_json::to_string_pretty(&json).unwrap() + "\n")
        .unwrap_or_else(|e| fail(&format!("cannot write {}: {e}", out.display())));
    println!(
        "flip_analysis: open_rows {} flips {} verified {} root {} \
         (fixpoint {} rounds; or-witness {}, and-complete {}, exact {})",
        json["open_rows"],
        an.order().len(),
        verified,
        root_word,
        rounds,
        or_witness,
        and_complete,
        exact
    );
}
