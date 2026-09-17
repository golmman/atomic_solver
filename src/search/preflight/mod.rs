//! Detector-gated bounded pre-phase for small decisive regions (plan13,
//! architecture R: region-closure fixpoint).
//!
//! This file is slightly larger than the 10 KiB guideline because it carries
//! the module's soundness contract (the bullet list below is the normative
//! documentation reviewers and external readers rely on) plus the detector,
//! report types, and the claim/defer orchestration; splitting the contract
//! prose from the code it governs would hurt maintainability. It stays well
//! under the ~20 KiB split threshold.
//!
//! On positions passing the detector (at most 3 men, no pawns, no castling
//! rights — plan13 R3), a root-only pre-phase decides the game value before
//! the DF-PN loop runs:
//!
//! 1. **Closure** — forward BFS over the reachable board-only region with
//!    exact, collision-free packed keys, within a position budget
//!    ([`REGION_BUDGET`]).
//! 2. **Fixpoint** — AND/OR relabeling with exact mate distances over the
//!    closure (table-free; undecided remainder = board-only Draw).
//! 3. **Guards** — rule50 rank guard for decisive claims (D3:
//!    `halfmove + rank ≤ 99`), optional bound check
//!    (`rank ≤ max_depth` under `search_depth`), position budget.
//! 4. **Certificate** — rank-decreasing strategy extraction, then the
//!    standalone replay verifier (`verifier`): the claim is only returned
//!    when the strategy replays cycle-free and mates within the rank from
//!    the real root position.
//! 5. **Claim or defer** — a claim returns the outcome plus the
//!    rank-decreasing principal-line PV (exact rank length); anything else
//!    defers and the ordinary search runs bit-identically.
//!
//! # Soundness contract
//!
//! - **No TT interaction of any kind** (no stores, no probes, no bound
//!   folding): the plan10/11 hazard is excluded by construction. The
//!   pre-phase's internal state is run-local and dropped after the call.
//! - **Draw claims** (D2) rest on the monotonicity lemma: the closure's
//!   board-only Draw is a genuine Draw under the solver's repetition and
//!   rule50 semantics. Draw claims carry no certificate and an empty PV.
//! - **Decisive claims** are replay-verified (cycle-free certificate);
//!   verification failure downgrades to deferred, never to a claim.
//! - **No `ProofEvent` emission** (R2, accepted gap): the proof pipeline is
//!   TT-driven offline and cannot reproduce a pre-phase proof regardless, so
//!   the "every proven node emits an event" property documented for `dfpn`
//!   deliberately does not extend to the pre-phase.
//! - **Budget accounting** (R4): pre-phase child evaluations count against
//!   `child_eval_budget` and are reported by `child_evaluations()`. On
//!   budget exhaustion the pre-phase defers and the normal search then
//!   reports `ExitReason::BudgetExhausted` as documented. The pre-phase's
//!   own caps are the deterministic eval budget and the region budget — it
//!   never consults the wall clock (the global timeout stays the search's).
//! - **Memory**: the closure is transient, bounded by [`REGION_BUDGET`]
//!   positions (~85 MB worst case at the 1M budget; the measured `KQvK`
//!   ladder region of 420,532 positions stays around ~35 MB, measured as the
//!   max-RSS delta against `--no-preflight` on the release build). This is the
//!   documented bounded exception to the search CLI's "RAM = TT only".

mod region;
mod verifier;

#[cfg(test)]
mod tests;

use std::sync::atomic::AtomicBool;

use atomic_movegen::board::Board;
use atomic_movegen::types::{Move, MoveList, PieceType};

use crate::position::{Outcome, Position};

pub(crate) use region::region_key;
use verifier::verify_certificate;

/// Maximum number of positions the closure may contain before aborting
/// (D4). The default covers the entire legal 3-man space (the measured `KQvK`
/// ladder region is 420,532 positions; larger components were observed in
/// the Phase 0 oracle) and aborts — never claims — above it.
pub const REGION_BUDGET: usize = 1_000_000;

/// Why the pre-phase did not claim, or (for decided reports) `""`.
///
/// Deferral reasons: `disabled` (`--no-preflight`), `detector` (predicate
/// false), `eval-budget`, `region-budget`, `stop`, `rank-guard` (rule50
/// guard, D3), `bound` (rank exceeds the `search_depth` bound), `cert-fail`
/// (strategy extraction or replay verification failed — defense in depth),
/// `internal` (an invariant violation; always defer).
pub type PreflightReason = &'static str;

/// Outcome of the pre-phase hook, reported via `Search::preflight_report()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreflightReport {
    /// Whether the pre-phase returned a decided outcome (true) or deferred.
    pub decided: bool,
    /// `""` for decided reports, otherwise one of the deferral reasons
    /// documented on [`PreflightReason`].
    pub reason: PreflightReason,
    /// Child evaluations consumed by the pre-phase (closure BFS, strategy
    /// verification, and PV extraction).
    pub evals: u64,
    /// Positions in the closure (0 when the detector did not fire).
    pub region: usize,
    /// The claimed outcome; `None` on deferral.
    pub outcome: Option<Outcome>,
    /// Exact rank (mate distance) of a decisive claim; 0 otherwise.
    pub rank: u32,
}

impl PreflightReport {
    pub(crate) fn deferred(reason: PreflightReason, evals: u64, region: usize) -> Self {
        Self {
            decided: false,
            reason,
            evals,
            region,
            outcome: None,
            rank: 0,
        }
    }

    pub(crate) fn decided(outcome: Outcome, evals: u64, region: usize, rank: u32) -> Self {
        Self {
            decided: true,
            reason: "",
            evals,
            region,
            outcome: Some(outcome),
            rank,
        }
    }
}

