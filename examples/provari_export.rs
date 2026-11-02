//! Export a `proof_tree.bin` to provari's JSON-lines checker format.
//!
//! The export is derived evidence: the tree (and the DB built from it) stays
//! the validated artifact, and provari is a second, independent rules
//! implementation that either accepts the proof or rejects it. Only root-`Win`
//! trees can be exported; the JSON-lines reader assumes the side to move is
//! the attacker.
//!
//! Usage:
//!     cargo run --release --example provari_export -- proof_tree.bin
//!     cargo run --release --example provari_export -- --out proof.jsonl proof_tree.bin
//!
//! Then check the result with the pinned oracle:
//!     provari check proof.jsonl --seconds 60

use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;

use atomic_solver::proof_tree::{ProofTree, write_provari_jsonl};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out: Option<String> = None;
    let mut path: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--out needs a value");
                    return ExitCode::FAILURE;
                };
                out = Some(value.clone());
                i += 2;
            }
            other if other.starts_with("--") => {
                eprintln!(
                    "unknown option '{other}' (usage: provari_export [--out PATH] <proof_tree.bin>)"
                );
                return ExitCode::FAILURE;
            }
            other => {
                if path.is_some() {
                    eprintln!(
                        "unexpected extra argument '{other}' (usage: provari_export [--out PATH] <proof_tree.bin>)"
                    );
                    return ExitCode::FAILURE;
                }
                path = Some(other.to_string());
                i += 1;
            }
        }
    }
    let Some(path) = path else {
        eprintln!("usage: provari_export [--out PATH] <proof_tree.bin>");
        return ExitCode::FAILURE;
    };

    let data = match fs::read(&path) {
        Ok(data) => data,
        Err(error) => {
            eprintln!("cannot read {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let tree = match ProofTree::from_bin(&mut &data[..]) {
        Ok(tree) => tree,
        Err(error) => {
            eprintln!("cannot parse {path}: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut jsonl = Vec::new();
    let lines = match write_provari_jsonl(&tree, &mut jsonl) {
        Ok(lines) => lines,
        Err(error) => {
            eprintln!("export failed: {error}");
            return ExitCode::FAILURE;
        }
    };

    match out {
        Some(out) => {
            if let Err(error) = fs::write(&out, &jsonl) {
                eprintln!("cannot write {out}: {error}");
                return ExitCode::FAILURE;
            }
        }
        None => {
            let mut stdout = io::stdout().lock();
            if let Err(error) = stdout.write_all(&jsonl) {
                eprintln!("cannot write stdout: {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    let outcome = tree.nodes[0].outcome.map_or("none", |o| o.as_str());
    eprintln!(
        "positions: {lines}, nodes: {}, root: {outcome}",
        tree.nodes.len()
    );
    ExitCode::SUCCESS
}
