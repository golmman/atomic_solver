//! Neutral search-to-worker event protocol.
//!
//! `ProofEvent` decouples the solver from the proof-tree implementation.
//! It has one variant today; it stays an enum so that new event kinds (e.g.
//! an explicit invalidation) can be added without touching `search`'s
//! signature — see `AGENTS.md` §Dependency direction.
//!
//! The solver only ever reports proven nodes, so the stream is monotone: the
//! consumer never sees a reset request.

use atomic_movegen::types::Move;

use crate::position::Outcome;

#[derive(Debug, Clone)]
pub enum ProofEvent {
    NodeProven(NodeProven),
}

#[derive(Debug, Clone)]
pub struct NodeProven {
    pub path: Vec<Move>,
    pub mv: Move,
    pub hash: u64,
    pub outcome: Outcome,
    pub depth: u32,
}

impl NodeProven {
    #[must_use]
    pub fn new(path: Vec<Move>, hash: u64, outcome: Outcome, depth: u32) -> Self {
        let mv = path.last().copied().unwrap_or(Move::NONE);
        Self {
            path,
            mv,
            hash,
            outcome,
            depth,
        }
    }
}
