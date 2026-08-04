//! PV extraction and validation.

use std::collections::HashSet;

use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::zobrist;

use super::Search;

impl Search {
    pub(super) fn extract_pv(&self, pos: &Position) -> Vec<Move> {
        self.extract_pv_internal(pos, None, None).0
    }

    pub(super) fn extract_ppv(&self, pos: &Position, expected: Outcome) -> Option<Vec<Move>> {
        let (pv, truncated) = self.extract_pv_internal(pos, Some(expected), None);
        if truncated {
            return None;
        }
        if Self::validate_pv(&pv, pos, expected, None) {
            Some(pv)
        } else {
            None
        }
    }

    pub(super) fn extract_pv_checked(
        &self,
        pos: &Position,
        expected: Outcome,
        expected_depth: Option<u32>,
    ) -> Option<Vec<Move>> {
        let (pv, truncated) = self.extract_pv_internal(pos, Some(expected), expected_depth);
        if let Some(d) = expected_depth
            && pv.len() as u32 != d
        {
            eprintln!(
                "warning: extracted PV length {} does not match stored depth {d} for {expected:?}",
                pv.len()
            );
        }
        if truncated {
            if Self::validate_pv_prefix(&pv, pos).is_some() {
                eprintln!("warning: PV truncated after {} plies", self.max_ply);
                return Some(pv);
            }
            eprintln!("warning: PV validation failed for {expected:?}");
            return None;
        }
        if Self::validate_pv(&pv, pos, expected, expected_depth) {
            Some(pv)
        } else {
            eprintln!("warning: PV validation failed for {expected:?}");
            None
        }
    }

    pub fn validate_pv(
        pv: &[Move],
        pos: &Position,
        expected: Outcome,
        expected_depth: Option<u32>,
    ) -> bool {
        if let Some(d) = expected_depth
            && pv.len() as u32 != d
        {
            return false;
        }

        let current = match Self::validate_pv_prefix(pv, pos) {
            Some(c) => c,
            None => return false,
        };

        // `Position::outcome()` is the canonical terminal detector, including
        // commoner extinction, rule50, the two-piece draw, checkmate, and stalemate.
        let final_outcome = current.outcome();

        // Outcome is from the side-to-move perspective. After `pv.len()` plies,
        // the side to move is the original player on even lengths and the
        // opponent on odd lengths, so the terminal outcome must be adjusted.
        let final_expected = if pv.len().is_multiple_of(2) {
            expected
        } else {
            expected.flip()
        };
        final_outcome == Some(final_expected)
    }

    fn validate_pv_prefix(pv: &[Move], pos: &Position) -> Option<Position> {
        let mut current = pos.clone();
        for &m in pv {
            let mut moves = MoveList::new();
            current.legal_moves(&mut moves);

            let mut legal = false;
            for i in 0..moves.len() {
                if moves[i] == m {
                    legal = true;
                    break;
                }
            }
            if !legal {
                return None;
            }
            current.do_move(m);
        }
        Some(current)
    }

    fn extract_pv_internal(
        &self,
        pos: &Position,
        mut expected: Option<Outcome>,
        expected_depth: Option<u32>,
    ) -> (Vec<Move>, bool) {
        let mut pv = Vec::new();
        let mut seen = HashSet::new();
        let mut current = pos.clone();
        let mut path_code = 0u64;
        let mut remaining = expected_depth;

        for _ in 0..self.max_ply {
            let tt_key = current.hash();
            let rep_key = current.repetition_key();
            if seen.contains(&rep_key) {
                break;
            }
            if current.outcome().is_some() {
                break;
            }

            let entry = match self.tt.probe(tt_key) {
                Some(e) => e,
                None => break,
            };

            let node_expected = match expected {
                Some(e) => e,
                None => {
                    if let Some((_, Some(o), _)) = entry.best_result_for_path(path_code) {
                        expected = Some(o);
                        o
                    } else {
                        break;
                    }
                }
            };

            let node_remaining = if let Some(r) = remaining {
                r
            } else if let Some(r) = entry.find_result_for_path(path_code, node_expected) {
                r.depth
            } else if let Some((_, Some(o), d)) = entry.best_result_for_path(path_code) {
                if o == node_expected {
                    d
                } else {
                    break;
                }
            } else {
                break;
            };

            // Prefer an entry whose stored depth matches the remaining plies.
            // Fall back to any entry with the expected outcome so extraction
            // still succeeds when the depth-aware entry is missing.
            let result = if let Some(r) =
                entry.find_result_for_path_with_depth(path_code, node_expected, node_remaining)
            {
                Some(r)
            } else {
                entry.find_result_for_path(path_code, node_expected)
            };

            if let Some(r) = result {
                let mv = r.best_move;
                if mv == Move::NONE {
                    break;
                }
                seen.insert(rep_key);
                pv.push(mv);
                current.do_move(mv);
                path_code ^= zobrist::path_random(mv, pv.len());
                expected = expected.map(Outcome::flip);
                remaining = Some(r.depth.saturating_sub(1));
            } else {
                break;
            }
        }

        let truncated = pv.len() == self.max_ply && current.outcome().is_none();
        (pv, truncated)
    }

