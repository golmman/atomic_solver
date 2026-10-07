//! `proofdb_ledger_union` — N-way union of proofdb work ledgers (plan7 §2):
//! the standing-state merge mechanism. Batch 3's arm 2 merges the standing
//! working ledger (default `data/proofdb/work.json`, plan14) with the
//! committed plan6 arm snapshots to recover the censor knowledge the
//! standing-ledger advance dropped (report6 finding 4); item 6's per-worker
//! ledger merge builds on the same primitive.
//!
//! One record per known-open frontier path; per path the union keeps the
//! record with the highest `passes_failed`, ties broken by the higher
//! `work_done` (the pre-registered §2 rule — selection state only, so a
//! union cannot create, destroy, or contradict a fact; the shard/manifest
//! layer is untouched).
//!
//! Usage:
//! `proofdb_ledger_union --out <ledger.json> [--expect <file>=<sha256>]... <input.json>...`
//!
//! - N ≥ 1 inputs; N = 1 normalizes/validates only (a deterministic
//!   rewrite through [`Ledger::save`]).
//! - `--expect` pins an input's sha256 digest: base compatibility is the
//!   operator's contract (the ledger format carries no base stamp,
//!   plan7 §2.4), so every pinned digest is verified before the merge.
//! - Output: the merged ledger (deterministic sorted bytes) plus a
//!   per-input census on stderr (records / new paths / sole pass
//!   upgrades).

mod proofdb;

use proofdb::digest_hex;
use proofdb::ledger::Ledger;
use proofdb::ledger_union::union;

fn fail(msg: &str) -> ! {
    eprintln!("proofdb_ledger_union: {msg}");
    std::process::exit(1);
}

fn usage() -> ! {
    eprintln!(
        "usage: proofdb_ledger_union --out <ledger.json> \
         [--expect <file>=<sha256>]... <input.json>..."
    );
    std::process::exit(1);
}

struct Args {
    out: std::path::PathBuf,
    inputs: Vec<std::path::PathBuf>,
    expect: Vec<(std::path::PathBuf, String)>,
}

fn parse_args() -> Args {
    let mut out: Option<std::path::PathBuf> = None;
    let mut inputs = Vec::new();
    let mut expect = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => {
                let Some(v) = it.next() else { usage() };
                out = Some(v.into());
            }
            "--expect" => {
                let Some(pair) = it.next() else { usage() };
                let Some((file, sha)) = pair.rsplit_once('=') else {
                    fail("--expect wants <file>=<sha256>");
                };
                if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
                    fail("--expect wants a 64-hex-digit sha256");
                }
                expect.push((file.into(), sha.to_ascii_lowercase()));
            }
            _ if a.starts_with("--") => usage(),
            _ => inputs.push(a.into()),
        }
    }
    if inputs.is_empty() {
        usage();
    }
    Args {
        out: match out {
            Some(o) => o,
            None => usage(),
        },
        inputs,
        expect,
    }
}

fn main() {
    let args = parse_args();
    // §2.4: verify every pinned digest before merging (base compatibility
    // is the operator's contract).
    for (file, want) in &args.expect {
        let bytes = std::fs::read(file)
            .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", file.display())));
        let got = digest_hex(&bytes);
        if got != *want {
            fail(&format!(
                "digest mismatch for {}: expected {want}, got {got}",
                file.display()
            ));
        }
        if !args.inputs.contains(file) {
            fail(&format!(
                "--expect file {} is not among the inputs",
                file.display()
            ));
        }
    }
    let ledgers: Vec<Ledger> = args
        .inputs
        .iter()
        .map(|p| Ledger::load(p).unwrap_or_else(|e| fail(&e)))
        .collect();
    let (merged, st) = union(&ledgers);
    for (i, p) in args.inputs.iter().enumerate() {
        eprintln!(
            "union: input {} ({} records): new_paths {}, sole_pass_upgrades {}",
            p.display(),
            st.input_records[i],
            st.new_paths[i],
            st.sole_pass_upgrades[i],
        );
    }
    eprintln!(
        "union: {} inputs -> {} records",
        st.inputs, st.union_records
    );
    merged.save(&args.out).unwrap_or_else(|e| fail(&e));
    println!(
        "ledger_union: {} -> {} records",
        args.out.display(),
        st.union_records
    );
}