/// A claimed outcome plus its certificate PV.
pub(crate) struct PreflightClaim {
    pub outcome: Outcome,
    pub pv: Vec<Move>,
    pub evals: u64,
    pub region: usize,
    pub rank: u32,
}

/// Exit of the pre-phase run.
pub(crate) enum PreflightExit {
    Claimed(Box<PreflightClaim>),
    Deferred {
        reason: PreflightReason,
        evals: u64,
        region: usize,
    },
}

/// The plan13 R3 detector predicate: at most 3 men, no pawns (promotions and
/// clock resets would break the monotone closure), no castling rights (extra
/// state that cannot occur in the round-1 class). En passant needs pawns and
/// is therefore transitively excluded.
pub(crate) fn detector_applies(board: &Board) -> bool {
    board.occupied().count() <= 3
        && board.pieces_pt(PieceType::Pawn).is_empty()
        && board.castling_rights() == 0
}

/// Run the pre-phase for `pos` (root only).
///
/// `bound` is the certificate line-length bound: `None` under
/// `Search::solve`, `Some(max_depth)` under `Search::search_depth`. Returns
/// a claim only for a fully verified decision; every other path defers with
/// a reason and the consumed child-eval count (which the caller adds to the
/// run's counters).
pub(crate) fn run(
    pos: &Position,
    bound: Option<u32>,
    eval_budget: u64,
    stop: Option<&AtomicBool>,
) -> PreflightExit {
    let board = pos.board();
    // Defense in depth: the caller checked the detector, but the run itself
    // must never claim outside the gated class.
    if !detector_applies(board) {
        return PreflightExit::Deferred {
            reason: "detector",
            evals: 0,
            region: 0,
        };
    }
    if region_key(board).is_none() {
        return PreflightExit::Deferred {
            reason: "internal",
            evals: 0,
            region: 0,
        };
    }

    let analysis = match region::analyze(board, eval_budget, stop, REGION_BUDGET) {
        Ok(analysis) => analysis,
        Err(abort) => {
            return PreflightExit::Deferred {
                reason: abort.reason,
                evals: abort.evals,
                region: abort.region,
            };
        }
    };
    let region = analysis.region;
    let mut evals = analysis.evals;

    let outcome = analysis.root_outcome();
    if outcome == Outcome::Draw {
        // D2: a board-only Draw is a genuine repetition-semantics Draw
        // (monotonicity lemma; clock-independent). No certificate, empty PV.
        return PreflightExit::Claimed(Box::new(PreflightClaim {
            outcome: Outcome::Draw,
            pv: Vec::new(),
            evals,
            region,
            rank: 0,
        }));
    }

    let rank = analysis.root_rank();
    // D3 rank guard: rule50 expiry is a Draw at clock >= 100, and the clock
    // grows by exactly one per ply on these lines (only extinction captures
    // occur, which reset it), so `halfmove + rank <= 99` keeps every
    // certificate position below the boundary. Verified in the tests.
    if u32::from(board.rule50()) + rank > 99 {
        return PreflightExit::Deferred {
            reason: "rank-guard",
            evals,
            region,
        };
    }
    if let Some(bound) = bound
        && rank > bound
    {
        return PreflightExit::Deferred {
            reason: "bound",
            evals,
            region,
        };
    }

    let winner = if outcome == Outcome::Win {
        pos.side_to_move()
    } else {
        pos.side_to_move().flip()
    };
    let Some(strategy) = analysis.build_strategy() else {
        return PreflightExit::Deferred {
            reason: "cert-fail",
            evals,
            region,
        };
    };
    let (ok, stats) =
        verify_certificate(pos, &strategy, winner, rank, &mut evals, eval_budget, stop);
    if !ok {
        let reason = if stats.aborted {
            if evals >= eval_budget {
                "eval-budget"
            } else {
                "stop"
            }
        } else {
            "cert-fail"
        };
        return PreflightExit::Deferred {
            reason,
            evals,
            region,
        };
    }
    let Some(line) = analysis.principal_line() else {
        return PreflightExit::Deferred {
            reason: "cert-fail",
            evals,
            region,
        };
    };
    let Some(pv) = line_to_moves(pos, &line, &mut evals, eval_budget) else {
        return PreflightExit::Deferred {
            reason: "cert-fail",
            evals,
            region,
        };
    };
    if pv.len() as u32 != rank {
        return PreflightExit::Deferred {
            reason: "cert-fail",
            evals,
            region,
        };
    }

    PreflightExit::Claimed(Box::new(PreflightClaim {
        outcome,
        pv,
        evals,
        region,
        rank,
    }))
}

/// Convert the principal-line child keys into real moves by replaying from
/// the root with real movegen (one child evaluation per candidate move).
fn line_to_moves(
    pos: &Position,
    line: &[u32],
    evals: &mut u64,
    eval_budget: u64,
) -> Option<Vec<Move>> {
    let mut cur = pos.clone();
    let mut pv = Vec::with_capacity(line.len());
    for &target in line {
        let mut moves = MoveList::new();
        cur.legal_moves(&mut moves);
        let mut found: Option<Move> = None;
        for i in 0..moves.len() {
            *evals += 1;
            if *evals >= eval_budget {
                return None;
            }
            cur.do_move(moves[i]);
            let key = region_key(cur.board());
            cur.undo_move(moves[i]);
            if key == Some(target) {
                found = Some(moves[i]);
                break;
            }
        }
        let mv = found?;
        cur.do_move(mv);
        pv.push(mv);
    }
    Some(pv)
}

// Test-only access to region internals; the region module is the owner.
#[cfg(test)]
use region::{board_from_key, classify_terminal};
