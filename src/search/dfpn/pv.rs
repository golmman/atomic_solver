//! PV extraction and validation.

use std::collections::HashSet;

use atomic_movegen::types::{Move, MoveList};

use crate::notation::move_to_uci;
use crate::position::{Outcome, Position};
use crate::proof_tree::{NodeProven, ProofMessage};

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
        let mut remaining = expected_depth;

        for _ in 0..self.max_ply {
            let rep_key = current.repetition_key();
            if seen.contains(&rep_key) {
                break;
            }
            if current.outcome().is_some() {
                break;
            }

            let entry = match self.tt.probe(current.hash()) {
                Some(e) => e,
                None => break,
            };

            let node_expected = match expected {
                Some(e) => e,
                None => {
                    if let Some((_, o, _)) = entry.best_result() {
                        expected = Some(o);
                        o
                    } else {
                        break;
                    }
                }
            };

            let node_remaining = if let Some(r) = remaining {
                r
            } else if let Some(r) = entry.result_for(node_expected) {
                r.depth
            } else {
                break;
            };

            // Prefer an entry whose stored depth matches the remaining plies.
            // Fall back to any entry with the expected outcome so extraction
            // still succeeds when the depth-aware entry is missing.
            let result = if let Some(r) = entry.result_for_depth(node_expected, node_remaining) {
                Some(r)
            } else {
                entry.result_for(node_expected)
            };

            if let Some(r) = result {
                let mv = r.best_move;
                if mv == Move::NONE {
                    break;
                }
                seen.insert(rep_key);
                pv.push(mv);
                current.do_move(mv);
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
        let _ = self.emit_proof_subtree(pos, "root", Move::NONE, root_outcome, pv);
    }

    fn emit_proof_subtree(
        &mut self,
        pos: &mut Position,
        path: &str,
        mv: Move,
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
        let result = entry.result_for(expected)?;
        let depth = result.depth;
        self.send_proof_node(path, mv, expected, depth);

        // A terminal cached result has no children to expand.
        if depth == 0 {
            return Some(0);
        }

        if expected == Outcome::Win {
            let (best_move, tail) = if let Some((&m, rest)) = pv_tail.split_first() {
                (m, rest)
            } else {
                (result.best_move, &[][..])
            };
            if best_move != Move::NONE {
                let uci = move_to_uci(best_move);
                let child_path = format!("{path}.{uci}");
                pos.do_move(best_move);
                let _ = self.emit_proof_subtree(pos, &child_path, best_move, Outcome::Loss, tail);
                pos.undo_move(best_move);
            }
            Some(depth)
        } else {
            let mut moves = MoveList::new();
            pos.legal_moves(&mut moves);
            let (pv_head, pv_rest) = if let Some((&m, rest)) = pv_tail.split_first() {
                (Some(m), rest)
            } else {
                (None, &[][..])
            };
            for i in 0..moves.len() {
                let mv = moves[i];
                let uci = move_to_uci(mv);
                let child_path = format!("{path}.{uci}");
                let child_tail = if pv_head == Some(mv) {
                    pv_rest
                } else {
                    &[][..]
                };
                pos.do_move(mv);
                let _ = self.emit_proof_subtree(pos, &child_path, mv, Outcome::Win, child_tail);
                pos.undo_move(mv);
            }
            Some(depth)
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

    #[test]
    fn extract_pv_follows_tt_entries() {
        use crate::zobrist::INF;

        let mut search = Search::new(1);
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        let mv = Move::make_move(Square::E1, Square::E8);

        // Store a win at the root with the mating move as the best move.
        search.tt.store(
            pos.hash(),
            mv,
            0,
            0,
            Some(Outcome::Win),
            0,
            INF,
            1,
            u32::MAX,
        );

        // Store the terminal child.
        let mut child = pos.clone();
        child.do_move(mv);
        search.tt.store(
            child.hash(),
            Move::NONE,
            u8::MAX,
            0,
            Some(Outcome::Loss),
            INF,
            0,
            0,
            u32::MAX,
        );

        let pv = search.extract_pv(&pos);
        assert_eq!(
            pv,
            vec![mv],
            "extract_pv should follow the TT to the terminal"
        );
    }

    #[test]
    fn extract_pv_checked_rejects_wrong_depth() {
        use crate::zobrist::INF;

        let mut search = Search::new(1);
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        let mv = Move::make_move(Square::E1, Square::E8);

        search.tt.store(
            pos.hash(),
            mv,
            0,
            0,
            Some(Outcome::Win),
            0,
            INF,
            1,
            u32::MAX,
        );

        let mut child = pos.clone();
        child.do_move(mv);
        search.tt.store(
            child.hash(),
            Move::NONE,
            u8::MAX,
            0,
            Some(Outcome::Loss),
            INF,
            0,
            0,
            u32::MAX,
        );

        assert!(
            search
                .extract_pv_checked(&pos, Outcome::Win, Some(1))
                .is_some(),
            "depth 1 should match the stored PV"
        );
        assert!(
            search
                .extract_pv_checked(&pos, Outcome::Win, Some(2))
                .is_none(),
            "depth 2 should not match the stored PV"
        );
    }

    #[test]
    fn emit_proof_tree_populates_validate_ppv() {
        use crate::proof_tree::{ProofMessage, ProofResponse, ProofTreeWorker};
        use crate::zobrist::INF;
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let mut search = Search::new(1);
        let (tx, handle) = ProofTreeWorker::spawn(
            "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1".to_string(),
            64,
            Arc::new(AtomicBool::new(false)),
        );
        search.set_proof_tree_sender(Some(tx.clone()));

        let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        let pv = vec![Move::make_move(Square::E1, Square::E8)];
        let mv = pv[0];

        // Seed the TT so emit_proof_tree can walk to the terminal child.
        search.tt.store(
            pos.hash(),
            mv,
            0,
            0,
            Some(Outcome::Win),
            0,
            INF,
            1,
            u32::MAX,
        );
        pos.do_move(mv);
        search.tt.store(
            pos.hash(),
            Move::NONE,
            u8::MAX,
            0,
            Some(Outcome::Loss),
            INF,
            0,
            0,
            u32::MAX,
        );
        pos.undo_move(mv);

        search.emit_proof_tree(&mut pos, Outcome::Win, &pv);

        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        tx.send(ProofMessage::GetTree(reply_tx)).unwrap();
        let ProofResponse::Tree(tree) = reply_rx.recv().unwrap() else {
            panic!("expected Tree response");
        };

        assert!(
            tree.validate_ppv(&pv),
            "emitted proof tree should validate the supplied PV"
        );

        drop(search);
        drop(tx);
        handle.join().unwrap();
    }
}
