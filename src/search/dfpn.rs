//! Sequential DF-PN+ solver for atomic chess.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use atomic_movegen::types::{Move, MoveList};

use crate::notation::move_to_uci;
use crate::position::{Outcome, Position};
use crate::zobrist;

use super::ordering::{MoveScorer, StaticAtomicScorer};
use super::tt::TranspositionTable;

pub const INF: u64 = zobrist::INF;

const EPSILON: f64 = 0.25;
const TIMEOUT_SECS: u64 = 5;
const SIM_MAX_DEPTH: usize = 1000;
const SIM_MAX_NODES: u64 = 1000;

pub struct Search {
    tt: TranspositionTable,
    path: HashSet<u64>,
    path_stack: Vec<u64>,
    path_code: u64,
    nodes: u64,
    start: Instant,
    deadline: Instant,
    epsilon: f64,
    scorer: Box<dyn MoveScorer>,
    refine_shortest: bool,
    timeout: Duration,
    last_pv: Vec<Move>,
}

impl Search {
    pub fn new(tt_mb: usize) -> Self {
        Self {
            tt: TranspositionTable::with_mb(tt_mb),
            path: HashSet::new(),
            path_stack: Vec::new(),
            path_code: 0,
            nodes: 0,
            start: Instant::now(),
            deadline: Instant::now(),
            epsilon: EPSILON,
            scorer: Box::new(StaticAtomicScorer),
            refine_shortest: false,
            timeout: Duration::from_secs(TIMEOUT_SECS),
            last_pv: Vec::new(),
        }
    }

    pub fn refine_shortest(&mut self, value: bool) {
        self.refine_shortest = value;
    }

    pub fn set_timeout(&mut self, seconds: u64) {
        self.timeout = Duration::from_secs(seconds);
    }

    pub fn solve(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        self.nodes = 0;
        self.start = Instant::now();
        self.deadline = self.start + self.timeout;
        self.path.clear();
        self.path_stack.clear();
        self.path_code = 0;
        self.last_pv.clear();

        if self.refine_shortest {
            self.solve_refined(pos)
        } else {
            let outcome = self.dfpn(pos, INF, INF, u32::MAX, true);
            let pv = self.extract_pv(pos);
            (outcome, pv, self.nodes)
        }
    }

    fn solve_refined(&mut self, pos: &mut Position) -> (Outcome, Vec<Move>, u64) {
        // Depth-bounded refinement: first find any win/loss without a depth bound
        // to get an initial PV, then binary search the smallest depth bound that
        // still yields the same outcome.
        let saved_refine = self.refine_shortest;
        self.refine_shortest = false;

        let outcome = self.dfpn(pos, INF, INF, u32::MAX, true);
        let best_outcome = outcome;
        let best_depth = self
            .tt
            .probe(pos.hash())
            .and_then(|e| e.best_result_for_path(0).map(|(.., depth)| depth))
            .unwrap_or(u32::MAX);

        if let Some(first_pv) = self.extract_pv_checked(pos) {
            self.print_pv_update(outcome, &first_pv);
        }

        if outcome != Outcome::Draw && best_depth > 1 {
            for mid in 1..best_depth {
                self.reset_search_state();
                self.tt.clear();
                let o = self.dfpn(pos, INF, INF, mid, true);
                if o == outcome {
                    if let Some(pv) = self.extract_pv_checked(pos) {
                        self.print_pv_update(outcome, &pv);
                    }
                    break;
                }
            }
        }

        self.refine_shortest = saved_refine;

        let pv = if !self.last_pv.is_empty() {
            self.last_pv.clone()
        } else {
            self.extract_pv(pos)
        };
        (best_outcome, pv, self.nodes)
    }

    fn reset_search_state(&mut self) {
        self.path.clear();
        self.path_stack.clear();
        self.path_code = 0;
    }

    fn extract_pv_checked(&self, pos: &Position) -> Option<Vec<Move>> {
        let pv = self.extract_pv(pos);
        if Self::validate_pv(&pv, pos) {
            Some(pv)
        } else {
            None
        }
    }

    fn validate_pv(pv: &[Move], pos: &Position) -> bool {
        let mut current = pos.clone();
        for &m in pv {
            current.do_move(m);
        }
        current.outcome().is_some()
    }

