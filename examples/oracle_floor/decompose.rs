//! Static decomposition of oracle proof trees (`docs/plans/nn/plan8.md`,
//! Step 3, mode `decompose`).
//!
//! No search is run: the recorded `work` of the finalized baseline proof
//! trees is split at every internal node to bound what perfect ordering
//! could have saved.
//!
//! Structural caveat (measured, see `report8.md`): the finalized proof tree
//! keeps only the winning claim — `reconcile_children` keeps exactly one
//! (shallowest) `Loss` child at every `Win` parent, so refuted OR siblings
//! are *not* recorded as children. Their cost is however included in the OR
//! node's own cumulative `work`. The decomposition therefore measures at OR
//! nodes:
//!
//! - `decisive share` = recorded work of the decisive child / node `work`
//!   — the fraction perfect OR ordering cannot avoid;
//! - `recoverable share` = 1 - decisive share — refuted-sibling exploration
//!   plus the node's own evals, i.e. the upper bound on what perfect OR
//!   ordering saves.
//!
//! At AND nodes every child is recorded (all are proven `Win`), so the
//! per-child share of the node's recorded child `work` distribution
//! (min/median/max) shows how concentrated disproving work is, i.e. whether
//! AND ordering is a lever at all.

use atomic_solver::position::Outcome;
use atomic_solver::proof_tree::ProofTree;

pub struct CaseDecomposition {
    pub name: String,
    pub tree_nodes: usize,
    pub max_depth: u32,
    pub or_nodes: usize,
    pub and_nodes: usize,
    /// Sum of OR-node cumulative `work`.
    pub or_node_work: u64,
    /// Sum of recorded decisive-child `work` over OR nodes (raw; inflated by
    /// transposition copies).
    pub or_decisive_work: u64,
    /// Sum of `min(decisive-child work, node work)` over OR nodes; the
    /// honest numerator for aggregate shares.
    pub or_decisive_work_clamped: u64,
    /// Decisive share of each OR node's own `work`, clamped to [0,1].
    pub or_decisive_shares: Vec<f64>,
    /// OR nodes whose recorded decisive-child `work` exceeds the node's own
    /// cumulative `work` (transposition copies double-count; the share is
    /// clamped to 1 for those).
    pub or_inflated: usize,
    /// Per-child share of its AND node's recorded child `work`.
    pub and_child_shares: Vec<f64>,
    /// Per-AND-node share of the largest child (concentration).
    pub and_node_max_shares: Vec<f64>,
}

pub fn decompose(name: &str, tree: &ProofTree) -> CaseDecomposition {
    let mut d = CaseDecomposition {
        name: name.to_string(),
        tree_nodes: tree.nodes.len(),
        max_depth: 0,
        or_nodes: 0,
        and_nodes: 0,
        or_node_work: 0,
        or_decisive_work: 0,
        or_decisive_work_clamped: 0,
        or_decisive_shares: Vec::new(),
        or_inflated: 0,
        and_child_shares: Vec::new(),
        and_node_max_shares: Vec::new(),
    };
    for id in 0..tree.nodes.len() {
        let node = &tree.nodes[id];
        d.max_depth = d.max_depth.max(node.depth);
        let children: Vec<usize> = tree.children(id).collect();
        if children.is_empty() {
            continue;
        }
        match node.outcome {
            Some(Outcome::Win) => {
                d.or_nodes += 1;
                let decisive: u64 = children
                    .iter()
                    .copied()
                    .filter(|&c| tree.nodes[c].outcome == Some(Outcome::Loss))
                    .map(|c| tree.nodes[c].work)
                    .sum();
                d.or_node_work += node.work;
                d.or_decisive_work += decisive;
                d.or_decisive_work_clamped += decisive.min(node.work);
                if node.work > 0 {
                    let raw = decisive as f64 / node.work as f64;
                    if raw > 1.0 {
                        d.or_inflated += 1;
                        d.or_decisive_shares.push(1.0);
                    } else {
                        d.or_decisive_shares.push(raw);
                    }
                }
            }
            Some(Outcome::Loss) => {
                d.and_nodes += 1;
                let total: u64 = children.iter().map(|&c| tree.nodes[c].work).sum();
                if total > 0 {
                    let mut max_share = 0.0f64;
                    for &c in &children {
                        let share = tree.nodes[c].work as f64 / total as f64;
                        d.and_child_shares.push(share);
                        max_share = max_share.max(share);
                    }
                    d.and_node_max_shares.push(max_share);
                }
            }
            _ => {}
        }
    }
    d
}

pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

pub fn print_case(d: &CaseDecomposition) {
    println!(
        "=== {}  nodes={}  max_depth={}  or_nodes={}  and_nodes={}",
        d.name, d.tree_nodes, d.max_depth, d.or_nodes, d.and_nodes
    );
    let decisive_pct = if d.or_node_work > 0 {
        100.0 * d.or_decisive_work_clamped as f64 / d.or_node_work as f64
    } else {
        0.0
    };
    let inflated = if d.or_inflated > 0 {
        format!(", {} transposition-inflated nodes clamped", d.or_inflated)
    } else {
        String::new()
    };
    println!(
        "  OR work: decisive-child share={:.1}%  recoverable (refutation+own)={:.1}%  (node={} decisive_raw={}{})",
        decisive_pct,
        100.0 - decisive_pct,
        d.or_node_work,
        d.or_decisive_work,
        inflated,
    );
    if !d.or_decisive_shares.is_empty() {
        let mut shares = d.or_decisive_shares.clone();
        shares.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "  OR decisive share per node: min={:.1}%  median={:.1}%  max={:.1}%",
            100.0 * percentile(&shares, 0.0),
            100.0 * percentile(&shares, 0.5),
            100.0 * percentile(&shares, 1.0),
        );
    }
    if !d.and_child_shares.is_empty() {
        let mut shares = d.and_child_shares.clone();
        shares.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut maxes = d.and_node_max_shares.clone();
        maxes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "  AND child work share: min={:.1}%  median={:.1}%  max={:.1}%  (per-node max: median={:.1}%)",
            100.0 * percentile(&shares, 0.0),
            100.0 * percentile(&shares, 0.5),
            100.0 * percentile(&shares, 1.0),
            100.0 * percentile(&maxes, 0.5),
        );
    }
}
