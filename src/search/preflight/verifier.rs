//! Replay verifier for pre-phase WIN certificates (plan13 G-B, defense in
//! depth).
//!
//! A decisive pre-phase claim is only returned when its rank-decreasing
//! strategy replays, from the real root position with the solver's own
//! movegen, as a **cycle-free forced win within the rank bound**: at
//! winner-to-move nodes the certified move is played, at loser-to-move nodes
//! *every* legal reply is covered, every line ends in a terminal win for the
//! winner within `bound` plies, and no board repeats anywhere on any line.
//!
//! This is the plan's defense-in-depth gate (R3): a cyclic position cannot
//! yield a cycle-free certificate, so even a mis-gated position degrades to
//! "deferred" instead of a false decisive claim. The verifier is
//! self-contained on purpose: it consumes only the root `Position` and the
//! strategy map, never the closure internals. The file is slightly larger
//! than the 10 KiB guideline because the walk and its negative-case unit
//! tests (missing/corrupted entries, injected cycles, bound and budget
//! exits) live together by the project's unit-test convention; it stays
//! well under the ~20 KiB split threshold.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use atomic_movegen::types::{Color, Move, MoveList};

use crate::position::{Outcome, Position};

use super::region::{classify_terminal, region_key};

/// Replay statistics (diagnostics for the `cert-fail` deferral path).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct CertStats {
    pub lines: u64,
    pub mates: u64,
    pub repeats: u64,
    pub budget_exits: u64,
    pub bad_strategy: u64,
    pub nodes: u64,
    pub max_plies: u32,
    /// Set when the eval budget or the stop flag ended the replay early; the
    /// claim is then deferred, not failed.
    pub aborted: bool,
}

