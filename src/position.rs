//! Position wrapper around `atomic_movegen::board::Board`.

use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::movegen::{generate_legal, generate_legal_with_state};
use atomic_movegen::types::{Bitboard, Color, Move, MoveList};

use crate::zobrist;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    Loss,
    Draw,
    Win,
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Outcome::Win => "win",
            Outcome::Loss => "loss",
            Outcome::Draw => "draw",
        };
        write!(f, "{s}")
    }
}

impl Outcome {
    #[must_use]
    pub fn to_pn_dn(self) -> (u64, u64) {
        match self {
            Outcome::Win => (0, zobrist::INF),
            Outcome::Loss | Outcome::Draw => (zobrist::INF, 0),
        }
    }

    #[must_use]
    pub fn pn_dn_for(self, is_or_node: bool) -> (u64, u64) {
        if is_or_node {
            self.to_pn_dn()
        } else {
            self.flip().to_pn_dn()
        }
    }

    #[must_use]
    pub fn flip(self) -> Self {
        match self {
            Outcome::Win => Outcome::Loss,
            Outcome::Loss => Outcome::Win,
            Outcome::Draw => Outcome::Draw,
        }
    }
}

pub struct Position {
    board: Board,
    zobrist: u64,
    undo_stack: Vec<StateInfo>,
}

impl Position {
    pub const STARTPOS_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    pub fn new() -> Self {
        Self::from_fen(Self::STARTPOS_FEN).unwrap()
    }

    pub fn from_fen(fen: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let board = Board::from_fen(fen)?;
        let zobrist = zobrist::hash(&board, board.rule50());
        Ok(Self {
            board,
            zobrist,
            undo_stack: Vec::new(),
        })
    }

    /// Read-only access to the underlying `Board`.
    ///
    /// Callers should prefer `Position` methods for state changes; this
    /// accessor is provided for `MoveScorer` implementations and diagnostics.
    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn do_move(&mut self, m: Move) {
        let mut state = StateInfo::new();
        self.board.do_move(m, &mut state);

        // `Board::hash()` is maintained incrementally; only the rule50 key changes.
        self.zobrist = self.board.hash() ^ zobrist::rule50_key(self.board.rule50());
        self.undo_stack.push(state);
    }

    pub fn undo_move(&mut self, m: Move) {
        let state = self.undo_stack.pop().expect("undo without move");
        self.board.undo_move(m, &state);

        // `Board::hash()` is maintained incrementally; only the rule50 key changes.
        self.zobrist = self.board.hash() ^ zobrist::rule50_key(self.board.rule50());
    }

    /// Play a move only if it is legal, returning `true` on success.
    ///
    /// This is intended for property-based and fuzz tests where blindly
    /// calling `do_move` could panic.
    #[allow(dead_code)]
    pub(crate) fn try_do_move(&mut self, m: Move) -> bool {
        let mut moves = MoveList::new();
        self.legal_moves(&mut moves);
        for i in 0..moves.len() {
            if moves[i] == m {
                self.do_move(m);
                return true;
            }
        }
        false
    }

    pub fn legal_moves(&self, moves: &mut MoveList) {
        generate_legal(&self.board, moves);
    }

    /// Populate `state` with the board state needed by `generate_legal_with_state`.
    pub fn populate_state(&self, state: &mut StateInfo) {
        self.board.populate_state(state);
    }

    pub fn legal_moves_with_state(&self, moves: &mut MoveList, state: &mut StateInfo) {
        self.board.populate_state(state);
        generate_legal_with_state(&self.board, state, moves);
    }

    /// Convenience helper that returns all legal moves in a `Vec`.
    pub fn legal_moves_vec(&self) -> Vec<Move> {
        let mut moves = MoveList::new();
        self.legal_moves(&mut moves);
        moves.as_slice().to_vec()
    }

    pub fn side_to_move(&self) -> Color {
        self.board.side_to_move()
    }

    pub fn commoners(&self, c: Color) -> Bitboard {
        self.board.commoners(c)
    }

    pub fn outcome(&self) -> Option<Outcome> {
        let mut moves = MoveList::new();
        let mut state = StateInfo::new();
        self.legal_moves_with_state(&mut moves, &mut state);
        self.outcome_from_state(&state, &moves)
    }

    /// Terminal detector for callers that already have a move list and the
    /// corresponding `StateInfo`. Returns `None` if the position is not
    /// terminal, otherwise the `Outcome` from the side-to-move perspective.
    pub fn outcome_from_state(&self, state: &StateInfo, moves: &MoveList) -> Option<Outcome> {
        let us = self.side_to_move();
        let them = us.flip();
        if self.commoners(us).is_empty() {
            return Some(Outcome::Loss);
        }
        if self.commoners(them).is_empty() {
            return Some(Outcome::Win);
        }
        if moves.is_empty() {
            if state.checkers.is_empty() {
                return Some(Outcome::Draw);
            }
            return Some(Outcome::Loss);
        }
        if self.board.rule50() >= 100 {
            return Some(Outcome::Draw);
        }
        if self.board.occupied().count() == 2 {
            return Some(Outcome::Draw);
        }
        None
    }

