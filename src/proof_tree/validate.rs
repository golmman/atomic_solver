//! Replay-based structural validation of a finalized proof tree.
//!
//! This file is larger than the 10 KiB guideline because the proof rules,
//! the defect taxonomy, and the two-phase (Enter/Exit) replay DFS share one
//! state machine: the path/hash bookkeeping, the replay invariants (only
//! parent-verified legal moves are played), and the depth-completeness
//! propagation are documented against each other and must not be scattered
//! across modules.
//!
//! [`validate_proof_tree`] re-plays every root-to-node path on a real
//! [`Position`] (state, legality, static outcome, and Zobrist hash are
//! therefore exact) and checks the structural rules of a proof:
//!
//! - **Outcome domain**: every realized node is `Win` or `Loss`, and the root
//!   is decisive. The search never emits Draw events
//!   (`search::dfpn::emit_proof_node` returns early for `Outcome::Draw`), so
//!   a Draw or unrealized node in a finalized tree is a defect.
//! - **Win, non-terminal** (`depth > 0`): exactly one child, its move is
//!   legal, and every child is `Loss`.
//! - **Loss, non-terminal**: the set of child moves equals the full
//!   `legal_moves_vec()` of the replayed position (each missing reply is one
//!   `LossIncomplete` defect — a Loss proof must cover *all* legal replies)
//!   and every child is `Win`.
//! - **Terminal** (`depth == 0`): [`Position::outcome_from_state`] on the
//!   replayed position must yield the node's outcome. A depth-0 node that is
//!   *not* statically terminal is a defect — this catches finalize's
//!   cycle-guard leaves and any "leaf-ified" interior node. Children of a
//!   depth-0 node are not descended into.
//! - **Depths**: bottom-up `Win = min(child)+1`, `Loss = max(child)+1`,
//!   terminal = 0 — an independent re-derivation of what the worker's
//!   `finalize_tree` recomputes, including copied canonical subtrees. The
//!   check is skipped for subtrees that could not be fully replayed (those
//!   already carry defects).
//! - **Hashes**: skipped when `node.hash == 0` (that is what trees loaded
//!   from a binary dump carry, since the dump format does not store hashes);
//!   otherwise the replayed `Position::hash()` must equal `node.hash`.
//! - **Cycles**: a node whose replayed full Zobrist hash already appeared on
//!   the current path is reported as [`DefectKind::CycleLeaf`], mirroring the
//!   worker's finalize cycle guard. By the first-player-loss GHI shortcut a
//!   repetition on a proof path is never a proof child, so this should not
//!   occur in a decisive proof tree.
//!
//! An unparseable `root_fen` yields a single `NotDecisive` defect (the root
//! position cannot be established, so nothing can be proven from it).
//!
//! The validator is O(nodes) with one movegen per node, deterministic, and
//! never mutates the tree. It requires `tree` to be a well-formed finite tree
//! as produced by the worker or the binary reader; a cyclic sibling chain
//! would not terminate.

use std::collections::HashSet;

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};

use crate::notation::{move_to_uci, moves_to_uci_path};
use crate::position::{Outcome, Position};

use super::ProofTree;

/// A single structural defect found by [`validate_proof_tree`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeDefect {
    /// Root-relative move path of the defective node.
    pub path: Vec<Move>,
    pub kind: DefectKind,
    /// Human-readable context, e.g. the missing reply `d2e3`.
    pub detail: String,
}

impl std::fmt::Display for TreeDefect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "path={} kind={}: {}",
            moves_to_uci_path(&self.path),
            self.kind.as_str(),
            self.detail
        )
    }
}

/// The kind of a structural proof-tree defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefectKind {
    /// Root (or any node) is Draw or unrealized; also an unparseable root FEN.
    NotDecisive,
    /// Child of a Win node is not Loss.
    ChildNotLoss,
    /// Child of a Loss node is not Win.
    ChildNotWin,
    /// Non-terminal Win node has != 1 child.
    WinNotSingle,
    /// A legal reply has no child under it.
    LossIncomplete,
    /// Child move not legal in the replayed position.
    IllegalChildMove,
    /// Depth-0 outcome contradicts `Position::outcome_from_state`.
    TerminalMismatch,
    /// Depth != bottom-up Win=min+1 / Loss=max+1.
    DepthInconsistent,
    /// Replayed Zobrist hash != node hash (skipped when node.hash == 0).
    HashMismatch,
    /// Non-terminal node truncated by finalize's cycle guard.
    CycleLeaf,
}

