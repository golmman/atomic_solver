//! The path-keyed partial AND/OR tree and the shard overlay semantics.
//!
//! Node identity is the UCI move path from the startpos
//! (`docs/spec/global_proof_store.md` §1); the same position at a different
//! halfmove clock is a different node. Overlay rules (pre-registered in the
//! initiative's design constraints):
//!
//! - *Agreement*: same path, same proven outcome → union children, provenance
//!   = union of contributing shard tags.
//! - *Refinement*: same path, both proven → the tighter bound wins. This is
//!   implemented as a global bottom-up recomputation over the merged
//!   structure ([`PathTree::finalize`]), which is sound because merging only
//!   refines: a proven AND-node's reply set is complete in every contributing
//!   shard, and a proven OR-node's bound is `min(loss-child depth) + 1` over
//!   its proven-loss children.
//! - *Contradiction*: different proven outcomes at the same path, or an
//!   `exact` claim inconsistent with the merged structure → hard error (the
//!   caller aborts). A conflict is a soundness bug in a shard or the
//!   manifest — never patched.
//!
//! Proven OR-nodes may additionally carry *annotation children* (sibling
//! probes grafted below them: disproven or alternative winning moves, or open
//! continuations). These are facts, not proof structure; the proof *skeleton*
//! — an OR-node's single minimum-depth proven-loss child, an AND-node's full
//! reply set, terminals as leaves — is what [`PathTree::reassemble_skeleton`]
//! extracts for replay validation. Annotation children never contribute to a
//! node's recomputed bound.
//!
//! Submodules: [`depth`] (the refinement fixpoint and row-id assignment) and
//! [`skeleton`] (proof-skeleton reassembly for replay validation); the tests
//! live in [`tests`] so the integration target can include them verbatim.

use std::collections::{BTreeMap, BTreeSet};

use atomic_movegen::types::Move;

use atomic_solver::notation::move_to_bits;
use atomic_solver::position::Outcome;
use atomic_solver::proof_tree::ProofTree;

mod depth;
mod skeleton;
#[cfg(test)]
mod tests;

pub use depth::DepthStatus;

/// A node of the merged path-keyed tree.
pub struct PathNode {
    pub parent: Option<usize>,
    pub ply: usize,
    pub mv: Option<Move>,
    pub outcome: Option<Outcome>,
    pub provenance: BTreeSet<String>,
    /// Children keyed by 16-bit move code (deterministic iteration order).
    pub children: BTreeMap<u16, usize>,
    /// `exact` depth claims contributed by shards (terminals claim 0).
    pub exact_claims: Vec<u32>,
}

/// The merged tree plus merge counters (the census).
pub struct PathTree {
    pub nodes: Vec<PathNode>,
    /// Bottom-up recomputed depth bounds, filled by [`PathTree::finalize`].
    pub depths: Vec<u32>,
    /// Creation-index → row-id map; row ids are assigned in lexicographic
    /// path order by [`PathTree::finalize`]. The parent-row lookups in the
    /// DB/dump writers go through this mapping.
    pub row_of: Vec<usize>,
    /// Row-id → creation-index (the lexicographic order itself); the DB/dump
    /// writers iterate rows through it.
    pub row_order: Vec<usize>,
    pub ancestor_insertions: usize,
    pub overlay_insertions: usize,
    pub open_upgraded: usize,
}

impl PathTree {
    /// A tree containing only the open startpos root (row id 0).
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: vec![PathNode {
                parent: None,
                ply: 0,
                mv: None,
                outcome: None,
                provenance: BTreeSet::new(),
                children: BTreeMap::new(),
                exact_claims: Vec::new(),
            }],
            depths: Vec::new(),
            row_of: Vec::new(),
            row_order: Vec::new(),
            ancestor_insertions: 0,
            overlay_insertions: 0,
            open_upgraded: 0,
        }
    }

    /// Create `path`'s ancestor chain as open nodes (a graft), tagging each
    /// node's provenance with `tag`, and return the graft node's index.
    pub fn graft_path(&mut self, path: &[Move], tag: &str) -> usize {
        let mut cur = 0;
        self.nodes[0].provenance.insert(tag.to_string());
        for &mv in path {
            let code = move_to_bits(mv);
            let next = match self.nodes[cur].children.get(&code) {
                Some(&id) => id,
                None => {
                    let id = self.nodes.len();
                    self.nodes.push(PathNode {
                        parent: Some(cur),
                        ply: self.nodes[cur].ply + 1,
                        mv: Some(mv),
                        outcome: None,
                        provenance: BTreeSet::new(),
                        children: BTreeMap::new(),
                        exact_claims: Vec::new(),
                    });
                    self.nodes[cur].children.insert(code, id);
                    self.ancestor_insertions += 1;
                    id
                }
            };
            self.nodes[next].provenance.insert(tag.to_string());
            cur = next;
        }
        cur
    }

    /// Overlay a validated shard tree at `root_id` (the graft node = the
    /// shard's root position). Union children, check proven-outcome
    /// agreement, and union provenance. Depths are *not* stored here; they
    /// are recomputed bottom-up by [`PathTree::finalize`].
    ///
    /// # Errors
    /// A proven-outcome contradiction between the shard and the merged tree
    /// at any path, or a shard node without a decisive outcome.
    pub fn overlay_subtree(
        &mut self,
        root_id: usize,
        tree: &ProofTree,
        tag: &str,
    ) -> Result<(), String> {
        let pre = self.nodes.len(); // nodes created by this overlay are not upgrades
        let mut stack = vec![(root_id, 0usize)];
        while let Some((mid, tid)) = stack.pop() {
            let tnode = &tree.nodes[tid];
            match tnode.outcome {
                Some(o @ (Outcome::Win | Outcome::Loss)) => {
                    let node = &mut self.nodes[mid];
                    match node.outcome {
                        Some(prev) if prev != o => {
                            return Err(format!(
                                "contradiction: node at ply {} has proven outcome {prev} \
                                 in the merged tree but {o} in shard {tag}",
                                node.ply
                            ));
                        }
                        Some(_) => {}
                        None => {
                            node.outcome = Some(o);
                            if mid < pre {
                                self.open_upgraded += 1;
                            }
                        }
                    }
                    if tnode.depth == 0 {
                        // Rule-terminal: minimality holds by definition.
                        node.exact_claims.push(0);
                    }
                    node.provenance.insert(tag.to_string());
                }
                Some(Outcome::Draw) | None => {
                    return Err(format!(
                        "shard {tag} contains a draw or unrealized node at tree node {tid}"
                    ));
                }
            }
            for tc in tree.children(tid) {
                let mv = tree.nodes[tc].mv;
                let code = move_to_bits(mv);
                let cid = match self.nodes[mid].children.get(&code) {
                    Some(&cid) => cid,
                    None => {
                        let cid = self.nodes.len();
                        self.nodes.push(PathNode {
                            parent: Some(mid),
                            ply: self.nodes[mid].ply + 1,
                            mv: Some(mv),
                            outcome: None,
                            provenance: BTreeSet::new(),
                            children: BTreeMap::new(),
                            exact_claims: Vec::new(),
                        });
                        self.nodes[mid].children.insert(code, cid);
                        self.overlay_insertions += 1;
                        cid
                    }
                };
                stack.push((cid, tc));
            }
        }
        Ok(())
    }
}

impl Default for PathTree {
    fn default() -> Self {
        Self::new()
    }
}
