//! Random-playout cross-check of the solver-side existence query and
//! terminal classifier against the full move-generation path (lean item #15).
//!
//! The hot path (`evaluate_child` in `src/search/dfpn/children.rs`) decides
//! child terminality without generating move lists: it calls
//! `Position::has_legal_move` (upstream `has_legal_move_with_state`, fed a
//! caller-populated `StateInfo`) for ~95% of evaluated children, then
//! classifies via `Position::outcome_from_state` + the checkers/occupied
//! board bits. This test locks that path on live playout positions:
//!
//! - **P1** — `pos.has_legal_move(&state)` agrees with the full legal
//!   move list emptiness, both derived from one populated `StateInfo`
//!   (mirroring the hot-path populate-once pattern).
//! - **P2** — the outcome reconstructed from the fast-path signals
//!   (extinction bitboards, checkers bit, `rule50`, `occupied == 2`)
//!   equals `pos.outcome_from_state(&state, &moves)`.
//! - **P3** — `pos.outcome()` (which re-derives the move list internally)
//!   equals the same classification.
//! - **P4** — branch-coverage guard: the aggregate counters must show at
//!   least one checkmate-terminal, one stalemate-draw-terminal, and one
//!   extinction-terminal position, so the property test cannot silently
//!   degenerate into "both sides return `false`/`None` on quiet
//!   middlegames".
//!
//! No `src/` change is involved; if any assertion fires, the failure
//! message carries the hard-coded seed set label, playout index, ply, and
//! position FEN for exact reproduction.

mod common;

use atomic_movegen::board::StateInfo;
use atomic_movegen::movegen::generate_legal_with_state;
use atomic_movegen::types::MoveList;
use atomic_solver::position::{Outcome, Position};

/// Deterministic splitmix64 PRNG (no `rand` dependency; determinism is a
/// requirement, not a nicety).
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform-enough index in `0..n` for move picking (modulo bias is
    /// irrelevant at move-list sizes).
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// FNV-1a-style seed derivation from the FEN and playout index so every
/// game has its own deterministic stream.
fn game_seed(fen: &str, game: usize) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ (game as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for b in fen.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Aggregate branch-coverage counters (P4).
#[derive(Debug, Default, Clone, Copy)]
struct Coverage {
    positions_checked: u64,
    checkmate_loss_terminals: u64,
    stalemate_draw_terminals: u64,
    extinction_terminals: u64,
}

/// Reconstruct the terminal outcome purely from the fast-path signals the
/// hot path uses: the extinction commoner bitboards, the checkers bit,
/// `rule50`, and the `occupied == 2` popcount. `moves_empty` comes from
/// the generated list, cross-checked against `has_legal_move` by P1.
fn classify_from_fast_signals(
    pos: &Position,
    state: &StateInfo,
    moves_empty: bool,
) -> Option<Outcome> {
    let us = pos.side_to_move();
    let them = us.flip();
    if pos.commoners(us).is_empty() {
        return Some(Outcome::Loss);
    }
    if pos.commoners(them).is_empty() {
        return Some(Outcome::Win);
    }
    if moves_empty {
        if state.checkers.is_empty() {
            return Some(Outcome::Draw);
        }
        return Some(Outcome::Loss);
    }
    if pos.board().rule50() >= 100 {
        return Some(Outcome::Draw);
    }
    if pos.board().occupied().count() == 2 {
        return Some(Outcome::Draw);
    }
    None
}

/// Assert P1/P2/P3 (and the position invariants) at one visited position,
/// then update the P4 counters.
fn check_position(
    ctx: &PlyContext,
    pos: &Position,
    state: &StateInfo,
    moves: &MoveList,
    cov: &mut Coverage,
) {
    cov.positions_checked += 1;

    // P1 — existence equivalence (one populated StateInfo for both paths).
    let has = pos.has_legal_move(state);
    assert_eq!(
        has,
        !moves.is_empty(),
        "P1 existence mismatch ({ctx}): has_legal_move={has}, {} legal moves",
        moves.len()
    );

    // P2 — fast-path classification agreement.
    let fast = classify_from_fast_signals(pos, state, moves.is_empty());
    let classified = pos.outcome_from_state(state, moves);
    assert_eq!(
        fast, classified,
        "P2 classification mismatch ({ctx}): fast-path={fast:?}, outcome_from_state={classified:?}"
    );

    // P2 sharpening: with legal moves present the position must not be
    // mate/stalemate-terminal. Any decisive outcome here can only come
    // from commoner extinction (the move-list-independent branches);
    // rule50/occupied==2 draws are also independent of the move list.
    if !moves.is_empty()
        && let Some(outcome) = classified
    {
        let us = pos.side_to_move();
        let them = us.flip();
        let extinction = pos.commoners(us).is_empty() || pos.commoners(them).is_empty();
        assert!(
            extinction || pos.board().rule50() >= 100 || pos.board().occupied().count() == 2,
            "P2 terminal with legal moves but no move-list-independent branch ({ctx}): {outcome:?}"
        );
    }

    // P3 — full-path consistency: `outcome()` re-derives the move list.
    let full_path = pos.outcome();
    assert_eq!(
        full_path, classified,
        "P3 full-path mismatch ({ctx}): outcome()={full_path:?}, outcome_from_state={classified:?}"
    );

    // Optional extra lock: incremental Zobrist hash along playout lines.
    common::assert_position_invariants(pos);

    // P4 bookkeeping (terminal positions only).
    if classified.is_some() {
        let us = pos.side_to_move();
        let them = us.flip();
        if pos.commoners(us).is_empty() || pos.commoners(them).is_empty() {
            cov.extinction_terminals += 1;
        } else if moves.is_empty() {
            if state.checkers.is_empty() {
                cov.stalemate_draw_terminals += 1;
            } else {
                cov.checkmate_loss_terminals += 1;
            }
        }
    }
}

/// One-ply context for failure diagnostics: hard-coded seed-set label,
/// seed FEN, playout index, ply index, and the live position FEN.
struct PlyContext {
    label: &'static str,
    seed_fen: String,
    game: usize,
    ply: usize,
}

impl std::fmt::Display for PlyContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "seed set '{}', seed '{}', playout {}, ply {}",
            self.label, self.seed_fen, self.game, self.ply
        )
    }
}