    fn dfpn(
        &mut self,
        pos: &mut Position,
        th_pn: u64,
        th_dn: u64,
        max_depth: u32,
        is_or_node: bool,
    ) -> Outcome {
        if Instant::now() >= self.deadline {
            return Outcome::Draw;
        }

        self.nodes += 1;

        let key = pos.hash();

        if let Some(outcome) = pos.outcome() {
            let (pn, dn) = outcome.pn_dn_for(is_or_node);
            self.tt.store(
                key,
                Move::NONE,
                Some(outcome),
                pn,
                dn,
                0,
                self.path_code,
                false,
            );
            return outcome;
        }

        if max_depth == 0 {
            let (pn, dn) = Outcome::Draw.pn_dn_for(is_or_node);
            self.tt.store(
                key,
                Move::NONE,
                Some(Outcome::Draw),
                pn,
                dn,
                0,
                self.path_code,
                false,
            );
            return Outcome::Draw;
        }

        if let Some(entry) = self.tt.probe(key).copied()
            && let Some(resolved) = self.try_use_tt(pos, &entry, max_depth, self.path_code)
        {
            return resolved.outcome;
        }

        if !self.path.insert(key) {
            return Outcome::Draw;
        }

        let mut moves = MoveList::new();
        pos.legal_moves(&mut moves);

        if moves.is_empty() {
            self.path.remove(&key);
            let (pn, dn) = Outcome::Draw.pn_dn_for(is_or_node);
            self.tt.store(
                key,
                Move::NONE,
                Some(Outcome::Draw),
                pn,
                dn,
                0,
                self.path_code,
                false,
            );
            return Outcome::Draw;
        }

        let best_from_tt = self
            .tt
            .probe(key)
            .and_then(|e| e.best_result_for_path(self.path_code).map(|(mv, ..)| mv))
            .unwrap_or(Move::NONE);
        self.sort_moves(pos, &mut moves, best_from_tt);

        self.path_stack.push(key);
        let old_path_code = self.path_code;

        let mut outcome_to_store: Option<Outcome> = None;
        let mut outcome_to_store_best_move = Move::NONE;
        let mut outcome_to_store_pn = INF;
        let mut outcome_to_store_dn = INF;
        let mut outcome_to_store_depth = 0;
        let mut outcome_to_store_repetition_seen = false;
        let mut best_move = Move::NONE;
        let mut pn = INF;
        let mut dn = INF;
        let mut depth = 0;
        let mut repetition_seen = false;
        let mut last_printed: Option<(Outcome, u32)> = None;

        loop {
            if Instant::now() >= self.deadline {
                break;
            }

            let selection = self.select_children(pos, &moves, max_depth, is_or_node);
            best_move = selection.best_move;
            pn = selection.pn;
            dn = selection.dn;
            depth = selection.depth;
            repetition_seen = selection.repetition_seen;

            if let Some(solved) = selection.solved_outcome {
                outcome_to_store = Some(solved);
                outcome_to_store_best_move = best_move;
                outcome_to_store_pn = pn;
                outcome_to_store_dn = dn;
                outcome_to_store_depth = depth;
                outcome_to_store_repetition_seen = repetition_seen;

                if self.refine_shortest
                    && self.path_stack.len() == 1
                    && (solved == Outcome::Win || solved == Outcome::Loss)
                    && self.should_print_update(solved, depth, last_printed)
                {
                    // Store the current best move so extract_pv can follow it.
                    self.tt.store(
                        key,
                        best_move,
                        Some(solved),
                        pn,
                        dn,
                        depth,
                        old_path_code,
                        repetition_seen,
                    );
                    let pv = self.extract_pv(pos);
                    self.print_pv_update(solved, &pv);
                    last_printed = Some((solved, depth));
                }

                if selection.all_solved {
                    break;
                }

                // When refinement is enabled at the root, keep refining own Win
                // nodes to find the shortest win. For the opponent's Win nodes,
                // non-root Win nodes, or when refinement is disabled, stop at
                // the first winning line.
                if selection.solved_outcome == Some(Outcome::Win)
                    && (!self.refine_shortest || !is_or_node || self.path_stack.len() > 1)
                {
                    break;
                }

                // A Win with unresolved siblings: keep refining.
            }

            if (th_pn != INF && pn >= th_pn) || (th_dn != INF && dn >= th_dn) {
                // A pending Win for the side to move has pn = 0 / dn = INF;
                // keep refining siblings. Otherwise, respect thresholds.
                if selection.solved_outcome != Some(Outcome::Win)
                    || !self.refine_shortest
                    || !is_or_node
                    || self.path_stack.len() > 1
                    || selection.all_solved
                {
                    break;
                }
            }

            let (mv, child_pn, child_dn, _vpn, _vdn) = selection.best_child;
            if mv == Move::NONE {
                break;
            }
            let (second_pn, second_dn) = selection.second_child;

            let (np, nd) = if is_or_node {
                let new_th_pn = std::cmp::min(th_pn, self.epsilon_ceil(second_pn));
                let new_th_dn = if th_dn == INF {
                    INF
                } else {
                    th_dn.saturating_sub(dn).saturating_add(child_dn)
                };
                (new_th_pn, new_th_dn)
            } else {
                let new_th_dn = std::cmp::min(th_dn, self.epsilon_ceil(second_dn));
                let new_th_pn = if th_pn == INF {
                    INF
                } else {
                    th_pn.saturating_sub(pn).saturating_add(child_pn)
                };
                (new_th_pn, new_th_dn)
            };

            pos.do_move(mv);
            self.path_code ^= zobrist::path_random(mv, self.path_stack.len());
            let _ = self.dfpn(pos, np, nd, max_depth.saturating_sub(1), !is_or_node);
            self.path_code ^= zobrist::path_random(mv, self.path_stack.len());
            pos.undo_move(mv);
        }

        self.path_stack.pop();
        self.path.remove(&key);
        self.path_code = old_path_code;

        if outcome_to_store.is_some() {
            self.tt.store(
                key,
                outcome_to_store_best_move,
                outcome_to_store,
                outcome_to_store_pn,
                outcome_to_store_dn,
                outcome_to_store_depth,
                old_path_code,
                outcome_to_store_repetition_seen,
            );
        } else {
            self.tt.store(
                key,
                best_move,
                outcome_to_store,
                pn,
                dn,
                depth,
                old_path_code,
                repetition_seen,
            );
        }
        outcome_to_store.unwrap_or(Outcome::Draw)
    }

