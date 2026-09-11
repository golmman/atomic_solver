//! The reconstruction walk: top-down resolution, hole filling, and event
//! synthesis into the proof-tree worker. See the parent module docs for the
//! algorithm and its soundness argument.
//!
//! This file is larger than the 10 KiB guideline because the walk, the fill
//! loop, and the event synthesis share one `Position`/prefix state machine
//! whose invariants (repetition-prefix alignment with the live search's path
//! stack, harvest-after-fill, finalize-only-on-complete-walk) are documented
//! next to the code that maintains them; splitting it would scatter them.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};

use crate::notation::{move_to_uci, moves_to_uci_path};
use crate::position::{Outcome, Position};
use crate::proof_event::{NodeProven, ProofEvent};
use crate::proof_tree::ProofTreeWorkerHandle;
use crate::search::dfpn::Search;
use crate::tt_snapshot::SolvedRecord;

use super::{
    ReconstructConfig, ReconstructOutput, ReconstructStats, Resolution, build_solved_map,
    classify_node, seed_search,
};

/// Fill searches are child-eval-budgeted only; give them no meaningful wall
/// clock so reconstruction stays deterministic.
const FILL_TIMEOUT_SECS: u64 = 100 * 365 * 24 * 60 * 60;

pub(super) fn run(
    root_fen: &str,
    solved: &[SolvedRecord],
    config: &ReconstructConfig,
) -> ReconstructOutput {
    let mut stats = ReconstructStats::default();
    let pos = match Position::from_fen(root_fen) {
        Ok(pos) => pos,
        Err(e) => {
            return ReconstructOutput {
                stats,
                fill_evals: 0,
                root_outcome: None,
                tree: None,
                error: Some(format!("invalid root FEN: {e}")),
            };
        }
    };
    let (map, duplicates) = build_solved_map(solved);
    stats.duplicate_keys = duplicates;

    let mut fill = Search::new(config.tt_mb);
    fill.set_timeout(FILL_TIMEOUT_SECS);
    seed_search(&mut fill, solved);

    let memory_limited = Arc::new(AtomicBool::new(false));
    let (handle, join) = ProofTreeWorkerHandle::spawn(
        root_fen.to_string(),
        config.pt_size_mb,
        Arc::clone(&memory_limited),
    );
    let event_tx = handle.event_sender();

    let mut walker = Walker {
        config,
        map,
        fill,
        event_tx,
        memory_limited: Arc::clone(&memory_limited),
        pos,
        path: Vec::new(),
        prefix: Vec::new(),
        stats,
        fill_evals: 0,
    };
    walker.prefix.push(walker.pos.repetition_key());
    let result = walker.visit(None, None);
    let root_outcome = result.as_ref().ok().copied();
    let success = result.is_ok() && !memory_limited.load(Ordering::Acquire);
    let tree = if success {
        // Finalize only after a complete walk; the worker drains the
        // remaining events first. On the abort path the worker is joined
        // cleanly without finalizing.
        handle.finalize();
        let tree = handle.tree();
        Some(tree)
    } else {
        None
    };
    let walker_fill_evals = walker.fill_evals;
    let walker_stats = std::mem::take(&mut walker.stats);
    // Tear the worker down cleanly on every path: drop all senders so the
    // worker loop sees the disconnect, then join the thread. On a failed
    // walk the worker was never finalized and its partial tree is discarded.
    drop(walker);
    drop(handle);
    let _ = join.join();

    let error = if tree.is_some() {
        None
    } else if memory_limited.load(Ordering::Acquire) && result.is_ok() {
        Some("proof-tree memory limit reached during reconstruction".to_string())
    } else {
        result.err()
    };

    ReconstructOutput {
        stats: walker_stats,
        fill_evals: walker_fill_evals,
        root_outcome,
        tree,
        error,
    }
}

struct Walker<'a> {
    config: &'a ReconstructConfig,
    map: HashMap<u64, SolvedRecord>,
    fill: Search,
    event_tx: Sender<ProofEvent>,
    memory_limited: Arc<AtomicBool>,
    pos: Position,
    path: Vec<Move>,
    /// Board (repetition) keys root..=current, inclusive.
    prefix: Vec<u64>,
    stats: ReconstructStats,
    fill_evals: u64,
}