    #[must_use]
    pub fn hash(&self) -> u64 {
        self.zobrist
    }

    /// Board-only key for repetition detection, ignoring the halfmove clock.
    #[must_use]
    pub fn repetition_key(&self) -> u64 {
        zobrist::board_hash(&self.board)
    }

    pub fn fen(&self) -> String {
        self.board.fen()
    }
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Position {
    /// A clone is a snapshot of the current board and hash. The undo stack
    /// starts empty because a clone is not a replayable history.
    fn clone(&self) -> Self {
        Self {
            board: self.board.clone(),
            zobrist: self.zobrist,
            undo_stack: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_prefers_own_commoner_extinction_over_rule50() {
        // White has no commoners (only a pawn) and rule50 >= 100; should still be Loss.
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4P3 w - - 100 1").unwrap();
        assert_eq!(pos.outcome(), Some(Outcome::Loss));
    }

    #[test]
    fn outcome_prefers_opponent_extinction_over_rule50() {
        // Black to move, white has no commoners, rule50 >= 100; should be Win for Black.
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/8 b - - 100 1").unwrap();
        assert_eq!(pos.outcome(), Some(Outcome::Win));
    }

    #[test]
    fn no_legal_moves_is_stalemate_draw() {
        // White commoner on a1 is stalemated: no legal moves and not in check.
        let pos = Position::from_fen("7k/8/8/8/8/8/2q5/K7 w - - 0 1").unwrap();
        assert_eq!(pos.outcome(), Some(Outcome::Draw));

        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);
        assert_eq!(moves.len(), 0);
    }

    #[test]
    fn no_legal_moves_in_check_is_checkmate_loss() {
        let pos = Position::from_fen("7K/8/8/8/8/8/1Q6/k7 b - - 0 1").unwrap();
        assert_eq!(pos.outcome(), Some(Outcome::Loss));
    }

    #[test]
    fn fifty_move_checkmate_is_loss_not_draw() {
        // Black has no legal moves and is in check, but rule50 is 100.
        // Checkmate ends the game before the 50-move draw can be claimed.
        let pos = Position::from_fen("7K/8/8/8/8/8/1Q6/k7 b - - 100 1").unwrap();
        assert_eq!(pos.outcome(), Some(Outcome::Loss));
    }

    #[test]
    fn two_piece_touching_commoners_is_draw() {
        // In standard atomic chess touching commoners (kings) are allowed and
        // do not count as an attack, so the side to move has legal moves and
        // the two-piece material heuristic is a draw.
        let pos = Position::from_fen("8/8/8/8/8/8/1K6/k7 b - - 0 1").unwrap();
        assert_eq!(pos.outcome(), Some(Outcome::Draw));
    }

    #[test]
    fn fifty_move_stalemate_is_draw() {
        // White has no legal moves and is not in check; with rule50 >= 100
        // the result is still a draw because stalemate is terminal.
        let pos = Position::from_fen("7k/8/8/8/8/8/2q5/K7 w - - 100 1").unwrap();
        assert_eq!(pos.outcome(), Some(Outcome::Draw));
    }

    #[test]
    fn position_with_legal_moves_is_not_terminal() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        assert_eq!(pos.outcome(), None);
    }

    #[test]
    fn repetition_key_ignores_rule50() {
        let pos0 = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let pos1 = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 25 1").unwrap();
        assert_eq!(pos0.repetition_key(), pos1.repetition_key());
        assert_ne!(pos0.hash(), pos1.hash());
    }

    #[test]
    fn try_do_move_accepts_legal_and_rejects_illegal() {
        let mut pos =
            Position::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let legal = atomic_movegen::types::Move::make_move(
            atomic_movegen::types::Square::E2,
            atomic_movegen::types::Square::E4,
        );
        let illegal = atomic_movegen::types::Move::make_move(
            atomic_movegen::types::Square::E2,
            atomic_movegen::types::Square::E5,
        );

        assert!(
            pos.try_do_move(legal),
            "e2e4 should be legal from the start position"
        );
        assert_eq!(pos.undo_stack.len(), 1);

        assert!(!pos.try_do_move(illegal), "e2e5 should be illegal");
        assert_eq!(
            pos.undo_stack.len(),
            1,
            "illegal move must not change state"
        );
    }
}