    fn try_use_tt(
        &mut self,
        pos: &Position,
        entry: &super::tt::TtEntry,
        max_depth: u32,
        path_code: u64,
    ) -> Option<Resolved> {
        // 1. Path-independent base result.
        if let Some(outcome) = entry.outcome
            && !entry.repetition_seen
            && entry.depth <= max_depth
        {
            return Some(Resolved {
                outcome,
                depth: entry.depth,
                repetition_seen: false,
            });
        }

        // 2. Try existing twins for the current path.
        for twin in entry.twins.iter() {
            if let Some(outcome) = twin.outcome
                && twin.path_code == path_code
                && twin.depth <= max_depth
            {
                return Some(Resolved {
                    outcome,
                    depth: twin.depth,
                    repetition_seen: true,
                });
            }
        }

        // 3. Kawano simulation: verify a twin from another path for the current path.
        for twin in entry.twins.iter() {
            let outcome = match twin.outcome {
                Some(o) => o,
                None => continue,
            };
            if twin.depth > max_depth {
                continue;
            }
            let mut sim_pos = pos.clone();
            let mut sim_path = HashSet::new();
            let mut sim_stack = Vec::new();
            let mut sim_nodes = 0u64;
            if self.simulate(
                &mut sim_pos,
                twin.path_code,
                outcome,
                twin.best_move,
                &mut sim_path,
                &mut sim_stack,
                &mut sim_nodes,
            ) {
                self.tt
                    .store_twin(entry.key, path_code, outcome, twin.best_move, twin.depth);
                return Some(Resolved {
                    outcome,
                    depth: twin.depth,
                    repetition_seen: true,
                });
            }
        }

        None
    }

