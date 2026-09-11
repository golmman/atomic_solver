use std::fs;

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Outcome;
use atomic_solver::proof_tree::{ProofTree, validate_proof_tree};

fn dump_tree(tree: &ProofTree, node: usize, prefix: &mut Vec<String>, max_ply: usize) {
    let n = &tree.nodes[node];
    if prefix.len() > max_ply {
        println!("{} ... (truncated)", prefix.join(" "));
        return;
    }
    if n.first_child.is_none() {
        let uci = prefix.join(" ");
        println!(
            "leaf ply={} outcome={:?} depth={} path={}",
            prefix.len(),
            n.outcome.unwrap_or(Outcome::Draw),
            n.depth,
            uci
        );
        return;
    }
    for c in tree.children(node) {
        let uci = move_to_uci(tree.nodes[c].mv);
        prefix.push(uci);
        dump_tree(tree, c, prefix, max_ply);
        prefix.pop();
    }
}

fn main() {
    let mut path = "proof_tree.bin".to_string();
    let mut validate = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--validate" => validate = true,
            other if other.starts_with("--") => {
                panic!("unknown option '{other}' (usage: inspect_pt [--validate] [file])")
            }
            _ => path = arg,
        }
    }
    let data = fs::read(&path).expect("read proof tree");
    let tree = ProofTree::from_bin(&mut &data[..]).expect("parse proof tree");

    println!("nodes: {}", tree.nodes.len());
    println!(
        "root outcome: {:?} depth: {}",
        tree.nodes[0].outcome.unwrap_or(Outcome::Draw),
        tree.nodes[0].depth
    );
    println!("root children:");
    for c in tree.children(0) {
        let n = &tree.nodes[c];
        println!(
            "  {} outcome={:?} depth={} children={}",
            move_to_uci(n.mv),
            n.outcome.unwrap_or(Outcome::Draw),
            n.depth,
            tree.children(c).count()
        );
    }

    let ppv = tree.extract_ppv();
    println!(
        "extract_ppv: {}",
        ppv.iter()
            .map(|m| move_to_uci(*m))
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("validate_ppv: {}", tree.validate_ppv(&ppv));
    println!("--- leaves (max 20) ---");
    let mut prefix = Vec::new();
    dump_tree(&tree, 0, &mut prefix, 30);

    // Replay-based structural validation. Off by default to keep the plain
    // JSON dump cheap. Loaded trees carry hash == 0, so hash checks are
    // skipped; cycle, terminal, depth, and completeness checks still run.
    if validate {
        match validate_proof_tree(&tree) {
            Ok(()) => println!("pt_validate: ok"),
            Err(defects) => {
                for defect in &defects {
                    println!("pt_validate: FAILED {defect}");
                }
                println!("pt_validate: FAILED {} defect(s)", defects.len());
                std::process::exit(1);
            }
        }
    }
}