    /// Reconstruct a PPV by walking the proven subtree itself.
    ///
    /// This pass does not read the transposition table or trust stored
    /// `best_move` hints.  It evaluates every legal child, selects the child
    /// that matches the minimax PPV rule (shortest attacker win, longest
    /// defender resistance), and recurses.  Repetitions are detected with the
    /// same `path_stack` convention as `dfpn`.  Results are memoized in
    /// `Search::ppv_cache`.
    #[cfg(test)]
    pub(super) fn extract_ppv_from_proven_subtree(
        &mut self,
        pos: &mut Position,
        expected: Outcome,
        remaining: u32,
    ) -> Option<(Vec<Move>, u32)> {
        self.extract_ppv_from_proven_subtree_impl(pos, expected, remaining, false)
    }

    pub(super) fn extract_ppv_from_proven_subtree_emit(
        &mut self,
        pos: &mut Position,
        expected: Outcome,
        remaining: u32,
    ) -> Option<(Vec<Move>, u32)> {
        self.extract_ppv_from_proven_subtree_impl(pos, expected, remaining, true)
    }

    fn extract_ppv_from_proven_subtree_impl(
        &mut self,
        pos: &mut Position,
        expected: Outcome,
        remaining: u32,
        emit: bool,
    ) -> Option<(Vec<Move>, u32)> {
        let key = (pos.hash(), self.path_code, expected);
        if let Some((cached_pv, cached_depth)) = self.ppv_cache.get(&key) {
            if *cached_depth <= remaining {
                return Some((cached_pv.clone(), *cached_depth));
            }
            return None;
        }

        if self.time_exceeded() {
            return None;
        }

        if let Some(outcome) = pos.outcome() {
            self.path_pop();
            if outcome == expected {
                if emit {
                    self.emit_proof_node(true, expected, 0);
                }
                return Some((Vec::new(), 0));
            }
            return None;
        }

        if remaining == 0 {
            self.path_pop();
            return None;
        }

        let rep_key = pos.repetition_key();
        if self.path_contains(rep_key) {
            return None;
        }

        self.path_push(rep_key);

        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);

        let best_from_tt = if expected == Outcome::Win {
            self.tt
                .probe_best_move(pos.hash(), self.path_code)
                .unwrap_or(Move::NONE)
        } else {
            Move::NONE
        };
        self.sort_moves(pos, &mut moves, best_from_tt);

        let mut best: Option<(Move, Vec<Move>, u32)> = None;

        for i in 0..moves.len() {
            let mv = moves[i];

            // Alpha-beta bound for the attacker: to beat the current best
            // the child must win in strictly fewer plies.  For the defender
            // every reply must be checked, so use the full remaining budget.
            let child_bound = if expected == Outcome::Win {
                best.as_ref()
                    .map_or(remaining.saturating_sub(1), |(_, _, total)| {
                        total.saturating_sub(2)
                    })
            } else {
                remaining.saturating_sub(1)
            };
            if child_bound == 0 && best.is_some() && expected == Outcome::Win {
                // Can't improve on a 1-ply win.
                break;
            }

            let proof_len = self.proof_path.len();
            let uci = crate::notation::move_to_uci(mv);
            self.proof_path.push('.');
            self.proof_path.push_str(&uci);
            self.move_stack.push(mv);

            pos.do_move(mv);
            self.path_code ^= zobrist::path_random(mv, self.path_stack.len());

            let child_result = if let Some(outcome) = pos.outcome() {
                if outcome == expected.flip() {
                    if emit {
                        self.emit_proof_node(true, outcome, 0);
                    }
                    Some((Vec::new(), 0))
                } else {
                    None
                }
            } else if child_bound > 0 {
                self.extract_ppv_from_proven_subtree_impl(pos, expected.flip(), child_bound, emit)
            } else {
                None
            };

            self.path_code ^= zobrist::path_random(mv, self.path_stack.len());
            pos.undo_move(mv);

            self.move_stack.pop();
            self.proof_path.truncate(proof_len);

            match (expected, child_result) {
                (Outcome::Win, Some((child_pv, child_depth))) => {
                    let total = 1 + child_depth;
                    if best.is_none() || total < 1 + best.as_ref().unwrap().2 {
                        let mut pv = Vec::with_capacity(1 + child_pv.len());
                        pv.push(mv);
                        pv.extend(child_pv);
                        best = Some((mv, pv, child_depth));
                    }
                    if total == 1 {
                        break;
                    }
                }
                (Outcome::Loss, None) => {
                    self.path_pop();
                    return None;
                }
                (Outcome::Loss, Some((child_pv, child_depth))) => {
                    let total = 1 + child_depth;
                    if best.is_none() || total > 1 + best.as_ref().unwrap().2 {
                        let mut pv = Vec::with_capacity(1 + child_pv.len());
                        pv.push(mv);
                        pv.extend(child_pv);
                        best = Some((mv, pv, child_depth));
                    }
                }
                _ => {}
            }
        }