    #[allow(clippy::too_many_arguments)]
    fn simulate(
        &self,
        pos: &mut Position,
        path_code: u64,
        expected: Outcome,
        best_move: Move,
        sim_path: &mut HashSet<u64>,
        sim_stack: &mut Vec<u64>,
        sim_nodes: &mut u64,
    ) -> bool {
        if *sim_nodes >= SIM_MAX_NODES {
            return false;
        }
        *sim_nodes += 1;

        if let Some(outcome) = pos.outcome() {
            return outcome == expected;
        }

        let key = pos.hash();
        if !sim_path.insert(key) {
            return false;
        }
        sim_stack.push(key);

        if sim_stack.len() > SIM_MAX_DEPTH {
            sim_stack.pop();
            sim_path.remove(&key);
            return false;
        }

        let ok = match expected {
            Outcome::Win | Outcome::Draw => {
                if best_move == Move::NONE {
                    false
                } else {
                    pos.do_move(best_move);
                    let child_path_code =
                        path_code ^ zobrist::path_random(best_move, sim_stack.len());
                    let child_expected = if expected == Outcome::Draw {
                        Outcome::Draw
                    } else {
                        Outcome::Loss
                    };
                    let entry = self.tt.probe(pos.hash()).copied();
                    let child_best =
                        entry.and_then(|e| e.find_result_for_path(child_path_code, child_expected));
                    let ok = child_best.is_some_and(|b| {
                        self.simulate(
                            pos,
                            child_path_code,
                            child_expected,
                            b.best_move,
                            sim_path,
                            sim_stack,
                            sim_nodes,
                        )
                    });
                    pos.undo_move(best_move);
                    ok
                }
            }
            Outcome::Loss => {
                let mut moves = MoveList::new();
                pos.legal_moves(&mut moves);
                let mut ok = true;
                for i in 0..moves.len() {
                    let mv = moves[i];
                    pos.do_move(mv);
                    let child_path_code = path_code ^ zobrist::path_random(mv, sim_stack.len());
                    let entry = self.tt.probe(pos.hash()).copied();
                    let child_best =
                        entry.and_then(|e| e.find_result_for_path(child_path_code, Outcome::Win));
                    if !child_best.is_some_and(|b| {
                        self.simulate(
                            pos,
                            child_path_code,
                            Outcome::Win,
                            b.best_move,
                            sim_path,
                            sim_stack,
                            sim_nodes,
                        )
                    }) {
                        ok = false;
                    }
                    pos.undo_move(mv);
                    if !ok {
                        break;
                    }
                }
                ok
            }
        };

        sim_stack.pop();
        sim_path.remove(&key);
        ok
    }

    fn epsilon_ceil(&self, x: u64) -> u64 {
        if x >= INF {
            return INF;
        }
        let scaled = (x as f64 * (1.0 + self.epsilon)).ceil() as u64;
        scaled.min(INF)
    }

    fn select_children(
        &mut self,
        pos: &mut Position,
        moves: &MoveList,
        max_depth: u32,
        is_or_node: bool,
    ) -> ChildSelection {
        let mut children = Vec::with_capacity(moves.len());
        for i in 0..moves.len() {
            let mv = moves[i];
            let info = self.evaluate_child(pos, mv, max_depth, is_or_node);
            children.push(info);
        }

        let mut pn;
        let mut dn;
        if is_or_node {
            pn = INF;
            dn = 0;
            for c in &children {
                pn = std::cmp::min(pn, c.pn);
                dn = std::cmp::min(INF, dn.saturating_add(c.dn));
            }
        } else {
            pn = 0;
            dn = INF;
            for c in &children {
                pn = std::cmp::min(INF, pn.saturating_add(c.pn));
                dn = std::cmp::min(dn, c.dn);
            }
        }

        let solved = Self::is_solved_by_children(&children, is_or_node);

        // Choose the child to expand from the unsolved children only.
        let (best_idx, second_idx) = Self::best_and_second_unsolved(&children, is_or_node);
        let best = best_idx.map(|i| &children[i]);
        let second = second_idx.map(|i| &children[i]);

        let best_child = best.map(|b| (b.mv, b.pn, b.dn, b.vpn, b.vdn)).unwrap_or((
            Move::NONE,
            INF,
            INF,
            INF,
            INF,
        ));
        let second_child = second.map(|s| (s.pn, s.dn)).unwrap_or((INF, INF));

        let best_move = if let Some((_, _, mv, _, _)) = solved {
            mv
        } else {
            best_idx.map(|i| children[i].mv).unwrap_or(Move::NONE)
        };

        let depth = solved.map(|(_, d, _, _, _)| d).unwrap_or(0);
        let all_solved = solved.map(|(_, _, _, all, _)| all).unwrap_or(false);

        let repetition_seen = if let Some((outcome, _, _, _, idx)) = solved {
            match outcome {
                Outcome::Win | Outcome::Draw => children[idx].repetition_seen,
                Outcome::Loss => children.iter().any(|c| c.repetition_seen),
            }
        } else {
            children.iter().any(|c| c.repetition_seen)
        };

        ChildSelection {
            best_child,
            second_child,
            pn,
            dn,
            depth,
            best_move,
            solved_outcome: solved.map(|(o, _, _, _, _)| o),
            all_solved,
            repetition_seen,
        }
    }

