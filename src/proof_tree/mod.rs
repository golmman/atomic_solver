//! In-memory proof tree and compact binary dump serializer.
//!
//! The proof tree records the nodes that belong to the final proof subtree.
//! Each `ProofNode` carries the position Zobrist hash; the worker's
//! `finalize()` pass uses these hashes to copy fully expanded canonical
//! subtrees onto unexpanded transpositions, producing an authoritative proven
//! subtree without reconstructing it from the transposition table.

pub mod binary;
mod node;
mod worker;

pub use node::{ProofNode, ProofTree};
pub use worker::{ProofStats, ProofTreeWorkerHandle};
