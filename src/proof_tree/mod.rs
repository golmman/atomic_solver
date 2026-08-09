//! In-memory proof tree and compact binary dump serializer.
//!
//! The proof tree records the nodes that belong to the final proof subtree.
//! Each `ProofNode` carries the position Zobrist hash; the worker's
//! `finalize()` pass uses these hashes to copy fully expanded canonical
//! subtrees onto unexpanded transpositions, producing an authoritative proven
//! subtree without reconstructing it from the transposition table.
//!
//! This module is intentionally larger than 10 KiB because the `ProofTree` data
//! model, the `ProofTreeWorker` thread, and the binary serialization logic form a
//! single cohesive unit; the worker was split into `worker.rs` because it was
//! approaching the 20 KiB soft module-size limit.

use std::io::{self, Read, Write};

use atomic_movegen::types::Move;

use crate::position::Outcome;

pub mod binary;
mod worker;

pub use worker::{ProofStats, ProofTreeWorkerHandle};

/// A single node in the proof tree.
#[derive(Debug, Clone)]
pub struct ProofNode {
    pub parent: Option<usize>,
    pub mv: Move,
    pub hash: u64,
    /// `None` means the node was created as an ancestor placeholder and has
    /// not yet been realized by its own `NodeProven` event.
    pub outcome: Option<Outcome>,
    pub depth: u32,
    pub children: Vec<usize>,
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
                mv: Move::NONE,
                hash: root_hash,
                outcome: root_outcome,
                depth: root_depth,
                children: Vec::new(),
            }],
        }
    }

    /// Add a child node under `parent` and return its id.
    pub(crate) fn add_node(
        &mut self,
        parent_id: usize,
        mv: Move,
        hash: u64,
        outcome: Option<Outcome>,
        depth: u32,
    ) -> usize {
        let id = self.nodes.len();
        self.nodes[parent_id].children.push(id);
        self.nodes.push(ProofNode {
            parent: Some(parent_id),
            mv,
            hash,
            outcome,
            depth,
            children: Vec::new(),
        });
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
            let children = node.children.iter().copied().filter(|&c| {
                let child = &self.nodes[c];
                match outcome {
                    Outcome::Win => child.outcome == Some(Outcome::Loss),
                    Outcome::Loss => child.outcome == Some(Outcome::Win),
                    Outcome::Draw => false,
                }
            });
            let next = match outcome {
                Outcome::Win => children.min_by_key(|&c| self.nodes[c].depth),
                Outcome::Loss => children.max_by_key(|&c| self.nodes[c].depth),
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
            let Some(&next_id) = node.children.iter().find(|&&c| self.nodes[c].mv == *mv) else {
                return false;
            };
            id = next_id;
        }
        self.is_terminal(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;
    use atomic_movegen::types::{Move, Square};

    #[test]
    fn add_node_builds_path() {
        let mut tree = ProofTree::new("fen".to_string(), 0, Some(Outcome::Win), 2);
        let child = tree.add_node(
            0,
            Move::make_move(Square::E2, Square::E4),
            0,
            Some(Outcome::Loss),
            1,
        );
        let grandchild = tree.add_node(
            child,
            Move::make_move(Square::E7, Square::E5),
            0,
            Some(Outcome::Win),
            0,
        );
        assert_eq!(tree.nodes[0].children, vec![child]);
        assert_eq!(tree.nodes[child].children, vec![grandchild]);
    }

    #[test]
    fn to_bin_round_trips_small_tree() {
        let mut tree = ProofTree::new(Position::STARTPOS_FEN.to_string(), 0, Some(Outcome::Win), 2);
        tree.add_node(
            0,
            Move::make_move(Square::E2, Square::E4),
            0,
            Some(Outcome::Loss),
            1,
        );
        tree.add_node(
            1,
            Move::make_move(Square::E7, Square::E5),
            0,
            Some(Outcome::Win),
            0,
        );

        let mut buf = Vec::new();
        tree.to_bin(&mut buf).unwrap();
        let loaded = ProofTree::from_bin(&mut &buf[..]).unwrap();

        assert_eq!(loaded.nodes.len(), tree.nodes.len());
        assert_eq!(loaded.root_fen, tree.root_fen);
        for (a, b) in loaded.nodes.iter().zip(tree.nodes.iter()) {
            assert_eq!(a.mv, b.mv);
            assert_eq!(a.outcome, b.outcome);
            assert_eq!(a.depth, b.depth);
            assert_eq!(a.children, b.children);
        }
    }

    #[test]
    fn extract_ppv_returns_empty_for_drawn_root() {
        let tree = ProofTree::new("fen".to_string(), 0, Some(Outcome::Draw), 0);
        assert!(tree.extract_ppv().is_empty());
    }

    #[test]
    fn validate_ppv_rejects_wrong_path() {
        let mut tree = ProofTree::new("fen".to_string(), 0, Some(Outcome::Win), 2);
        let child = tree.add_node(
            0,
            Move::make_move(Square::E2, Square::E4),
            0,
            Some(Outcome::Loss),
            1,
        );
        tree.add_node(
            child,
            Move::make_move(Square::E7, Square::E5),
            0,
            Some(Outcome::Win),
            0,
        );

        let wrong = vec![Move::make_move(Square::D2, Square::D4)];
        assert!(!tree.validate_ppv(&wrong));
    }

    #[test]
    fn validate_ppv_rejects_premature_termination() {
        let mut tree = ProofTree::new("fen".to_string(), 0, Some(Outcome::Win), 2);
        let _ = tree.add_node(
            0,
            Move::make_move(Square::E2, Square::E4),
            0,
            Some(Outcome::Loss),
            1,
        );
        // The child node at depth 1 is not terminal, so a one-move PV is invalid.
        assert!(!tree.validate_ppv(&[Move::make_move(Square::E2, Square::E4)]));
    }
}
