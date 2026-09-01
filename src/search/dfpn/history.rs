//! History and killer move ordering helpers.
//!
//! This file is larger than 10 KiB because the history table, killer slots,
//! move sorting, and diagnostic breakdown all share the same scoring constants
//! and mutable search state.

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Color, Move, MoveList, Square};

use crate::position::Position;

use super::Search;
use crate::search::ordering::nearest_commoner_map;

pub(crate) const HISTORY_MAX: i32 = 10_000;
pub(crate) const HISTORY_BONUS: i32 = 100;
pub(crate) const HISTORY_AGE_INTERVAL: u64 = 10_000;
pub(crate) const SCORE_KILLER: i32 = 50_000;
pub(crate) const KILLER_SLOTS: usize = 2;
pub(crate) const MAX_KILLER_DEPTH: usize = 256;

/// Add a history bonus for `side`'s move from `from` to `to`, capping at
/// `HISTORY_MAX`.
fn update_history_entry(history: &mut [[[i32; 64]; 64]; 2], side: Color, from: Square, to: Square) {
    let entry = &mut history[side as usize][from as usize][to as usize];
    *entry = (*entry + HISTORY_BONUS).min(HISTORY_MAX);
}

/// Age every history value by halving it (and re-capping at `HISTORY_MAX`).
fn age_history(history: &mut [[[i32; 64]; 64]; 2]) {
    for side in history {
        for from in side {
            for entry in from {
                *entry = (*entry / 2).min(HISTORY_MAX);
            }
        }
    }
}

/// Insert `m` into the killer slots for `depth`, shifting the previous primary
/// killer to the secondary slot.
fn update_killer_slots(
    killers: &mut [[Move; KILLER_SLOTS]; MAX_KILLER_DEPTH],
    depth: usize,
    m: Move,
) {
    if m == Move::NONE {
        return;
    }
    if depth >= MAX_KILLER_DEPTH {
        return;
    }
    let slot = &mut killers[depth];
    if m != slot[0] {
        slot[1] = slot[0];
        slot[0] = m;
    }
}

/// Return the killer bonus for `m` if it is stored at `depth`.
fn killer_bonus(killers: &[[Move; KILLER_SLOTS]; MAX_KILLER_DEPTH], m: Move, depth: usize) -> i32 {
    if depth >= MAX_KILLER_DEPTH {
        return 0;
    }
    if killers[depth].contains(&m) {
        SCORE_KILLER
    } else {
        0
    }
}