    fn evaluate_child(
        &mut self,
        pos: &mut Position,
        mv: Move,
        max_depth: u32,
        is_or_node: bool,
    ) -> ChildInfo {
        pos.do_move(mv);
        let child_key = pos.hash();
        let child_is_or = !is_or_node;
        let child_path_code = self.path_code ^ zobrist::path_random(mv, self.path_stack.len());

        let info = if let Some(outcome) = pos.outcome() {
            let (pn, dn) = outcome.pn_dn_for(child_is_or);
            ChildInfo {
                mv,
                pn,
                dn,
                vpn: pn,
                vdn: dn,
                outcome: Some(outcome),
                depth: 0,
                repetition_seen: false,
            }
        } else if self.path.contains(&child_key) {
            let (pn, dn) = Outcome::Draw.pn_dn_for(child_is_or);
            ChildInfo {
                mv,
                pn,
                dn,
                vpn: pn,
                vdn: dn,
                outcome: Some(Outcome::Draw),
                depth: 0,
                repetition_seen: true,
            }
        } else if let Some(entry) = self.tt.probe(child_key).copied() {
            let child_max_depth = max_depth.saturating_sub(1);
            if let Some(resolved) = self.try_use_tt(pos, &entry, child_max_depth, child_path_code) {
                let (pn, dn) = resolved.outcome.pn_dn_for(child_is_or);
                ChildInfo {
                    mv,
                    pn,
                    dn,
                    vpn: pn,
                    vdn: dn,
                    outcome: Some(resolved.outcome),
                    depth: resolved.depth,
                    repetition_seen: resolved.repetition_seen,
                }
            } else {
                let use_as_unsolved = entry.outcome.is_none() && entry.depth <= child_max_depth;
                let (pn, dn) = if use_as_unsolved {
                    (entry.pn, entry.dn)
                } else {
                    (1, 1)
                };
                ChildInfo {
                    mv,
                    pn,
                    dn,
                    vpn: pn,
                    vdn: dn,
                    outcome: None,
                    depth: 0,
                    repetition_seen: entry.repetition_seen,
                }
            }
        } else {
            ChildInfo {
                mv,
                pn: 1,
                dn: 1,
                vpn: 1,
                vdn: 1,
                outcome: None,
                depth: 0,
                repetition_seen: false,
            }
        };

        pos.undo_move(mv);
        info
    }

