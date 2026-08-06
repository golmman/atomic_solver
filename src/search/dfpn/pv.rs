//! PV extraction and validation.

use std::collections::HashSet;

use atomic_movegen::types::{Move, MoveList};

use crate::notation::move_to_uci;
use crate::position::{Outcome, Position};
use crate::proof_tree::{NodeProven, ProofMessage};
use crate::zobrist;

use super::Search;

impl Search {
    pub(super) fn extract_pv(&self, pos: &Position) -> Vec<Move> {
        self.extract_pv_internal(pos, None, None).0
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

    /// Rebuild the proof tree from the transposition table and emit it to the
    /// configured proof-tree sender.
    ///
    /// During iterative refinement many nodes are resolved from the TT without
    /// re-searching their descendants, so the incremental `NodeProven` events
    /// may leave the in-memory proof tree with non-terminal leaves.  This method
    /// clears the existing tree and re-emits a complete proven subtree by
    /// walking the TT directly.  The supplied `pv` is used as the principal
    /// variation so that the returned line is guaranteed to be present in the
    /// tree; other branches are expanded using the winning reply from the TT.
    pub(super) fn emit_proof_tree(
        &mut self,
        pos: &mut Position,
        root_outcome: Outcome,
        pv: &[Move],
    ) {
        if self.proof_tree_sender.is_none() || root_outcome == Outcome::Draw {
            return;
        }
        self.clear_proof_tree();
        self.emit_proof_subtree(pos, "root", Move::NONE, 0, 0usize, root_outcome, pv);
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_proof_subtree(
        &mut self,
        pos: &mut Position,
        path: &str,
        mv: Move,
        path_code: u64,
        path_length: usize,
        expected: Outcome,
        pv_tail: &[Move],
    ) -> Option<u32> {
        if let Some(terminal) = pos.outcome() {
            if terminal == expected {
                self.send_proof_node(path, mv, expected, 0);
                return Some(0);
            }
            return None;
        }

        let entry = self.tt.probe(pos.hash())?;
        let result = entry.find_result_for_path(path_code, expected)?;
        let depth = result.depth;
        self.send_proof_node(path, mv, expected, depth);

        if depth == 0 {
            return Some(0);
        }

        if expected == Outcome::Win {
            let (best_move, tail) = if let Some((&m, rest)) = pv_tail.split_first() {
                (m, rest)
            } else {
                (result.best_move, &[][..])
            };
            if best_move == Move::NONE {
                return None;
            }
            let uci = move_to_uci(best_move);
            let child_path = format!("{path}.{uci}");
            let child_path_code = path_code ^ zobrist::path_random(best_move, path_length + 1);
            pos.do_move(best_move);
            let child_depth_opt = self.emit_proof_subtree(
                pos,
                &child_path,
                best_move,
                child_path_code,
                path_length + 1,
                Outcome::Loss,
                tail,
            );
            pos.undo_move(best_move);
            let child_depth = child_depth_opt?;
            Some(child_depth + 1)
        } else {
            let mut moves = MoveList::new();
            pos.legal_moves(&mut moves);
            let mut max_child_depth = 0u32;
            let (pv_head, pv_rest) = if let Some((&m, rest)) = pv_tail.split_first() {
                (Some(m), rest)
            } else {
                (None, &[][..])
            };
            for i in 0..moves.len() {
                let mv = moves[i];
                let uci = move_to_uci(mv);
                let child_path = format!("{path}.{uci}");
                let child_path_code = path_code ^ zobrist::path_random(mv, path_length + 1);
                let child_tail = if pv_head == Some(mv) {
                    pv_rest
                } else {
                    &[][..]
                };
                pos.do_move(mv);
                if let Some(child_depth) = self.emit_proof_subtree(
                    pos,
                    &child_path,
                    mv,
                    child_path_code,
                    path_length + 1,
                    Outcome::Win,
                    child_tail,
                ) {
                    max_child_depth = max_child_depth.max(child_depth);
                }
                pos.undo_move(mv);
            }
            Some(max_child_depth + 1)
        }
    }

    fn send_proof_node(&self, path: &str, mv: Move, outcome: Outcome, depth: u32) {
        if let Some(sender) = &self.proof_tree_sender {
            let _ = sender.send(ProofMessage::NodeProven(NodeProven {
                path: path.to_string(),
                mv,
                outcome,
                depth,
            }));
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
