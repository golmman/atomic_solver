//! Position wrapper around `atomic_movegen::board::Board`.

use atomic_movegen::board::{Board, StateInfo};
use atomic_movegen::movegen::{generate_legal, generate_legal_with_state};
use atomic_movegen::types::{Bitboard, Color, Move, MoveList};

use crate::zobrist;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome {
    Loss,
    Draw,
    Win,
}

impl Outcome {
    pub fn to_pn_dn(self) -> (u64, u64) {
        match self {
            Outcome::Win => (0, zobrist::INF),
            Outcome::Loss => (zobrist::INF, 0),
            Outcome::Draw => (zobrist::INF, 0),
        }
    }

    pub fn pn_dn_for(self, is_or_node: bool) -> (u64, u64) {
        if is_or_node {
            self.to_pn_dn()
        } else {
            self.flip().to_pn_dn()
        }
    }

    pub fn flip(self) -> Self {
        match self {
            Outcome::Win => Outcome::Loss,
            Outcome::Loss => Outcome::Win,
            Outcome::Draw => Outcome::Draw,
        }
    }
}

pub struct Position {
    pub board: Board,
    pub zobrist: u64,
    undo_stack: Vec<StateInfo>,
}

impl Position {
    pub fn new() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap()
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

    pub fn do_move(&mut self, m: Move) {
        let mut state = StateInfo::new();
        self.board.do_move(m, &mut state);

        self.zobrist = zobrist::hash(&self.board, self.board.rule50());
        self.undo_stack.push(state);
    }

    pub fn undo_move(&mut self, m: Move) {
        let state = self.undo_stack.pop().expect("undo without move");
        self.board.undo_move(m, &state);
        self.zobrist = zobrist::hash(&self.board, self.board.rule50());
    }

    pub fn legal_moves(&self, moves: &mut MoveList) {
        generate_legal(&self.board, moves);
    }

    pub fn legal_moves_with_state(&self, moves: &mut MoveList, state: &mut StateInfo) {
        self.board.populate_state(state);
        generate_legal_with_state(&self.board, state, moves);
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
        if self.board.rule50() >= 100 {
            return Some(Outcome::Draw);
        }
        if self.board.occupied().count() == 2 {
            return Some(Outcome::Draw);
        }
        if moves.is_empty() {
            if state.checkers.is_empty() {
                return Some(Outcome::Draw);
            } else {
                return Some(Outcome::Loss);
            }
        }
        None
    }

    pub fn hash(&self) -> u64 {
        self.zobrist
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
    fn position_with_legal_moves_is_not_terminal() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
        assert_eq!(pos.outcome(), None);
    }
}