/// Verify a winning strategy from `root` under the path-repetition
/// semantics.
///
/// `winner` is the color whose win is claimed; `bound` is the certificate
/// line-length bound (the exact rank for the region-closure architecture).
/// `evals`/`eval_budget` account and bound the replay's child evaluations
/// (one per played move); `stop` is checked between subtrees. Returns
/// `(ok, stats)`; `ok == false` with `stats.aborted` means "insufficient
/// budget", otherwise "defective certificate" — both defer, neither claims.
pub(super) fn verify_certificate(
    root: &Position,
    strategy: &HashMap<u32, u32>,
    winner: Color,
    bound: u32,
    evals: &mut u64,
    eval_budget: u64,
    stop: Option<&AtomicBool>,
) -> (bool, CertStats) {
    let mut stats = CertStats::default();
    let mut path: Vec<u32> = Vec::with_capacity(bound as usize + 1);
    if let Some(key) = region_key(root.board()) {
        path.push(key);
    } else {
        stats.bad_strategy += 1;
        return (false, stats);
    }
    let mut pos = root.clone();
    let ok = replay_walk(
        &mut pos,
        0,
        &mut path,
        strategy,
        winner,
        bound,
        evals,
        eval_budget,
        stop,
        &mut stats,
    );
    (ok, stats)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_walk(
    pos: &mut Position,
    plies: u32,
    path: &mut Vec<u32>,
    strategy: &HashMap<u32, u32>,
    winner: Color,
    bound: u32,
    evals: &mut u64,
    eval_budget: u64,
    stop: Option<&AtomicBool>,
    stats: &mut CertStats,
) -> bool {
    stats.nodes += 1;
    if stats.nodes.is_multiple_of(4096)
        && let Some(flag) = stop
        && flag.load(Ordering::Acquire)
    {
        stats.aborted = true;
        return false;
    }
    // Terminal classification is clock-independent (extinction, moves-empty,
    // two-piece draw) and the rank guard keeps every line below clock 100,
    // so the board-only classifier is exact here.
    let us = pos.side_to_move();
    if let Some(t) = classify_terminal(pos.board()) {
        let winner_won =
            (us == winner && t == Outcome::Win) || (us != winner && t == Outcome::Loss);
        if winner_won {
            stats.lines += 1;
            stats.mates += 1;
            stats.max_plies = stats.max_plies.max(plies);
            return true;
        }
        stats.bad_strategy += 1;
        return false;
    }
    if plies >= bound {
        stats.budget_exits += 1;
        return false;
    }
    let Some(key) = region_key(pos.board()) else {
        stats.bad_strategy += 1;
        return false;
    };
    let mut moves = MoveList::new();
    pos.legal_moves(&mut moves);
    if us == winner {
        let Some(&chosen) = strategy.get(&key) else {
            stats.bad_strategy += 1;
            return false;
        };
        // Find the certified move by playing each candidate and matching the
        // exact child key (one child evaluation per candidate).
        let mut found: Option<Move> = None;
        for i in 0..moves.len() {
            *evals += 1;
            if *evals >= eval_budget {
                stats.aborted = true;
                return false;
            }
            pos.do_move(moves[i]);
            let child_key = region_key(pos.board());
            pos.undo_move(moves[i]);
            if child_key == Some(chosen) {
                found = Some(moves[i]);
                break;
            }
        }
        let Some(mv) = found else {
            stats.bad_strategy += 1;
            return false;
        };
        if path.contains(&chosen) {
            stats.repeats += 1;
            return false;
        }
        path.push(chosen);
        pos.do_move(mv);
        let ok = replay_walk(
            pos,
            plies + 1,
            path,
            strategy,
            winner,
            bound,
            evals,
            eval_budget,
            stop,
            stats,
        );
        pos.undo_move(mv);
        path.pop();
        ok
    } else {
        // Loser to move: every reply must be covered; stop at the first
        // failing line (the verdict is false regardless).
        for i in 0..moves.len() {
            *evals += 1;
            if *evals >= eval_budget {
                stats.aborted = true;
                return false;
            }
            pos.do_move(moves[i]);
            let child_key = region_key(pos.board());
            if let Some(child_key) = child_key {
                if path.contains(&child_key) {
                    pos.undo_move(moves[i]);
                    stats.repeats += 1;
                    return false;
                }
                path.push(child_key);
            }
            let ok = replay_walk(
                pos,
                plies + 1,
                path,
                strategy,
                winner,
                bound,
                evals,
                eval_budget,
                stop,
                stats,
            );
            pos.undo_move(moves[i]);
            if child_key.is_some() {
                path.pop();
            }
            if !ok {
                return false;
            }
        }
        stats.lines += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;
    use atomic_movegen::types::{Color, Move, Square};

    /// White to move; the certified strategy `Qb2-b1` forces the black king
    /// into... nothing immediate — this root is a *draw* per the engine's
    /// adjacency-immunity semantics, which makes it ideal for negative
    /// verifier tests: a claimed White win with this strategy must fail
    /// replay (the defender escapes), independent of the game value.
    const WIN_ROOT_FEN: &str = "8/8/8/8/8/1K6/1Q6/k7 w - - 0 1";

    fn legal_move(pos: &Position, from: u8, to: u8) -> Move {
        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);
        let (f, t) = (Square::from_u8(from), Square::from_u8(to));
        moves
            .as_slice()
            .iter()
            .find(|m| m.from_sq() == f && m.to_sq() == t)
            .copied()
            .unwrap_or_else(|| panic!("move {from}->{to} must be legal"))
    }

    fn child_key_after(pos: &Position, from: u8, to: u8) -> u32 {
        let mut cur = pos.clone();
        cur.do_move(legal_move(pos, from, to));
        region_key(cur.board()).expect("three men")
    }

    fn valid_strategy(pos: &Position) -> std::collections::HashMap<u32, u32> {
        let root_key = region_key(pos.board()).unwrap();
        let b1_child = child_key_after(pos, 9, 1); // b2 -> b1
        let mut strategy = std::collections::HashMap::new();
        strategy.insert(root_key, b1_child);
        strategy
    }

    #[test]
    fn verifier_rejects_a_strategy_the_defender_escapes() {
        // The queen leaves the contact square; the defender reply that steps
        // adjacent to the white king gains adjacency immunity and survives,
        // so any bound must eventually expose the hole. This is the
        // positive-control negative: an unsound "strategy" fails replay.
        let pos = Position::from_fen(WIN_ROOT_FEN).unwrap();
        let strategy = valid_strategy(&pos);
        let (ok, stats) =
            verify_certificate(&pos, &strategy, Color::White, 6, &mut 0, u64::MAX, None);
        assert!(!ok, "the defender escape must fail the certificate");
        assert!(!stats.aborted);
        assert!(stats.repeats + stats.bad_strategy + stats.budget_exits >= 1);
    }

    #[test]
    fn verifier_rejects_a_missing_entry() {
        let pos = Position::from_fen(WIN_ROOT_FEN).unwrap();
        let strategy = std::collections::HashMap::new();
        let (ok, stats) =
            verify_certificate(&pos, &strategy, Color::White, 4, &mut 0, u64::MAX, None);
        assert!(!ok);
        assert!(stats.bad_strategy >= 1, "the winner node needs an entry");
        assert!(!stats.aborted);
    }

    #[test]
    fn verifier_rejects_a_corrupted_entry() {
        let pos = Position::from_fen(WIN_ROOT_FEN).unwrap();
        let root_key = region_key(pos.board()).unwrap();
        let mut strategy = std::collections::HashMap::new();
        // Points at no legal child of the root.
        strategy.insert(root_key, u32::MAX);
        let (ok, stats) =
            verify_certificate(&pos, &strategy, Color::White, 4, &mut 0, u64::MAX, None);
        assert!(!ok);
        assert!(stats.bad_strategy >= 1);
    }

    #[test]
    fn verifier_rejects_an_injected_cycle() {
        // Seed the replay path so that a line returns to a key already on
        // it: the cycle check must fail the certificate.
        let pos = Position::from_fen(WIN_ROOT_FEN).unwrap();
        let root_key = region_key(pos.board()).unwrap();
        let strategy = valid_strategy(&pos);

        // Path: root -> certified child -> black's first reply; replaying
        // the certified strategy from the root then hits the seeded reply
        // key at the loser node's first branch.
        let mut cur = pos.clone();
        cur.do_move(legal_move(&pos, 9, 1));
        let mut replies = MoveList::new();
        cur.legal_moves(&mut replies);
        assert!(!replies.is_empty());
        cur.do_move(replies[0]);
        let seeded = region_key(cur.board()).unwrap();

        let mut path = vec![root_key, seeded];
        let mut stats = CertStats::default();
        let mut evals = 0u64;
        let mut walk_pos = pos.clone();
        let ok = replay_walk(
            &mut walk_pos,
            0,
            &mut path,
            &strategy,
            Color::White,
            6,
            &mut evals,
            u64::MAX,
            None,
            &mut stats,
        );
        assert!(!ok, "the injected cycle must fail the certificate");
        assert_eq!(stats.repeats, 1, "the failure must be the detected repeat");
    }

    #[test]
    fn verifier_rejects_a_line_exceeding_the_bound() {
        let pos = Position::from_fen(WIN_ROOT_FEN).unwrap();
        let strategy = valid_strategy(&pos);
        // bound 1: the certified line needs 2+ plies, so it hits the bound.
        let (ok, stats) =
            verify_certificate(&pos, &strategy, Color::White, 1, &mut 0, u64::MAX, None);
        assert!(!ok);
        assert!(stats.budget_exits >= 1);
    }

    #[test]
    fn verifier_eval_budget_aborts_without_failing_the_strategy() {
        let pos = Position::from_fen(WIN_ROOT_FEN).unwrap();
        let strategy = valid_strategy(&pos);
        let mut evals = 0u64;
        let (ok, stats) = verify_certificate(&pos, &strategy, Color::White, 4, &mut evals, 0, None);
        assert!(!ok);
        assert!(stats.aborted, "exhausted budget aborts, never fails");
        assert_eq!(evals, 1, "the budget check fires after the first eval");
    }
}