        self.path_pop();

        if let Some((_, pv, child_depth)) = best {
            let result = (pv, 1 + child_depth);
            self.ppv_cache.insert(key, result.clone());
            if result.1 <= remaining {
                if emit {
                    self.emit_proof_node(true, expected, result.1);
                }
                Some(result)
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Search;
    use crate::position::{Outcome, Position};
    use crate::zobrist;
    use atomic_movegen::types::{Move, Square};

    #[test]
    fn validate_pv_accepts_valid_win() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        let pv = vec![Move::make_move(Square::E1, Square::E8)];
        assert!(Search::validate_pv(&pv, &pos, Outcome::Win, Some(1)));
        // Depth mismatch should fail.
        assert!(!Search::validate_pv(&pv, &pos, Outcome::Win, Some(2)));
        // Wrong expected outcome should fail.
        assert!(!Search::validate_pv(&pv, &pos, Outcome::Loss, Some(1)));
    }

    #[test]
    fn validate_pv_rejects_illegal_move() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let pv = vec![Move::make_move(Square::E1, Square::E8)];
        assert!(!Search::validate_pv(&pv, &pos, Outcome::Win, None));
    }

    #[test]
    fn validate_pv_rejects_wrong_terminal_outcome() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let pv = vec![];
        assert!(Search::validate_pv(&pv, &pos, Outcome::Draw, None));
        assert!(!Search::validate_pv(&pv, &pos, Outcome::Win, None));
    }

    #[test]
    fn pv_truncation_warns_and_keeps_outcome() {
        let fen = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";
        let mut pos = Position::from_fen(fen).unwrap();
        let mut search = Search::new(64);
        search.set_timeout(5);
        search.set_max_ply(2);
        let (outcome, pv, _nodes) = search.solve(&mut pos);
        assert_eq!(
            outcome,
            Outcome::Win,
            "truncation must not change the outcome"
        );
        assert_eq!(pv.len(), 2, "PV should be truncated to max_ply");
    }

    #[test]
    fn extract_pv_follows_path_dependent_twin_entries() {
        // Solve a short forced-mate position, then re-store every node along the
        // principal variation as a path-dependent twin. This exercises the
        // exact 1-indexed path-code arithmetic that `extract_pv` must share with
        // `dfpn`.
        let fen = "rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3";
        let mut pos = Position::from_fen(fen).unwrap();
        let mut search = Search::new(64);
        search.set_timeout(5);

        let (outcome, pv, _nodes) = search.solve(&mut pos);
        assert_eq!(outcome, Outcome::Loss, "expected a forced loss for black");
        assert!(!pv.is_empty(), "expected a non-empty PV");

        // Re-store each node as a twin keyed by the 1-indexed path code.
        let mut current = Position::from_fen(fen).unwrap();
        let mut path_code = 0u64;
        for (i, &mv) in pv.iter().enumerate() {
            let key = current.hash();
            let expected = if i % 2 == 0 {
                Outcome::Loss
            } else {
                Outcome::Win
            };
            let (pn, dn) = expected.to_pn_dn();
            let remaining = (pv.len() - i) as u32;
            search.tt.store(
                key,
                mv,
                u8::MAX,
                0,
                Some(expected),
                pn,
                dn,
                remaining,
                u32::MAX,
                path_code,
                i as u32,
                true,
            );
            current.do_move(mv);
            path_code ^= zobrist::path_random(mv, i + 1);
        }

        let extracted = search.extract_pv(&Position::from_fen(fen).unwrap());
        assert_eq!(
            extracted, pv,
            "extract_pv must follow twin entries using 1-indexed path codes"
        );
    }

    #[test]
    fn extract_ppv_from_proven_subtree_finds_shortest_win() {
        let fen = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";
        let mut pos = Position::from_fen(fen).unwrap();
        let mut search = Search::new(64);
        search.set_timeout(30);

        let (outcome, _pv, _nodes) = search.solve(&mut pos);
        assert_eq!(outcome, Outcome::Win);

        let mut start = Position::from_fen(fen).unwrap();
        let result = search.extract_ppv_from_proven_subtree(&mut start, outcome, 10);
        assert!(result.is_some(), "expected to find a proven PPV");
        let (pv, depth) = result.unwrap();
        assert_eq!(depth, 3, "expected the 3-plies mate depth, got {pv:?}");
        assert_eq!(pv.len(), 3, "expected PV of length 3, got {pv:?}");
    }

    #[test]
    fn validate_pv_accepts_three_ply_mate() {
        use crate::notation::uci_to_move;
        let fen = "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1";
        let pos = Position::from_fen(fen).unwrap();
        let mut replay = pos.clone();
        let uci = ["f1f7", "e8d8", "g1g8"];
        let mut pv = Vec::with_capacity(uci.len());
        for &s in &uci {
            let mv = uci_to_move(s, &replay).expect("legal move");
            replay.do_move(mv);
            pv.push(mv);
        }
        assert!(Search::validate_pv(&pv, &pos, Outcome::Win, None));
    }
}