impl DefectKind {
    /// Stable snake-case label used in `pt_validate:` output lines.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            DefectKind::NotDecisive => "not_decisive",
            DefectKind::ChildNotLoss => "child_not_loss",
            DefectKind::ChildNotWin => "child_not_win",
            DefectKind::WinNotSingle => "win_not_single",
            DefectKind::LossIncomplete => "loss_incomplete",
            DefectKind::IllegalChildMove => "illegal_child_move",
            DefectKind::TerminalMismatch => "terminal_mismatch",
            DefectKind::DepthInconsistent => "depth_inconsistent",
            DefectKind::HashMismatch => "hash_mismatch",
            DefectKind::CycleLeaf => "cycle_leaf",
        }
    }
}

/// Validate the structural rules of a proof over `tree`, replaying every path
/// on a real `Position` built from `tree.root_fen`.
///
/// Returns `Ok(())` for a clean proof tree, otherwise every defect found.
pub fn validate_proof_tree(tree: &ProofTree) -> Result<(), Vec<TreeDefect>> {
    let mut pos = match Position::from_fen(&tree.root_fen) {
        Ok(pos) => pos,
        Err(e) => {
            return Err(vec![TreeDefect {
                path: Vec::new(),
                kind: DefectKind::NotDecisive,
                detail: format!("root FEN failed to parse: {e}"),
            }]);
        }
    };

    let mut defects: Vec<TreeDefect> = Vec::new();
    // Bottom-up expected depths; garbage for subtrees that were not fully
    // replayed (those nodes are excluded from the depth assertion).
    let mut expected_depth = vec![0u32; tree.nodes.len()];
    // Whether a node's subtree was fully replayable (used to gate the depth
    // assertion and to propagate incompleteness to ancestors).
    let mut complete = vec![false; tree.nodes.len()];
    // Full Zobrist hashes on the current path (mirrors finalize's guard).
    let mut path_hashes: HashSet<u64> = HashSet::new();
    let mut path: Vec<Move> = Vec::new();
    let mut state = StateInfo::new();
    let mut moves = MoveList::new();

    enum Frame {
        /// Visit `id`, reached by playing `mv` (`Move::NONE` for the root).
        Enter { id: usize, mv: Move },
        /// Leave `id`: undo `mv`, drop the path hash, recompute the depth.
        Exit {
            id: usize,
            mv: Move,
            hash: Option<u64>,
            check_depth: bool,
        },
    }

    let mut stack: Vec<Frame> = vec![Frame::Enter {
        id: 0,
        mv: Move::NONE,
    }];

    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Enter { id, mv } => {
                let is_root = mv == Move::NONE && id == 0;
                let mut hash_on_path = None;
                if !is_root {
                    // The parent verified this move against its legal list,
                    // so the replay never plays an illegal move.
                    pos.do_move(mv);
                    path.push(mv);
                    let hash = pos.hash();
                    if path_hashes.contains(&hash) {
                        defects.push(TreeDefect {
                            path: path.clone(),
                            kind: DefectKind::CycleLeaf,
                            detail: "position already appears on the current path".to_string(),
                        });
                        pos.undo_move(mv);
                        path.pop();
                        continue;
                    }
                    path_hashes.insert(hash);
                    hash_on_path = Some(hash);
                }

                let node = &tree.nodes[id];
                let outcome = match node.outcome {
                    Some(o @ (Outcome::Win | Outcome::Loss)) => Some(o),
                    Some(Outcome::Draw) => {
                        defects.push(TreeDefect {
                            path: path.clone(),
                            kind: DefectKind::NotDecisive,
                            detail: "draw node; the search never emits draw events".to_string(),
                        });
                        None
                    }
                    None => {
                        defects.push(TreeDefect {
                            path: path.clone(),
                            kind: DefectKind::NotDecisive,
                            detail: "unrealized node".to_string(),
                        });
                        None
                    }
                };

                // Push the Exit frame before the children so it pops after
                // them (LIFO) and the replay state is correct for each child.
                stack.push(Frame::Exit {
                    id,
                    mv,
                    hash: hash_on_path,
                    check_depth: outcome.is_some(),
                });

                if let Some(outcome) = outcome {
                    if node.hash != 0 && node.hash != pos.hash() {
                        defects.push(TreeDefect {
                            path: path.clone(),
                            kind: DefectKind::HashMismatch,
                            detail: format!(
                                "node hash {:016x} != replayed {:016x}",
                                node.hash,
                                pos.hash()
                            ),
                        });
                    }

                    // `generate_legal_with_state` appends; clear the pooled
                    // list before refilling it for this node.
                    moves.clear();
                    pos.legal_moves_with_state(&mut moves, &mut state);
                    if node.depth == 0 {
                        let static_outcome = pos.outcome_from_state(&state, &moves);
                        if static_outcome != Some(outcome) {
                            defects.push(TreeDefect {
                                path: path.clone(),
                                kind: DefectKind::TerminalMismatch,
                                detail: format!(
                                    "depth-0 node outcome {} but replayed position yields {}",
                                    outcome,
                                    static_outcome
                                        .map_or("non-terminal".to_string(), |o| o.to_string())
                                ),
                            });
                        }
                        // Children of a depth-0 node are not descended into.
                    } else {
                        let legal: Vec<Move> = moves.as_slice().to_vec();
                        let child_ids: Vec<usize> = tree.children(id).collect();
                        if outcome == Outcome::Win && child_ids.len() != 1 {
                            defects.push(TreeDefect {
                                path: path.clone(),
                                kind: DefectKind::WinNotSingle,
                                detail: format!(
                                    "non-terminal win node has {} children",
                                    child_ids.len()
                                ),
                            });
                        }
                        // Every child of a Win node must be Loss; every
                        // child of a Loss node must be Win.
                        let (child_kind, expected_child) = match outcome {
                            Outcome::Win => (DefectKind::ChildNotLoss, Outcome::Loss),
                            _ => (DefectKind::ChildNotWin, Outcome::Win),
                        };
                        for &c in &child_ids {
                            let child = &tree.nodes[c];
                            if child.outcome != Some(expected_child) {
                                defects.push(TreeDefect {
                                    path: path.clone(),
                                    kind: child_kind,
                                    detail: format!(
                                        "child outcome {}",
                                        child.outcome.map_or("none", |o| o.as_str())
                                    ),
                                });
                            }
                        }
                        // Descend into children whose move is legal; an
                        // illegal child move cannot be replayed.
                        for &c in child_ids.iter().rev() {
                            let child_mv = tree.nodes[c].mv;
                            if legal.contains(&child_mv) {
                                stack.push(Frame::Enter {
                                    id: c,
                                    mv: child_mv,
                                });
                            } else {
                                defects.push(TreeDefect {
                                    path: path.clone(),
                                    kind: DefectKind::IllegalChildMove,
                                    detail: move_to_uci(child_mv),
                                });
                            }
                        }
                        // A Loss proof must cover all legal replies.
                        if outcome == Outcome::Loss {
                            for &reply in &legal {
                                if !child_ids.iter().any(|&c| tree.nodes[c].mv == reply) {
                                    defects.push(TreeDefect {
                                        path: path.clone(),
                                        kind: DefectKind::LossIncomplete,
                                        detail: format!("missing reply {}", move_to_uci(reply)),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            Frame::Exit {
                id,
                mv,
                hash,
                check_depth,
            } => {
                if mv != Move::NONE {
                    if let Some(h) = hash {
                        path_hashes.remove(&h);
                    }
                    path.pop();
                    pos.undo_move(mv);
                }

                let node = &tree.nodes[id];
                let child_ids: Vec<usize> = tree.children(id).collect();
                let children_complete = child_ids.iter().all(|&c| complete[c]);
                if check_depth {
                    let expected = if child_ids.is_empty() {
                        0
                    } else {
                        let best = match node.outcome {
                            Some(Outcome::Win) => child_ids
                                .iter()
                                .map(|&c| expected_depth[c])
                                .min()
                                .unwrap_or(0),
                            Some(Outcome::Loss) => child_ids
                                .iter()
                                .map(|&c| expected_depth[c])
                                .max()
                                .unwrap_or(0),
                            _ => 0,
                        };
                        best.saturating_add(1)
                    };
                    expected_depth[id] = expected;
                    if children_complete && node.depth != expected {
                        defects.push(TreeDefect {
                            path: path.clone(),
                            kind: DefectKind::DepthInconsistent,
                            detail: format!(
                                "stored depth {} != bottom-up expected {}",
                                node.depth, expected
                            ),
                        });
                    }
                }
                complete[id] = check_depth && children_complete;
            }
        }
    }

    if defects.is_empty() {
        Ok(())
    } else {
        Err(defects)
    }
}

#[cfg(test)]
mod tests;
