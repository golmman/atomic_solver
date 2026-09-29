//! Proof-skeleton reassembly over the merged path-keyed tree
//! ([`super::PathTree`]): the "always again at finalization" validation leg.
//!
//! The skeleton of a proven node is its minimal replay-verifiable proof
//! structure: at a proven OR-node the single minimum-depth proven-loss child,
//! at a proven AND-node the full reply set, terminals as leaves. Annotation
//! children (disproven moves, open continuations) are excluded — they are
//! facts about the search, not part of the forcing proof.

use atomic_movegen::types::Move;

use atomic_solver::position::Outcome;
use atomic_solver::proof_tree::{ProofNode, ProofTree};

use super::PathTree;

impl PathTree {
    /// Reassemble the proof *skeleton* rooted at `root_id` (which must be
    /// proven) as a [`ProofTree`] with the given root FEN. Node hashes are
    /// left 0 (the replay validator skips them).
    ///
    /// # Errors
    /// A structural inconsistency that makes the skeleton unbuildable (an
    /// unproven skeleton root or a draw node).
    pub fn reassemble_skeleton(&self, root_id: usize, root_fen: &str) -> Result<ProofTree, String> {
        let outcome = self.nodes[root_id].outcome.ok_or_else(|| {
            format!(
                "skeleton root at ply {} is not proven",
                self.nodes[root_id].ply
            )
        })?;
        let mut tree = ProofTree {
            root_fen: root_fen.to_string(),
            nodes: Vec::new(),
        };
        tree.nodes.push(ProofNode {
            parent: None,
            first_child: None,
            next_sibling: None,
            mv: Move::NONE,
            hash: 0,
            outcome: Some(outcome),
            depth: self.depths[root_id],
        });
        // (merged node id, tree node id)
        let mut stack = vec![(root_id, 0usize)];
        while let Some((mid, tid)) = stack.pop() {
            let node = &self.nodes[mid];
            let o = node.outcome.expect("skeleton nodes are proven");
            let kept: Vec<usize> = match o {
                Outcome::Win => node
                    .children
                    .values()
                    .copied()
                    .filter(|&c| self.nodes[c].outcome == Some(Outcome::Loss))
                    .min_by_key(|&c| self.depths[c])
                    .into_iter()
                    .collect(),
                Outcome::Loss => node.children.values().copied().collect(),
                Outcome::Draw => return Err("draw node in skeleton".to_string()),
            };
            for &c in &kept {
                let child = &self.nodes[c];
                let t_id = tree.nodes.len();
                let parent = std::num::NonZeroU32::new((tid as u32) + 1);
                let id_nz = std::num::NonZeroU32::new(t_id as u32).expect("tree id overflow");
                let old_first = tree.nodes[tid].first_child;
                tree.nodes.push(ProofNode {
                    parent,
                    first_child: None,
                    next_sibling: old_first,
                    mv: child.mv.expect("non-root merged node has a move"),
                    hash: 0,
                    outcome: child.outcome,
                    depth: self.depths[c],
                });
                tree.nodes[tid].first_child = Some(id_nz);
                stack.push((c, t_id));
            }
        }
        Ok(tree)
    }
}