impl Walker<'_> {
    /// Resolve the node at the current position and walk its proven subtree.
    ///
    /// `expected` is `None` for the root (any decisive outcome accepted) and
    /// the proven complement of the parent's outcome otherwise. `parent_depth`
    /// is the proven depth of the parent (`None` for the root); it seeds the
    /// fill bound for holes (a child's proven depth is `< parent_depth` by
    /// the bottom-up Win=min+1 / Loss=max+1 semantics).
    fn visit(
        &mut self,
        expected: Option<Outcome>,
        parent_depth: Option<u32>,
    ) -> Result<Outcome, String> {
        if self.memory_limited.load(Ordering::Acquire) {
            return Err("proof-tree memory limit reached during reconstruction".to_string());
        }

        // 1. Static terminal classification (mirrors the solver's terminal
        //    check; no TT dependency).
        let mut moves = MoveList::new();
        let mut state = StateInfo::new();
        self.pos.legal_moves_with_state(&mut moves, &mut state);
        if let Some(terminal) = self.pos.outcome_from_state(&state, &moves) {
            self.stats.terminal += 1;
            self.expect(expected, terminal)?;
            self.emit(terminal, 0);
            return Ok(terminal);
        }
        let legal: Vec<Move> = moves.as_slice().to_vec();

        // 2–5. Snapshot lookup, clock-scan, repetition context, or absent.
        let hash = self.pos.hash();
        let board_key = self.pos.repetition_key();
        let rep_on_prefix = self.prefix[..self.prefix.len() - 1].contains(&board_key);
        let (outcome, depth, record_best, filled) = match classify_node(
            &self.map,
            hash,
            board_key,
            rep_on_prefix,
        ) {
            Resolution::Hit(record) => {
                self.stats.hit += 1;
                self.expect(expected, record.outcome)?;
                (record.outcome, record.depth, record.best_move, false)
            }
            // A clock-scan hit adopts a Win/Loss record stored under a
            // different clock of the same board; its best move belongs to the
            // same board (castling/ep are part of the board hash), so it is
            // legal here too and is used directly — an exact-hash re-lookup
            // would miss.
            Resolution::ClockHit(record) => {
                self.stats.clock_hit += 1;
                self.expect(expected, record.outcome)?;
                (record.outcome, record.depth, record.best_move, false)
            }
            Resolution::ClockMissDraw => {
                self.stats.clock_miss_draw += 1;
                let (outcome, depth) = self.fill_hole(expected, parent_depth)?;
                (outcome, depth, Move::NONE, true)
            }
            Resolution::Repetition => {
                self.stats.repetition += 1;
                self.stats.anomalies += 1;
                return Err(format!(
                    "anomaly: repetition-context node at {} (a GHI draw cannot be a child inside a valid proof)",
                    moves_to_uci_path(&self.path)
                ));
            }
            Resolution::Absent => {
                self.stats.absent += 1;
                let (outcome, depth) = self.fill_hole(expected, parent_depth)?;
                (outcome, depth, Move::NONE, true)
            }
        };

        // A filled Win descends via the best move harvested from the fill
        // search's TT; a filled Loss expands all legal replies like a hit.
        let best_move = if filled {
            match self.map.get(&hash).filter(|r| r.outcome == outcome) {
                Some(record) => record.best_move,
                None => {
                    self.stats.anomalies += 1;
                    return Err(format!(
                        "anomaly: filled node at {} has no harvested record",
                        moves_to_uci_path(&self.path)
                    ));
                }
            }
        } else {
            record_best
        };

        self.emit(outcome, depth);

        match outcome {
            Outcome::Win => {
                self.descend_best(best_move, depth)?;
                Ok(Outcome::Win)
            }
            Outcome::Loss => {
                for mv in legal {
                    self.step(mv, Outcome::Win, depth)?;
                }
                Ok(Outcome::Loss)
            }
            // `expect` rejects Draw before this point.
            Outcome::Draw => unreachable!("Draw cannot pass the expectation check"),
        }
    }

    /// Validate the resolution against the parent's proof requirement.
    fn expect(&mut self, expected: Option<Outcome>, resolved: Outcome) -> Result<(), String> {
        match expected {
            None => {
                if resolved == Outcome::Draw {
                    return Err("snapshot does not prove a decisive root".to_string());
                }
                Ok(())
            }
            Some(expected) if expected == resolved => Ok(()),
            Some(expected) => {
                self.stats.anomalies += 1;
                Err(format!(
                    "anomaly: node at {} resolved {resolved} but {expected} required",
                    moves_to_uci_path(&self.path)
                ))
            }
        }
    }

    /// Fill a hole at the current node with an iterative-doubling bounded
    /// local solve that preserves the walk's repetition history. Returns the
    /// proven outcome and its advisory depth (win depth for a Win, the
    /// successful bound for a Loss).
    fn fill_hole(
        &mut self,
        expected: Option<Outcome>,
        parent_depth: Option<u32>,
    ) -> Result<(Outcome, u32), String> {
        let mut bound = parent_depth.unwrap_or(self.config.fill_base).max(1);
        loop {
            if bound > self.config.fill_depth_cap {
                self.stats.unfillable += 1;
                return Err(format!(
                    "unfillable hole at {}: fill depth cap {} exhausted",
                    moves_to_uci_path(&self.path),
                    self.config.fill_depth_cap
                ));
            }
            if self.config.fill_total_budget > 0 && self.fill_evals >= self.config.fill_total_budget
            {
                return Err("BudgetExhausted: fill total budget exhausted".to_string());
            }
            // The fill's prefix is the walk's repetition-key stack minus the
            // hole's own key: the positions *before* the hole, exactly what
            // the live search's path stack held at this node's frame.
            let prefix: Vec<u64> = self.prefix[..self.prefix.len() - 1].to_vec();
            self.fill
                .set_child_eval_budget(self.config.fill_attempt_budget);
            let (outcome, win_depth, _nodes) =
                self.fill
                    .search_depth_with_prefix(&mut self.pos, bound, &prefix);
            // `search_depth_with_prefix` resets `child_evals` per attempt.
            self.fill_evals += self.fill.child_evaluations();
            match outcome {
                Outcome::Draw => bound = bound.saturating_mul(2),
                outcome => {
                    if let Some(expected) = expected
                        && outcome != expected
                    {
                        self.stats.anomalies += 1;
                        return Err(format!(
                            "anomaly: fill at {} resolved {outcome} but {expected} required",
                            moves_to_uci_path(&self.path)
                        ));
                    }
                    // Harvest the fill's solved interior into the walk map
                    // (first-wins); this converts the whole proven subtree
                    // into exact hits and shrinks future holes.
                    self.harvest();
                    self.stats.filled += 1;
                    let depth = if outcome == Outcome::Win {
                        win_depth
                    } else {
                        bound
                    };
                    return Ok((outcome, depth));
                }
            }
        }
    }

    /// Copy solved entries from the fill search's TT into the walk map.
    fn harvest(&mut self) {
        for entry in self.fill.tt().entries() {
            if let Some(outcome) = entry.outcome {
                self.map.entry(entry.key).or_insert(SolvedRecord {
                    key: entry.key,
                    outcome,
                    depth: entry.depth,
                    best_move: entry.best_move,
                });
            }
        }
    }

    /// Validate a Win node's best move and descend into it. The node's own
    /// proven outcome does not depend on the child walk.
    fn descend_best(&mut self, best_move: Move, depth: u32) -> Result<(), String> {
        if best_move == Move::NONE {
            self.stats.anomalies += 1;
            return Err(format!(
                "anomaly: non-terminal Win node at {} has best_move NONE",
                moves_to_uci_path(&self.path)
            ));
        }
        if !self.pos.legal_moves_vec().contains(&best_move) {
            self.stats.anomalies += 1;
            return Err(format!(
                "anomaly: Win best_move {} is not legal at {}",
                move_to_uci(best_move),
                moves_to_uci_path(&self.path)
            ));
        }
        self.step(best_move, Outcome::Loss, depth)
    }

    /// Play `mv`, visit the child, and undo. The child's repetition key is
    /// pushed so its own visit and any nested fill see the walk's history.
    fn step(&mut self, mv: Move, child_expected: Outcome, parent_depth: u32) -> Result<(), String> {
        self.path.push(mv);
        self.pos.do_move(mv);
        self.prefix.push(self.pos.repetition_key());
        let result = self
            .visit(Some(child_expected), Some(parent_depth))
            .map(|_| ());
        self.prefix.pop();
        self.pos.undo_move(mv);
        self.path.pop();
        result
    }

    /// Synthesize one `NodeProven` event into the worker. The root is emitted
    /// with an empty path (`Move::NONE`), exactly like the live search.
    fn emit(&self, outcome: Outcome, depth: u32) {
        let event = ProofEvent::NodeProven(NodeProven::new(
            self.path.clone(),
            self.pos.hash(),
            outcome,
            depth,
        ));
        let _ = self.event_tx.send(event);
    }
}