    fn is_solved_by_children(
        children: &[ChildInfo],
        _is_or_node: bool,
    ) -> Option<(Outcome, u32, Move, bool, usize)> {
        let mut all_solved = true;
        let mut win_idx: Option<usize> = None;
        let mut win_depth = u32::MAX;
        let mut draw_idx: Option<usize> = None;
        let mut draw_depth = 0;
        let mut found_draw = false;
        let mut loss_idx: Option<usize> = None;
        let mut loss_depth = 0;

        for (i, c) in children.iter().enumerate() {
            let d = c.depth.saturating_add(1);
            match c.outcome {
                None => {
                    all_solved = false;
                }
                Some(Outcome::Loss) => {
                    // Prefer shortest loss, and among ties prefer path-independent.
                    if d < win_depth
                        || (d == win_depth
                            && win_idx.is_some()
                            && !c.repetition_seen
                            && children[win_idx.unwrap()].repetition_seen)
                    {
                        win_depth = d;
                        win_idx = Some(i);
                    }
                }
                Some(Outcome::Draw) => {
                    if d > draw_depth
                        || (d == draw_depth
                            && draw_idx.is_some()
                            && !c.repetition_seen
                            && children[draw_idx.unwrap()].repetition_seen)
                    {
                        draw_depth = d;
                        draw_idx = Some(i);
                    }
                    found_draw = true;
                }
                Some(Outcome::Win) => {
                    if d > loss_depth
                        || (d == loss_depth
                            && loss_idx.is_some()
                            && !c.repetition_seen
                            && children[loss_idx.unwrap()].repetition_seen)
                    {
                        loss_depth = d;
                        loss_idx = Some(i);
                    }
                }
            }
        }

        if let Some(idx) = win_idx {
            return Some((Outcome::Win, win_depth, children[idx].mv, all_solved, idx));
        }

        if all_solved {
            if found_draw {
                let idx = draw_idx.unwrap_or(0);
                return Some((Outcome::Draw, draw_depth, children[idx].mv, true, idx));
            }
            let idx = loss_idx.unwrap_or(0);
            return Some((Outcome::Loss, loss_depth, children[idx].mv, true, idx));
        }

        None
    }

    fn best_and_second_unsolved(
        children: &[ChildInfo],
        is_or_node: bool,
    ) -> (Option<usize>, Option<usize>) {
        let mut best: Option<usize> = None;
        let mut second: Option<usize> = None;

        for i in 0..children.len() {
            if children[i].outcome.is_some() {
                continue;
            }
            let cmp_c = if is_or_node {
                children[i].vpn
            } else {
                children[i].vdn
            };
            match best {
                None => {
                    best = Some(i);
                }
                Some(b) => {
                    let cmp_best = if is_or_node {
                        children[b].vpn
                    } else {
                        children[b].vdn
                    };
                    if cmp_c < cmp_best {
                        second = best;
                        best = Some(i);
                    } else {
                        match second {
                            None => {
                                second = Some(i);
                            }
                            Some(s) => {
                                let cmp_second = if is_or_node {
                                    children[s].vpn
                                } else {
                                    children[s].vdn
                                };
                                if cmp_c < cmp_second {
                                    second = Some(i);
                                }
                            }
                        }
                    }
                }
            }
        }
        (best, second)
    }

    fn sort_moves(&self, pos: &Position, moves: &mut MoveList, best_from_tt: Move) {
        let mut state = atomic_movegen::board::StateInfo::new();
        pos.board.populate_state(&mut state);

        let slice = moves.as_mut_slice();
        if best_from_tt != Move::NONE
            && let Some(idx) = slice.iter().position(|&m| m == best_from_tt)
        {
            slice.swap(0, idx);
        }

        slice.sort_by(|&a, &b| {
            let sa = self.scorer.score(&pos.board, a, &state);
            let sb = self.scorer.score(&pos.board, b, &state);
            sb.cmp(&sa)
        });
    }

