//! Neutral search-to-worker event protocol.
//!
//! `ProofEvent` decouples the solver from the proof-tree implementation.
//! The `search` module emits events; `proof_tree` consumes them.

use atomic_movegen::types::Move;

use crate::position::Outcome;

#[derive(Debug, Clone)]
pub enum ProofEvent {
    Clear,
    NodeProven(NodeProven),
}

#[derive(Debug, Clone)]
pub struct NodeProven {
    pub path: Vec<Move>,
    pub mv: Move,
    pub outcome: Outcome,
    pub depth: u32,
}

impl NodeProven {
    #[must_use]
    pub fn new(path: Vec<Move>, outcome: Outcome, depth: u32) -> Self {
        let mv = path.last().copied().unwrap_or(Move::NONE);
        Self {
            path,
            mv,
            outcome,
            depth,
        }
    }
}
