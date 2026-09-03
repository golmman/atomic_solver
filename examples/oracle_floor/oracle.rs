//! Oracle move-ordering scorer for the oracle-floor measurement
//! (`docs/plans/nn/plan8.md`, Step 3).
//!
//! The scorer answers "how much search work would *perfect* move ordering
//! save?" by overriding the static ordering term with the recorded proof
//! tree of the same position (pinned decision 2):
//!
//! - **OR nodes** (`outcome == Win`): the proven decisive child (a `Loss`
//!   child) is ranked first; all other recorded children are ranked next by
//!   their recorded `work` ascending (cheapest disprovals first).
//! - **AND nodes** (`outcome == Loss`): all recorded children are ranked by
//!   recorded `work` ascending.
//! - **Censored moves** (legal moves with no recorded child): ranked last,
//!   ordered by static score descending (best guess among unknowns).
//! - **Hash miss**: full static fallback, i.e. the node is ordered exactly
//!   like the baseline.
//!
//! Lookup is by exact Zobrist hash including the halfmove clock (pinned
//! decision 3); the scorer reports its per-node lookup *coverage* so that
//! low-coverage cases can be invalidated.

use std::cell::RefCell;
use std::collections::HashMap;

use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::movegen;
use atomic_movegen::types::{Move, MoveList};

use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::ProofTree;
use atomic_solver::proof_tree::binary::move_to_bits;
use atomic_solver::search::ordering::{MoveScorer, StaticAtomicScorer, nearest_commoner_map};
use atomic_solver::zobrist;

/// Score gap between consecutive oracle ranks. Larger than `HISTORY_MAX +
/// SCORE_KILLER` (60_000) so the history/killer residuals can never reorder
/// the oracle ranking.
const RANK_SCALE: i32 = 1_000_000;

/// Sentiment-free floor score for censored moves: below every recorded rank,
/// still far above the 60k history/killer range of the rank above it.
const CENSORED_SCORE: i32 = -RANK_SCALE * 60;

/// Per-position ordering resolved on first visit.
struct Visit {
    /// Move bits -> score. Oracle ranks for tree hits, static scores for
    /// fallback nodes and censored moves.
    scores: HashMap<u16, i32>,
}

#[derive(Default)]
struct Coverage {
    /// Distinct positions ordered so far.
    seen: u64,
    /// Distinct positions resolved from the oracle tree (full hash incl. rule50).
    hit: u64,
    /// Distinct positions whose *board* (rule50-ignoring) hash matches a
    /// tree node — diagnoses halfmove-clock lookup misses (decision 3).
    board_hit: u64,
}

pub struct OracleScorer {
    /// hash -> node id of the richest node carrying that hash.
    node_by_hash: HashMap<u64, usize>,
    /// board-only hash -> node id (diagnostic for clock misses).
    node_by_board_hash: HashMap<u64, usize>,
    tree: ProofTree,
    static_scorer: StaticAtomicScorer,
    visits: RefCell<HashMap<u64, Visit>>,
    coverage: RefCell<Coverage>,
}

/// Oracle order of a tree node's recorded children: decisive first (OR only,
/// cheapest first), then the rest by recorded `work` ascending. Ties break by
/// move bits for determinism.
fn oracle_child_order(tree: &ProofTree, id: usize) -> Vec<usize> {
    let mut decisive: Vec<usize> = Vec::new();
    let mut rest: Vec<usize> = Vec::new();
    let is_or = tree.nodes[id].outcome == Some(Outcome::Win);
    for c in tree.children(id) {
        if is_or && tree.nodes[c].outcome == Some(Outcome::Loss) {
            decisive.push(c);
        } else {
            rest.push(c);
        }
    }
    let key = |&c: &usize| (tree.nodes[c].work, move_to_bits(tree.nodes[c].mv));
    decisive.sort_unstable_by_key(key);
    rest.sort_unstable_by_key(key);
    decisive.extend(rest);
    decisive
}