    fn extract_pv(&self, pos: &Position) -> Vec<Move> {
        let mut pv = Vec::new();
        let mut seen = HashSet::new();
        let mut current = pos.clone();
        let mut path_code = 0u64;
        for _ in 0..1000 {
            let key = current.hash();
            if seen.contains(&key) {
                break;
            }
            if current.outcome().is_some() {
                break;
            }
            if let Some(entry) = self.tt.probe(key) {
                let resolved = entry.best_result_for_path(path_code);
                if let Some((mv, Some(_), _)) = resolved {
                    if mv == Move::NONE {
                        break;
                    }
                    seen.insert(key);
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
        pv
    }

    fn should_print_update(
        &self,
        outcome: Outcome,
        depth: u32,
        last_printed: Option<(Outcome, u32)>,
    ) -> bool {
        let Some((last_outcome, last_depth)) = last_printed else {
            return true;
        };
        if outcome != last_outcome {
            return true;
        }
        match outcome {
            Outcome::Win => depth < last_depth,
            Outcome::Loss => depth > last_depth,
            Outcome::Draw => depth != last_depth,
        }
    }

    fn print_pv_update(&mut self, outcome: Outcome, pv: &[Move]) {
        self.last_pv = pv.to_vec();
        let outcome_str = match outcome {
            Outcome::Win => "win",
            Outcome::Loss => "loss",
            Outcome::Draw => "draw",
        };
        let pv_str: String = pv
            .iter()
            .map(|&m| move_to_uci(m))
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!("outcome: {outcome_str}");
        eprintln!("pv: {pv_str}");
        eprintln!("nodes: {}", self.nodes);
    }
}

struct ChildInfo {
    mv: Move,
    pn: u64,
    dn: u64,
    vpn: u64,
    vdn: u64,
    outcome: Option<Outcome>,
    depth: u32,
    repetition_seen: bool,
}

struct ChildSelection {
    best_child: (Move, u64, u64, u64, u64),
    second_child: (u64, u64),
    pn: u64,
    dn: u64,
    depth: u32,
    best_move: Move,
    solved_outcome: Option<Outcome>,
    all_solved: bool,
    repetition_seen: bool,
}

struct Resolved {
    outcome: Outcome,
    depth: u32,
    repetition_seen: bool,
}

pub fn outcome_from_pn_dn(pn: u64, dn: u64) -> Option<Outcome> {
    if pn == 0 && dn == INF {
        Some(Outcome::Win)
    } else if pn == INF && dn == 0 {
        Some(Outcome::Loss)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomic_movegen::types::Square;

    fn child(outcome: Option<Outcome>, depth: u32, from: Square, to: Square) -> ChildInfo {
        ChildInfo {
            mv: Move::make_move(from, to),
            pn: 1,
            dn: 1,
            vpn: 1,
            vdn: 1,
            outcome,
            depth,
            repetition_seen: false,
        }
    }

    #[test]
    fn win_picks_shortest_loss_child() {
        let children = vec![
            child(Some(Outcome::Loss), 5, Square::A1, Square::A2),
            child(Some(Outcome::Loss), 2, Square::B1, Square::B2),
            child(Some(Outcome::Win), 0, Square::C1, Square::C2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, true).unwrap();
        assert_eq!(outcome, Outcome::Win);
        assert_eq!(depth, 3);
        assert_eq!(mv, Move::make_move(Square::B1, Square::B2));
        assert!(all_solved);
    }

    #[test]
    fn draw_picks_longest_draw_child() {
        let children = vec![
            child(Some(Outcome::Win), 4, Square::A1, Square::A2),
            child(Some(Outcome::Draw), 1, Square::B1, Square::B2),
            child(Some(Outcome::Draw), 7, Square::C1, Square::C2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, false).unwrap();
        assert_eq!(outcome, Outcome::Draw);
        assert_eq!(depth, 8);
        assert_eq!(mv, Move::make_move(Square::C1, Square::C2));
        assert!(all_solved);
    }

    #[test]
    fn loss_picks_longest_win_child() {
        let children = vec![
            child(Some(Outcome::Win), 2, Square::A1, Square::A2),
            child(Some(Outcome::Win), 5, Square::B1, Square::B2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, true).unwrap();
        assert_eq!(outcome, Outcome::Loss);
        assert_eq!(depth, 6);
        assert_eq!(mv, Move::make_move(Square::B1, Square::B2));
        assert!(all_solved);
    }

    #[test]
    fn unsolved_returns_none() {
        let children = vec![
            child(Some(Outcome::Win), 0, Square::A1, Square::A2),
            child(None, 0, Square::B1, Square::B2),
        ];
        assert!(Search::is_solved_by_children(&children, true).is_none());
    }

    #[test]
    fn win_with_unsolved_returns_not_all_solved() {
        let children = vec![
            child(Some(Outcome::Loss), 5, Square::A1, Square::A2),
            child(None, 0, Square::B1, Square::B2),
        ];
        let (outcome, depth, mv, all_solved, _idx) =
            Search::is_solved_by_children(&children, true).unwrap();
        assert_eq!(outcome, Outcome::Win);
        assert_eq!(depth, 6);
        assert_eq!(mv, Move::make_move(Square::A1, Square::A2));
        assert!(!all_solved);
    }
}
