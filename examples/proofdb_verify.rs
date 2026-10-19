//! `proofdb_verify` — the whole-DB replay validator (plan22 D7): reads the
//! DB alone (no shards), replays every node path from the startpos, and
//! checks the structural invariants (proven AND completeness, proven OR
//! bound arithmetic, terminal claims, field domains, provenance,
//! `node_count`). Streaming DFS with do/undo — O(depth) RAM. Since the
//! fold made the DB the single source of truth, this replaces the retired
//! byte-identical-rebuild contract as the layer's audit gate.
//!
//! `--backup <path>` runs `VACUUM INTO` — the documented offsite-backup
//! duty (fold → verify → backup → delete, D8); it runs only on a green
//! verify (a defective DB is not backup material) and refuses an existing
//! target.
//!
//! Output: `verify: ok — nodes N (proven P, open O)` or `verify: FAILED k`
//! plus up to `--max-report` defect lines; exit 0 green, 2 on defects, 1 on
//! usage/IO. Defaults wholly under `data/proofdb/` (plan14).

use std::path::PathBuf;

use rusqlite::OpenFlags;

mod proofdb;

use proofdb::verify::verify_db;
use proofdb::{DEFAULT_DB, create_parent_dir};

fn fail(msg: &str) -> ! {
    eprintln!("proofdb_verify: {msg}");
    std::process::exit(1);
}

fn usage() -> ! {
    eprintln!(
        "usage: proofdb_verify [--db <proofdb.db>] [--max-report <n>] \
         [--backup <path>]  \
         (defaults: --db data/proofdb/proofdb.db --max-report 20; \
         --backup VACUUM INTOs the DB to <path> on a green verify — the \
         offsite-backup duty before shard deletion)"
    );
    std::process::exit(1);
}

struct Args {
    db: PathBuf,
    max_report: usize,
    backup: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut a = Args {
        db: PathBuf::from(DEFAULT_DB),
        max_report: 20,
        backup: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut next = || it.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "--db" => a.db = PathBuf::from(next()),
            "--max-report" => a.max_report = next().parse().unwrap_or_else(|_| usage()),
            "--backup" => a.backup = Some(PathBuf::from(next())),
            _ => usage(),
        }
    }
    a
}

fn main() {
    let args = parse_args();
    let conn = rusqlite::Connection::open_with_flags(&args.db, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap_or_else(|e| fail(&format!("cannot open DB {}: {e}", args.db.display())));
    proofdb::verify::check_meta(&conn).unwrap_or_else(|e| fail(&e));
    match verify_db(&conn, args.max_report) {
        Err(e) => fail(&e),
        Ok(proofdb::verify::Verdict::Ok {
            nodes,
            proven,
            open,
        }) => {
            println!("verify: ok — nodes {nodes} (proven {proven}, open {open})");
            if let Some(dest) = &args.backup {
                backup(&conn, dest);
            }
        }
        Ok(proofdb::verify::Verdict::Failed {
            total,
            reported,
            nodes,
        }) => {
            println!("verify: FAILED {total} (nodes {nodes})");
            for d in &reported {
                println!("defect: {d}");
            }
            if reported.len() < total {
                println!(
                    "defect: … {} more not shown (--max-report)",
                    total - reported.len()
                );
            }
            if args.backup.is_some() {
                eprintln!(
                    "proofdb_verify: --backup skipped (the DB has defects — \
                     defective state is not backup material)"
                );
            }
            std::process::exit(2);
        }
    }
}

/// The offsite-backup duty (D8): `VACUUM INTO <dest>` — a compact, readable
/// copy of the whole DB.
fn backup(conn: &rusqlite::Connection, dest: &std::path::Path) {
    if dest.exists() {
        fail(&format!(
            "backup target {} exists — refusing to overwrite",
            dest.display()
        ));
    }
    create_parent_dir(dest).unwrap_or_else(|e| fail(&e));
    let literal = dest
        .to_str()
        .unwrap_or_else(|| fail("backup path is not UTF-8"));
    let literal = literal.replace('\'', "''");
    conn.execute(&format!("VACUUM INTO '{literal}'"), [])
        .unwrap_or_else(|e| fail(&format!("VACUUM INTO {}: {e}", dest.display())));
    println!("backup: {} written", dest.display());
}
