//! The depth-refinement fixpoint and row-id assignment over the merged
//! path-keyed tree ([`super::PathTree`]).
//!
//! Depth bounds are *recomputed* bottom-up over the merged structure rather
//! than taken from the shards: the recomputation is order-independent and
//! always ≤ every contributing shard's claim, which makes the refinement
//! overlay rule (tighter bound wins) a property of the fixpoint instead of a
//! per-merge decision. An `exact` claim is accepted only where it survives
//! the consistency check against the recomputed structure; the seed shard
//! set contributes no `exact` claims beyond rule-terminals (their minimality
//! holds by definition), so every non-terminal proven node stays `bound` —
//! the conservative seed mapping fixed by plan1 §3.

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Outcome;

use super::PathTree;

/// Depth-status annotation of a proven node (`docs/spec/global_proof_store.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthStatus {
    /// Upper bound; the true distance-to-terminal may be smaller.
    Bound,
    /// The bound is proven minimal.
    Exact,
}

impl DepthStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bound => "bound",
            Self::Exact => "exact",
        }
    }
}

impl PathTree {
    /// Bottom-up recomputation of every proven node's depth bound over the
    /// merged structure (the refinement fixpoint: order-independent, always
    /// ≤ every contributing shard's claim), plus the `exact`-claim
    /// consistency checks. Row ids are assigned here.
    ///
    /// # Errors
    /// A proven AND-node with an unproven reply child, a proven OR-node
    /// without a proven-loss child, or an `exact` claim inconsistent with the
    /// merged structure.
    pub fn finalize(&mut self) -> Result<(), String> {
        let mut depth = vec![0u32; self.nodes.len()];
        let mut computed = vec![false; self.nodes.len()];
        for i in 0..self.nodes.len() {
            self.compute_depth(i, &mut depth, &mut computed)?;
        }
        self.depths = depth;
        let order = self.lexicographic_order();
        self.row_order = order.clone();
        // Inverse mapping: row id (= rank) → creation index is `row_order`;
        // creation index → row id is its inverse.
        self.row_of = vec![0; order.len()];
        for (rank, &idx) in order.iter().enumerate() {
            self.row_of[idx] = rank;
        }
        Ok(())
    }

    fn compute_depth(
        &self,
        i: usize,
        depth: &mut [u32],
        computed: &mut [bool],
    ) -> Result<(), String> {
        if computed[i] {
            return Ok(());
        }
        let node = &self.nodes[i];
        let bound =
            match node.outcome {
                None => 0,
                Some(o) => {
                    for &c in node.children.values() {
                        self.compute_depth(c, depth, computed)?;
                    }
                    if node.children.is_empty() {
                        0 // rule-terminal
                    } else {
                        let mut proven_depths: Vec<u32> = Vec::new();
                        let mut loss_depths: Vec<u32> = Vec::new();
                        for &c in node.children.values() {
                            match self.nodes[c].outcome {
                                Some(Outcome::Loss) => {
                                    // A proven-loss child is proof structure only
                                    // under a proven OR-node (its proving child or
                                    // an alternative winning move).
                                    if o == Outcome::Loss {
                                        return Err(format!(
                                            "contradiction: proven AND-node at ply {} (path {}) \
                                         has a proven-loss reply child at ply {} (path {})",
                                            node.ply,
                                            self.path_uci(i).join(" "),
                                            self.nodes[c].ply,
                                            self.path_uci(c).join(" ")
                                        ));
                                    }
                                    proven_depths.push(depth[c]);
                                    loss_depths.push(depth[c]);
                                }
                                Some(Outcome::Win) => {
                                    // The expected reply child of a proven AND-node;
                                    // under a proven OR-node it is a disproven-move
                                    // annotation.
                                    proven_depths.push(depth[c]);
                                }
                                Some(Outcome::Draw) => {
                                    return Err(format!(
                                        "draw node at ply {} in the merged tree",
                                        self.nodes[c].ply
                                    ));
                                }
                                None => {
                                    // An open child is only legal as an annotation
                                    // continuation below a proven OR-node.
                                    if o == Outcome::Loss {
                                        return Err(format!(
                                            "contradiction: proven AND-node at ply {} (path {}) \
                                         has an unproven reply child at ply {} (path {})",
                                            node.ply,
                                            self.path_uci(i).join(" "),
                                            self.nodes[c].ply,
                                            self.path_uci(c).join(" ")
                                        ));
                                    }
                                }
                            }
                        }
                        match o {
                            Outcome::Win => loss_depths.iter().min().copied().map_or_else(
                                || {
                                    Err(format!(
                                        "contradiction: proven OR-node at ply {} has no \
                                     proven-loss child",
                                        node.ply
                                    ))
                                },
                                |best| Ok(best + 1),
                            )?,
                            Outcome::Loss => {
                                proven_depths.iter().max().copied().expect(
                                    "a proven AND-node with children has proven reply children",
                                ) + 1
                            }
                            Outcome::Draw => unreachable!("checked above"),
                        }
                    }
                }
            };
        for &claim in &node.exact_claims {
            if claim != bound {
                return Err(format!(
                    "contradiction: node at ply {} claims exact depth {claim} but the \
                     merged structure proves {bound}",
                    node.ply
                ));
            }
        }
        depth[i] = bound;
        computed[i] = true;
        Ok(())
    }

    /// Row ids in lexicographic path order (elementwise 16-bit move-code
    /// order, prefix before extension; the root path is least).
    #[must_use]
    pub fn lexicographic_order(&self) -> Vec<usize> {
        let mut order = Vec::with_capacity(self.nodes.len());
        let mut stack = vec![0usize];
        while let Some(i) = stack.pop() {
            order.push(i);
            for &c in self.nodes[i].children.values().rev() {
                stack.push(c);
            }
        }
        order
    }

    /// The node's depth bound, if proven (`finalize` must have run).
    #[must_use]
    pub fn depth_bound(&self, i: usize) -> Option<u32> {
        self.nodes[i].outcome.is_some().then_some(self.depths[i])
    }

    /// The node's depth status, if proven. Terminals (no children) and nodes
    /// with a shard-contributed `exact` claim are `exact`.
    #[must_use]
    pub fn depth_status(&self, i: usize) -> Option<DepthStatus> {
        let node = &self.nodes[i];
        let status = if node.children.is_empty() || !node.exact_claims.is_empty() {
            DepthStatus::Exact
        } else {
            DepthStatus::Bound
        };
        node.outcome.is_some().then_some(status)
    }

    /// The node's UCI path from the root.
    #[must_use]
    pub fn path_uci(&self, i: usize) -> Vec<String> {
        let mut uci = Vec::new();
        let mut cur = i;
        while let Some(mv) = self.nodes[cur].mv {
            uci.push(move_to_uci(mv));
            cur = self.nodes[cur].parent.expect("non-root node has a parent");
        }
        uci.reverse();
        uci
    }
}