impl OracleScorer {
    /// Index a finalized proof tree by position hash.
    ///
    /// The compact dump format stores no per-node hashes (`binary.rs`:
    /// "driver-free" — only parent ids, move codes, and `work`), so the tree
    /// is replayed from `root_fen` and every node's Zobrist hash is
    /// recomputed along its own path (same approach as `corpus_gen`). The
    /// replayed halfmove clock matches the search's whenever the position is
    /// reached by the same move path; transpositions reached via different
    /// paths may miss — that is pinned decision 3, and the scorer reports
    /// its lookup coverage so affected cases can be invalidated.
    ///
    /// When the finalize pass copied a canonical subtree onto
    /// transpositions, several nodes can share a hash; the one with the most
    /// recorded children wins (lowest id on ties) so the lookup resolves to
    /// the richest ordering information.
    pub fn new(tree: ProofTree) -> Self {
        let mut node_by_hash: HashMap<u64, usize> = HashMap::new();
        let mut node_by_board_hash: HashMap<u64, usize> = HashMap::new();
        {
            let mut pos = Position::from_fen(&tree.root_fen)
                .unwrap_or_else(|e| panic!("invalid root FEN '{}': {e}", tree.root_fen));
            enum Op {
                Enter(usize),
                Descend(usize),
                Exit(usize),
            }
            let mut stack: Vec<Op> = vec![Op::Enter(0)];
            while let Some(op) = stack.pop() {
                match op {
                    Op::Enter(id) => {
                        let children: Vec<usize> = tree.children(id).collect();
                        if children.is_empty() {
                            if id != 0 {
                                stack.push(Op::Exit(id));
                            }
                            continue;
                        }
                        let hash = pos.hash();
                        let board_hash = zobrist::board_hash(pos.board());
                        match node_by_hash.get(&hash) {
                            Some(&best) => {
                                if children.len() > tree.children(best).count() {
                                    node_by_hash.insert(hash, id);
                                    node_by_board_hash.insert(board_hash, id);
                                }
                            }
                            None => {
                                node_by_hash.insert(hash, id);
                                node_by_board_hash.entry(board_hash).or_insert(id);
                            }
                        }
                        stack.push(Op::Exit(id));
                        for &c in children.iter().rev() {
                            stack.push(Op::Descend(c));
                        }
                    }
                    Op::Descend(c) => {
                        pos.do_move(tree.nodes[c].mv);
                        stack.push(Op::Enter(c));
                    }
                    Op::Exit(id) => {
                        if id != 0 {
                            pos.undo_move(tree.nodes[id].mv);
                        }
                    }
                }
            }
        }
        Self {
            node_by_hash,
            node_by_board_hash,
            tree,
            static_scorer: StaticAtomicScorer::default(),
            visits: RefCell::new(HashMap::new()),
            coverage: RefCell::new(Coverage::default()),
        }
    }

    /// (resolved-from-tree, total distinct positions ordered, board-hash
    /// matches) so far. The board-hash count ignores the halfmove clock and
    /// diagnoses decision-3 lookup misses.
    pub fn coverage(&self) -> (u64, u64, u64) {
        let c = self.coverage.borrow();
        (c.hit, c.seen, c.board_hit)
    }

    fn build_visit(&self, board: &Board, hash: u64, is_or_node: bool) -> Visit {
        let mut coverage = self.coverage.borrow_mut();
        coverage.seen += 1;
        if self
            .node_by_board_hash
            .contains_key(&zobrist::board_hash(board))
        {
            coverage.board_hit += 1;
        }

        let mut moves = MoveList::new();
        movegen::generate_legal(board, &mut moves);
        let slice = moves.as_slice();

        let mut scores: HashMap<u16, i32> = HashMap::with_capacity(slice.len());
        if let Some(&id) = self.node_by_hash.get(&hash) {
            coverage.hit += 1;
            // Recorded children in oracle order.
            let order = oracle_child_order(&self.tree, id);
            for (rank, &c) in order.iter().enumerate() {
                scores.insert(
                    move_to_bits(self.tree.nodes[c].mv),
                    -(rank as i32) * RANK_SCALE,
                );
            }
            // Censored legal moves: no recorded work; rank them last, best
            // static guess first, so their relative order is still sensible.
            // The static profile follows the actual node type, so the
            // censored relative order matches the baseline ordering.
            let mut state = StateInfo::new();
            board.populate_state(&mut state);
            let nearest = nearest_commoner_map(board, board.side_to_move().flip());
            let mut censored: Vec<(Move, i32)> = slice
                .iter()
                .copied()
                .filter(|&m| !scores.contains_key(&move_to_bits(m)))
                .map(|m| {
                    (
                        m,
                        self.static_scorer
                            .score_with_map(board, m, &state, &nearest, is_or_node),
                    )
                })
                .collect();
            censored.sort_by_key(|&(_, s)| std::cmp::Reverse(s));
            let base = CENSORED_SCORE - censored.len() as i32;
            for (i, &(m, _)) in censored.iter().enumerate() {
                scores.insert(move_to_bits(m), base + i as i32);
            }
        } else {
            // Static fallback: identical to the baseline ordering base term.
            let mut state = StateInfo::new();
            board.populate_state(&mut state);
            let nearest = nearest_commoner_map(board, board.side_to_move().flip());
            for &m in slice {
                let s = self
                    .static_scorer
                    .score_with_map(board, m, &state, &nearest, is_or_node);
                scores.insert(move_to_bits(m), s);
            }
        }
        Visit { scores }
    }
}

impl MoveScorer for OracleScorer {
    fn score(&self, board: &Board, m: Move, _state: &StateInfo, is_or_node: bool) -> i32 {
        let hash = zobrist::hash(board, board.rule50());
        let mut visits = self.visits.borrow_mut();
        let visit = match visits.get(&hash) {
            Some(v) => v,
            None => {
                let v = self.build_visit(board, hash, is_or_node);
                visits.insert(hash, v);
                visits.get(&hash).unwrap()
            }
        };
        visit
            .scores
            .get(&move_to_bits(m))
            .copied()
            .unwrap_or(i32::MIN / 2)
    }
}
