//! In-memory proof tree and compact binary dump serializer.
//!
//! The proof tree records the nodes that belong to the final proof subtree.
//! Each `ProofNode` carries the position Zobrist hash; the worker's
//! `finalize()` pass uses these hashes to copy fully expanded canonical
//! subtrees onto unexpanded transpositions, producing an authoritative proven
//! subtree without reconstructing it from the transposition table.
//!
//! The `validate` submodule re-plays finalized trees on real positions to
//! check the structural rules of a proof; it therefore depends on
//! `crate::position` (a base layer below `search`), which keeps the
//! worker/search decoupling intact.

pub mod binary;
mod node;
mod validate;
mod worker;

pub use node::{ProofNode, ProofTree};
pub use validate::{DefectKind, TreeDefect, validate_proof_tree};
pub use worker::{ProofStats, ProofTreeWorkerHandle};
