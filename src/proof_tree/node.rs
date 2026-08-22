//! Proof-tree nodes and the in-memory tree.
//!
//! Each `ProofNode` carries the position Zobrist hash; the worker's
//! `finalize()` pass uses these hashes to copy fully expanded canonical
//! subtrees onto unexpanded transpositions, producing an authoritative proven
//! subtree without reconstructing it from the transposition table.

use std::io::{self, Read, Write};
use std::num::NonZeroU32;

use atomic_movegen::types::Move;

use crate::position::Outcome;

use super::binary;

/// A single node in the proof tree.
#[derive(Debug, Clone)]
pub struct ProofNode {
    pub parent: Option<NonZeroU32>,       // id + 1; None for root
    pub first_child: Option<NonZeroU32>,  // raw child id; None if no children
    pub next_sibling: Option<NonZeroU32>, // raw sibling id; None if last
    pub mv: Move,
    pub hash: u64,
    /// `None` means the node was created as an ancestor placeholder and has
    /// not yet been realized by its own `NodeProven` event.
    pub outcome: Option<Outcome>,
    pub depth: u32,
    /// Cumulative `child_evals` spent proving this node's subtree, recorded
    /// from the `NodeProven` event at prove time.
    pub work: u64,
}

/// Proof tree built by traversing event paths and realizing dummy parents.
#[derive(Debug, Clone)]
pub struct ProofTree {
    pub root_fen: String,
    pub nodes: Vec<ProofNode>,
}

impl ProofTree {
    /// Create a new proof tree with a single root node.
    pub fn new(
        root_fen: String,
        root_hash: u64,
        root_outcome: Option<Outcome>,
        root_depth: u32,
    ) -> Self {
        Self {
            root_fen,
            nodes: vec![ProofNode {
                parent: None,
                first_child: None,
                next_sibling: None,
                mv: Move::NONE,
                hash: root_hash,
                outcome: root_outcome,
                depth: root_depth,
                work: 0,
            }],
        }
    }

    /// Iterate over the ids of `node_id`'s children.
    pub fn children(&self, node_id: usize) -> impl Iterator<Item = usize> + '_ {
        let mut next = self.nodes[node_id].first_child.map(|nz| nz.get() as usize);
        std::iter::from_fn(move || {
            let id = next?;
            next = self.nodes[id].next_sibling.map(|nz| nz.get() as usize);
            Some(id)
        })
    }

    /// Add a child under `parent_id` and return its id.
    pub(crate) fn add_node(
        &mut self,
        parent_id: usize,
        mv: Move,
        hash: u64,
        outcome: Option<Outcome>,
        depth: u32,
        work: u64,
    ) -> usize {
        let id = self.nodes.len();
        assert!(id < u32::MAX as usize, "proof tree node id overflow");

        let parent = NonZeroU32::new((parent_id as u32) + 1);
        let id_nz = NonZeroU32::new(id as u32).unwrap();

        let old_first = self.nodes[parent_id].first_child;
        self.nodes.push(ProofNode {
            parent,
            first_child: None,
            next_sibling: old_first,
            mv,
            hash,
            outcome,
            depth,
            work,
        });
        self.nodes[parent_id].first_child = Some(id_nz);
        id
    }

    /// Serialize the tree to the compact binary adjacency format.
    pub fn to_bin<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        binary::write_proof_tree(self, writer)
    }

    /// Load a tree from the compact binary adjacency format.
    #[must_use = "the loaded proof tree or its error should be handled"]
    pub fn from_bin<R: Read>(reader: &mut R) -> io::Result<Self> {
        binary::read_proof_tree(reader)
    }

    /// Return true if the node is a terminal leaf (real and proven at depth 0).
    #[must_use]
    pub fn is_terminal(&self, node_id: usize) -> bool {
        self.nodes
            .get(node_id)
            .is_some_and(|n| n.outcome.is_some() && n.depth == 0)
    }

    /// Extract a principal variation from the proof tree.
    ///
    /// * `Win` (OR) nodes pick the proven winning child with the smallest depth.
    /// * `Loss` (AND) nodes pick the defender reply with the largest depth.
    /// * The walk stops at a terminal node.
    #[must_use]
    pub fn extract_ppv(&self) -> Vec<Move> {
        let mut pv = Vec::new();
        let mut id = 0usize;
        while !self.is_terminal(id) {
            let node = &self.nodes[id];
            let Some(outcome) = node.outcome else {
                break;
            };
            let children: Vec<usize> = self
                .children(id)
                .filter(|&c| {
                    let child = &self.nodes[c];
                    match outcome {
                        Outcome::Win => child.outcome == Some(Outcome::Loss),
                        Outcome::Loss => child.outcome == Some(Outcome::Win),
                        Outcome::Draw => false,
                    }
                })
                .collect();
            let next = match outcome {
                Outcome::Win => children.into_iter().min_by_key(|&c| self.nodes[c].depth),
                Outcome::Loss => children.into_iter().max_by_key(|&c| self.nodes[c].depth),
                Outcome::Draw => None,
            };
            let Some(next_id) = next else {
                break;
            };
            pv.push(self.nodes[next_id].mv);
            id = next_id;
        }
        pv
    }

    /// Validate that `pv` is a path from the root to a terminal node in the
    /// proof tree.  This does not replay the moves on a chess board; it only
    /// checks that the sequence of moves exists in the tree.
    #[must_use]
    pub fn validate_ppv(&self, pv: &[Move]) -> bool {
        let mut id = 0usize;
        for mv in pv {
            if self.is_terminal(id) {
                return false;
            }
            let node = &self.nodes[id];
            if node.outcome.is_none() {
                return false;
            }
            let Some(next_id) = self.children(id).find(|&c| self.nodes[c].mv == *mv) else {
                return false;
            };
            id = next_id;
        }
        self.is_terminal(id)
    }
}

#[cfg(test)]
mod tests;