impl Search {
    pub(super) fn sort_moves(
        &self,
        pos: &Position,
        moves: &mut MoveList,
        best_from_tt: Move,
        is_or_node: bool,
    ) {
        let mut state = StateInfo::new();
        pos.populate_state(&mut state);

        let us = pos.side_to_move() as usize;
        let depth = self.path_stack.len();
        let slice = moves.as_mut_slice();

        // The neural scorer is a residual on top of the static term
        // (`nn.md` §6 v2 recipe): the final score is static + nn + history
        // + killer, so the nearest-commoner map is always needed.
        let nn_scores = self
            .nn_scorer
            .as_ref()
            .map(|nn| nn.move_scores(pos.board(), slice));
        let nearest = nearest_commoner_map(pos.board(), pos.side_to_move().flip());

        let board = pos.board();
        let mut scored: Vec<(Move, i32)> = slice
            .iter()
            .copied()
            .enumerate()
            .map(|(i, m)| {
                let static_score = self
                    .scorer
                    .score_with_map(board, m, &state, &nearest, is_or_node);
                let nn_score = nn_scores.as_ref().map_or(0, |scores| scores[i]);
                let score = static_score
                    + nn_score
                    + self.history[us][m.from_sq() as usize][m.to_sq() as usize]
                    + self.killer_bonus(m, depth);
                (m, score)
            })
            .collect();

        scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));

        if best_from_tt != Move::NONE
            && let Some(idx) = scored.iter().position(|&(m, _)| m == best_from_tt)
        {
            let entry = scored[idx];
            for i in (0..idx).rev() {
                scored[i + 1] = scored[i];
            }
            scored[0] = entry;
        }

        for (i, (m, _)) in scored.into_iter().enumerate() {
            slice[i] = m;
        }
    }

    pub(super) fn update_history(&mut self, m: Move, side: Color) {
        update_history_entry(&mut self.history, side, m.from_sq(), m.to_sq());
    }

    pub(super) fn update_killers(&mut self, best_move: Move) {
        let depth = self.path_stack.len();
        update_killer_slots(&mut self.killers, depth, best_move);
    }

    pub(super) fn maybe_age_history(&mut self) {
        self.history_age_counter += 1;
        if self.history_age_counter < HISTORY_AGE_INTERVAL {
            return;
        }
        self.history_age_counter = 0;
        age_history(&mut self.history);
    }

    fn killer_bonus(&self, m: Move, depth: usize) -> i32 {
        killer_bonus(&self.killers, m, depth)
    }

    /// Return a diagnostic breakdown of the move-ordering scores for all legal
    /// moves in `pos` at the current search depth.
    ///
    /// The returned vector contains `(move, static_score, history_bonus,
    /// killer_bonus, total_score)` tuples, sorted from highest total score to
    /// lowest. This is intended for the `move_order_debug` example. The
    /// breakdown always uses the hand-crafted static scorer; it does not
    /// reflect a configured `NnMoveScorer` (see `Search::set_nn_scorer`).
    #[must_use]
    pub fn move_order_breakdown(
        &self,
        pos: &Position,
        is_or_node: bool,
    ) -> Vec<(Move, i32, i32, i32, i32)> {
        use crate::search::ordering::nearest_commoner_map;

        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);

        let mut state = StateInfo::new();
        pos.populate_state(&mut state);

        let us = pos.side_to_move();
        let them = us.flip();
        let nearest = nearest_commoner_map(pos.board(), them);
        let depth = self.path_stack.len();
        let mut result = Vec::with_capacity(moves.len());

        for i in 0..moves.len() {
            let m = moves[i];
            let static_score =
                self.scorer
                    .score_with_map(pos.board(), m, &state, &nearest, is_or_node);
            let history = self.history[us as usize][m.from_sq() as usize][m.to_sq() as usize];
            let killer = self.killer_bonus(m, depth);
            let total = static_score + history + killer;
            result.push((m, static_score, history, killer, total));
        }

        result.sort_by_key(|b| std::cmp::Reverse(b.4));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;
    use atomic_movegen::types::{Color, Square};

    fn start_position() -> Position {
        Position::from_fen(Position::STARTPOS_FEN).unwrap()
    }

    #[test]
    fn sort_orders_empty_list_without_panic() {
        let search = Search::new(1);
        let pos = start_position();
        let mut moves = MoveList::new();
        search.sort_moves(&pos, &mut moves, Move::NONE, true);
        assert!(moves.is_empty());
    }

    #[test]
    fn sort_is_deterministic() {
        let search = Search::new(1);
        let pos = start_position();
        let mut moves1 = MoveList::new();
        let mut moves2 = MoveList::new();
        pos.legal_moves(&mut moves1);
        pos.legal_moves(&mut moves2);
        search.sort_moves(&pos, &mut moves1, Move::NONE, true);
        search.sort_moves(&pos, &mut moves2, Move::NONE, true);
        assert_eq!(moves1.as_slice(), moves2.as_slice());
    }

    /// Residual composition (`nn.md` §6a v2 recipe): with the NN scorer
    /// enabled the ordering must be `static + nn` (fresh search: history and
    /// killer are zero), not the v1 replacement of the static term.
    #[test]
    fn nn_scorer_is_residual_on_the_static_term() {
        use crate::nn::{NnMoveScorer, NnWeights};
        use crate::search::ordering::{StaticAtomicScorer, nearest_commoner_map};
        use atomic_movegen::board::StateInfo;
        use std::sync::Arc;

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/nn_trainer_ref/fixtures/weights.v1.bin");
        let weights = Arc::new(NnWeights::from_path(&path).expect("fixture weight file must load"));
        let nn = NnMoveScorer::new(Arc::clone(&weights));

        let mut search = Search::new(1);
        search.set_scorer(StaticAtomicScorer::default());
        search.set_nn_scorer(Some(nn));

        let pos = start_position();
        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);

        let slice: Vec<_> = moves.as_slice().to_vec();
        let nn_scores = search.nn_scorer().unwrap().move_scores(pos.board(), &slice);
        let mut state = StateInfo::new();
        pos.populate_state(&mut state);
        let nearest = nearest_commoner_map(pos.board(), pos.side_to_move().flip());
        let mut expected: Vec<(Move, i32)> = slice
            .iter()
            .copied()
            .enumerate()
            .map(|(i, m)| {
                (
                    m,
                    search
                        .scorer()
                        .score_with_map(pos.board(), m, &state, &nearest, true)
                        + nn_scores[i],
                )
            })
            .collect();
        expected.sort_by_key(|&(_, score)| std::cmp::Reverse(score));

        let mut sorted = MoveList::new();
        pos.legal_moves(&mut sorted);
        search.sort_moves(&pos, &mut sorted, Move::NONE, true);
        let sorted: Vec<Move> = sorted.as_slice().to_vec();
        let expected: Vec<Move> = expected.into_iter().map(|(m, _)| m).collect();
        assert_eq!(sorted, expected, "ordering must be static + nn");
    }

    #[test]
    fn history_bonus_raises_move_score() {
        let mut search = Search::new(1);
        let pos = start_position();
        let e2e4 = Move::make_move(Square::E2, Square::E4);

        let mut before = MoveList::new();
        pos.legal_moves(&mut before);
        search.sort_moves(&pos, &mut before, Move::NONE, true);
        let rank_before = before.as_slice().iter().position(|&m| m == e2e4).unwrap();

        search.update_history(e2e4, Color::White);

        let mut after = MoveList::new();
        pos.legal_moves(&mut after);
        search.sort_moves(&pos, &mut after, Move::NONE, true);
        let rank_after = after.as_slice().iter().position(|&m| m == e2e4).unwrap();

        assert!(
            rank_after <= rank_before,
            "history bonus should move e2e4 up the list (was {rank_before}, now {rank_after})"
        );
    }

    #[test]
    fn history_caps_at_maximum() {
        let mut search = Search::new(1);
        let e2e4 = Move::make_move(Square::E2, Square::E4);
        let from = e2e4.from_sq();
        let to = e2e4.to_sq();

        for _ in 0..(HISTORY_MAX / HISTORY_BONUS + 10) {
            search.update_history(e2e4, Color::White);
        }
        assert_eq!(
            search.history[Color::White as usize][from as usize][to as usize],
            HISTORY_MAX
        );
    }

    #[test]
    fn update_history_entry_helper_caps_at_max() {
        let mut history = [[[0i32; 64]; 64]; 2];
        let from = Square::A1;
        let to = Square::A2;
        for _ in 0..(HISTORY_MAX / HISTORY_BONUS + 10) {
            update_history_entry(&mut history, Color::White, from, to);
        }
        assert_eq!(
            history[Color::White as usize][from as usize][to as usize],
            HISTORY_MAX
        );
    }

    #[test]
    fn age_history_halves_scores() {
        let mut search = Search::new(1);
        let e2e4 = Move::make_move(Square::E2, Square::E4);
        let d2d4 = Move::make_move(Square::D2, Square::D4);

        search.update_history(e2e4, Color::White);
        search.update_history(d2d4, Color::White);
        let e2 =
            search.history[Color::White as usize][e2e4.from_sq() as usize][e2e4.to_sq() as usize];
        let d2 =
            search.history[Color::White as usize][d2d4.from_sq() as usize][d2d4.to_sq() as usize];
        assert_eq!(e2, HISTORY_BONUS);
        assert_eq!(d2, HISTORY_BONUS);

        search.history_age_counter = HISTORY_AGE_INTERVAL - 1;
        search.maybe_age_history();
        assert_eq!(
            search.history[Color::White as usize][e2e4.from_sq() as usize][e2e4.to_sq() as usize],
            HISTORY_BONUS / 2
        );
    }

    #[test]
    fn age_history_helper_halves_all_entries() {
        let mut history = [[[100i32; 64]; 64]; 2];
        age_history(&mut history);
        assert!(history.iter().all(|side| {
            side.iter()
                .all(|from| from.iter().all(|&entry| entry == 50.min(HISTORY_MAX)))
        }));
    }

    #[test]
    fn update_killers_shifts_previous_killer_to_second_slot() {
        let mut search = Search::new(1);
        search.path_stack.push(0); // depth 1

        let a = Move::make_move(Square::E2, Square::E4);
        let b = Move::make_move(Square::D2, Square::D4);

        search.update_killers(a);
        assert_eq!(search.killers[1][0], a);
        assert_eq!(search.killers[1][1], Move::NONE);

        search.update_killers(b);
        assert_eq!(search.killers[1][0], b);
        assert_eq!(search.killers[1][1], a);

        // Re-adding the current primary killer should not duplicate.
        search.update_killers(b);
        assert_eq!(search.killers[1][0], b);
        assert_eq!(search.killers[1][1], a);
    }

    #[test]
    fn update_killer_slots_helper_shifts_and_deduplicates() {
        let mut killers = [[Move::NONE; KILLER_SLOTS]; MAX_KILLER_DEPTH];
        let a = Move::make_move(Square::E2, Square::E4);
        let b = Move::make_move(Square::D2, Square::D4);

        update_killer_slots(&mut killers, 1, a);
        assert_eq!(killers[1][0], a);

        update_killer_slots(&mut killers, 1, b);
        assert_eq!(killers[1][0], b);
        assert_eq!(killers[1][1], a);

        update_killer_slots(&mut killers, 1, b);
        assert_eq!(killers[1][0], b);
        assert_eq!(killers[1][1], a);
    }

    #[test]
    fn killer_moves_get_sort_bonus() {
        let mut search = Search::new(1);
        search.path_stack.push(0); // depth 1

        let e2e4 = Move::make_move(Square::E2, Square::E4);
        search.update_killers(e2e4);

        let pos = start_position();
        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);
        search.sort_moves(&pos, &mut moves, Move::NONE, true);

        assert_eq!(moves[0], e2e4, "killer move should be sorted first");
    }

    #[test]
    fn killer_bonus_helper_matches_method() {
        let mut search = Search::new(1);
        search.path_stack.push(0);
        let e2e4 = Move::make_move(Square::E2, Square::E4);
        search.update_killers(e2e4);

        assert_eq!(
            search.killer_bonus(e2e4, 1),
            killer_bonus(&search.killers, e2e4, 1)
        );
        assert_eq!(killer_bonus(&search.killers, e2e4, 1), SCORE_KILLER);
        assert_eq!(
            killer_bonus(&search.killers, Move::make_move(Square::A1, Square::A2), 1),
            0
        );
    }
}
