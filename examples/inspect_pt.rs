use std::fs;

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Outcome;
use atomic_solver::proof_tree::ProofTree;

fn dump_tree(tree: &ProofTree, node: usize, prefix: &mut Vec<String>, max_ply: usize) {
    let n = &tree.nodes[node];
    if prefix.len() > max_ply {
        println!("{} ... (truncated)", prefix.join(" "));
        return;
    }
    if n.children.is_empty() {
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
    for &c in &n.children {
        let uci = move_to_uci(tree.nodes[c].mv);
        prefix.push(uci);
        dump_tree(tree, c, prefix, max_ply);
        prefix.pop();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).map(|s| s.as_str()).unwrap_or("proof_tree.bin");
    let data = fs::read(path).expect("read proof tree");
    let tree = ProofTree::from_bin(&mut &data[..]).expect("parse proof tree");

    println!("nodes: {}", tree.nodes.len());
    println!(
        "root outcome: {:?} depth: {}",
        tree.nodes[0].outcome.unwrap_or(Outcome::Draw),
        tree.nodes[0].depth
    );
    println!("root children:");
    for &c in &tree.nodes[0].children {
        let n = &tree.nodes[c];
        println!(
            "  {} outcome={:?} depth={} children={}",
            move_to_uci(n.mv),
            n.outcome.unwrap_or(Outcome::Draw),
            n.depth,
            n.children.len()
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
}
