//! PV extraction and validation.

use std::collections::HashSet;

use atomic_movegen::types::{Move, MoveList};

use crate::position::{Outcome, Position};
use crate::zobrist;

use super::Search;

impl Search {
    pub(super) fn extract_pv(&self, pos: &Position) -> Vec<Move> {
        self.extract_pv_internal(pos).0
    }

    pub(super) fn extract_pv_checked(
        &self,
        pos: &Position,
        expected: Outcome,
        expected_depth: Option<u32>,
    ) -> Option<Vec<Move>> {
        let (pv, truncated) = self.extract_pv_internal(pos);
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

    pub(super) fn validate_pv(
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

    fn extract_pv_internal(&self, pos: &Position) -> (Vec<Move>, bool) {
        let mut pv = Vec::new();
        let mut seen = HashSet::new();
        let mut current = pos.clone();
        let mut path_code = 0u64;
        for _ in 0..self.max_ply {
            let tt_key = current.hash();
            let rep_key = current.repetition_key();
            if seen.contains(&rep_key) {
                break;
            }
            if current.outcome().is_some() {
                break;
            }
            if let Some(entry) = self.tt.probe(tt_key) {
                let resolved = entry.best_result_for_path(path_code);
                if let Some((mv, Some(_), _)) = resolved {
                    if mv == Move::NONE {
                        break;
                    }
                    seen.insert(rep_key);
                    pv.push(mv);
                    current.do_move(mv);
                    path_code ^= zobrist::path_random(mv, pv.len());
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        let truncated = pv.len() == self.max_ply && current.outcome().is_none();
        (pv, truncated)
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
}