/// Run `playouts` random games of up to `ply_cap` plies from each seed.
/// The caller provides the live position FEN through `PlyContext` so a
/// failure is exactly reproducible.
fn run_playouts(
    label: &'static str,
    seeds: &[String],
    playouts: usize,
    ply_cap: usize,
) -> Coverage {
    let mut cov = Coverage::default();
    for seed_fen in seeds {
        for game in 0..playouts {
            let mut rng = SplitMix64::new(game_seed(seed_fen, game));
            let mut pos = Position::from_fen(seed_fen)
                .unwrap_or_else(|e| panic!("failed to parse seed FEN '{seed_fen}': {e}"));
            for ply in 0..ply_cap {
                // Hot-path pattern: one StateInfo, populated once, reused
                // for both the existence query and the move generation.
                let mut state = StateInfo::new();
                pos.populate_state(&mut state);
                let mut moves = MoveList::new();
                generate_legal_with_state(pos.board(), &state, &mut moves);

                let ctx = PlyContext {
                    label,
                    seed_fen: seed_fen.clone(),
                    game,
                    ply,
                };
                check_position(&ctx, &pos, &state, &moves, &mut cov);

                if pos.outcome_from_state(&state, &moves).is_some() {
                    break; // terminal; do not move on
                }
                // The move list is the legality filter (integration tests
                // cannot see `try_do_move`, which is `#[cfg(test)]`).
                let mv = moves[rng.below(moves.len())];
                pos.do_move(mv);
            }
        }
    }
    cov
}

/// Assert the P4 branch-coverage guard.
fn assert_branch_coverage(cov: &Coverage) {
    assert!(
        cov.checkmate_loss_terminals >= 1,
        "P4 coverage guard: no checkmate-terminal position was exercised ({cov:?})"
    );
    assert!(
        cov.stalemate_draw_terminals >= 1,
        "P4 coverage guard: no stalemate-draw-terminal position was exercised ({cov:?})"
    );
    assert!(
        cov.extinction_terminals >= 1,
        "P4 coverage guard: no extinction-terminal position was exercised ({cov:?})"
    );
}

/// All playout seeds: the standard start position, every fixture suite,
/// and the tiny endgame seeds that force P4 branch coverage (stalemate
/// draw and bare-kings two-piece draw).
fn seed_positions() -> Vec<String> {
    let mut seeds = vec![Position::STARTPOS_FEN.to_string()];
    seeds.extend(
        common::load_move_order_suite()
            .into_iter()
            .map(|case| case.fen),
    );
    seeds.extend(
        common::load_decisive_suite()
            .into_iter()
            .map(|case| case.fen),
    );
    seeds.extend(common::load_smoke_suite().into_iter().map(|case| case.fen));
    // Stalemate-draw terminal at ply 0.
    seeds.push("7k/8/8/8/8/8/2q5/K7 w - - 0 1".to_string());
    // Bare-kings two-piece draw terminal at ply 0.
    seeds.push("4k3/8/8/8/8/8/8/4K3 w - - 0 1".to_string());
    seeds
}

/// Fast tier: 2 playouts per seed, 120-ply cap. Must stay within ~5 s
/// (release) / ~30 s (debug) wall.
#[test]
fn playout_crosscheck_fast_tier() {
    let seeds = seed_positions();
    let cov = run_playouts("fast", &seeds, 2, 120);
    assert_branch_coverage(&cov);
}

/// Slow tier: 32 playouts per seed and a deeper ply cap.
#[test]
#[ignore = "slow: 32 playouts per seed over the full fixture set (minutes)"]
fn playout_crosscheck_slow_tier() {
    let seeds = seed_positions();
    let cov = run_playouts("slow", &seeds, 32, 200);
    assert_branch_coverage(&cov);
}
