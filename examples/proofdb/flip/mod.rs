//! The implied-flip analysis: what a grown DB's stored facts already imply,
//! and how strongly.
//!
//! **Semantics (normative).** A *flip* is a row whose stored facts already
//! imply a decisive outcome — an **open** row by construction (a proven row
//! is never flipped, so a flip never contradicts a stored outcome). For an
//! open row `R` with legal replies, each reply carries a *resolved status*:
//! the DB row's outcome + depth bound (proven), or the reply's own implied
//! claim (open), or nothing (unresolved — an absent row, e.g. a censored
//! sibling: censors are scheduling state, the tree model has no row for
//! them). The two claim polarities are **asymmetric**:
//!
//! - **side-to-move-win** — sound iff *some* reply is a resolved loss (an
//!   `∃`-witness: the row's mover picks that move and wins). Bind
//!   `1 + min{bound of the loss witnesses}`. Unresolved siblings never
//!   weaken the *outcome*; they can only hide a shorter win, so the bound is
//!   an upper bound (`coverage = or-witness`, `bound_mode = upper`).
//! - **side-to-move-loss** — sound iff **every** reply is resolved and none
//!   is a loss (an `∀`-completion: the defender's longest resistance). Bind
//!   `1 + max{bound}`. Full coverage is required
//!   (`coverage = and-complete`).
//!
//! This supersedes the plan8 §4.5 AND-completeness gate for side-to-move-win
//! claims (an OR claim needs one witness, not coverage). Routing note:
//! post-plan21 layers legitimately carry or-witness claims — or-close never
//! re-searches the censored siblings of a proven loss child, so such a
//! missing row is permanent, not a transient harvest gap. A *root* or-witness
//! claim is a sound startpos outcome claim; only its bound is an upper bound.
//!
//! **Exactness.** `or-witness` bounds are always `upper`. An `and-complete`
//! bound is `exact` iff every contributing child bound is exact — proven
//! children with `depth_status = 'exact'`, implied children whose own claim
//! is exact; contributors are the min-achieving loss witnesses for a win
//! claim and *all* replies for a loss claim. (22,808 of the standing layer's
//! 43,092 rows are `depth_status = 'bound'`, so and-complete alone cannot
//! imply minimality — a correctness-first refusal to claim it.)
//!
//! **The fixpoint is converged on values *and* metadata.** Every pass
//! recomputes every open row's claim *from the previous pass's state* (never
//! from the pass in progress, which keeps passes order-independent), and the
//! loop stops when the whole claim map — value, bound, coverage, bound mode,
//! diagnostics — is unchanged. A derive-once claim is history-dependent and
//! can emit a bound the independent verifier cannot reproduce.
//!
//! Factorisation (plan26 D5; every file under the 10 KB convention): this
//! module owns the types, the analysis state, the replay helper and the
//! accessors; the `fixpoint` submodule owns the converged pass and the reply
//! resolution, `verify` the independent recursive verifier (replay-from-
//! scratch, claim recomputation, metadata comparison), and `tests` the unit
//! fixtures. Read-only: nothing here writes DB state.

mod fixpoint;
#[cfg(test)]
mod tests;
mod verify;

use std::collections::HashMap;

use atomic_solver::notation::uci_to_move;
use atomic_solver::position::Position;

use super::db::DbRow;
use super::merge::DepthStatus;

/// One implied row value: side-to-move outcome + depth bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flip {
    /// `true` = a win for the row's side to move.
    pub stm_win: bool,
    pub bound: u32,
}

impl Flip {
    /// Root-player polarity for the report (parity conversion).
    #[must_use]
    pub fn root_label(&self, ply: usize) -> &'static str {
        if self.stm_win == ply.is_multiple_of(2) {
            "root-win"
        } else {
            "root-loss"
        }
    }
}

/// How much of a row's reply set a claim rests on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// One resolved loss reply proves the win; siblings may be unresolved.
    OrWitness,
    /// Every reply is resolved.
    AndComplete,
}

impl Coverage {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OrWitness => "or-witness",
            Self::AndComplete => "and-complete",
        }
    }
}

/// Whether a claim's bound is the true distance-to-terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundMode {
    /// An upper bound (unresolved siblings, or a `bound`-status contributor).
    Upper,
    /// Every contributing child bound is proven minimal.
    Exact,
}

impl BoundMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upper => "upper",
            Self::Exact => "exact",
        }
    }
}

/// A derived claim plus the metadata that qualifies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Claim {
    pub value: Flip,
    pub coverage: Coverage,
    pub bound_mode: BoundMode,
    /// Resolved replies that are losses for their own side to move.
    pub loss_witnesses: u32,
    /// Replies with no resolved status (absent rows or claim-less open rows).
    pub unresolved_replies: u32,
}

/// The analysis over one loaded DB content.
pub struct Analysis<'a> {
    rows: &'a [DbRow],
    pub(super) by_path: HashMap<String, usize>,
    root_fen: String,
    /// The converged claim map (path → claim); empty before `solve`.
    claims: HashMap<String, Claim>,
    /// Discovery order: (path, ply, first-derivation round).
    order: Vec<(String, usize, u32)>,
    rounds: u32,
}

impl<'a> Analysis<'a> {
    /// Set up the analysis over `rows`, replaying from `root_fen` (the DB's
    /// `meta.root_fen`; `load_db_rows` already refuses a non-startpos root).
    ///
    /// # Errors
    /// A row set without a root row.
    pub fn new(rows: &'a [DbRow], root_fen: &str) -> Result<Self, String> {
        let by_path: HashMap<String, usize> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.path.join(" "), i))
            .collect();
        if !by_path.contains_key("") {
            return Err("DB has no root row".to_string());
        }
        Ok(Self {
            rows,
            by_path,
            root_fen: root_fen.to_string(),
            claims: HashMap::new(),
            order: Vec::new(),
            rounds: 0,
        })
    }

    /// Open rows (the fixpoint's domain and the report's `open_rows`).
    #[must_use]
    pub fn open_rows(&self) -> usize {
        self.rows.iter().filter(|r| r.outcome.is_none()).count()
    }

    /// The converged claim of `path`, if it has one.
    #[must_use]
    pub fn claim(&self, path: &str) -> Option<&Claim> {
        self.claims.get(path)
    }

    /// The startpos claim, if the root row is implied.
    #[must_use]
    pub fn root(&self) -> Option<&Claim> {
        self.claims.get("")
    }

    /// Every claim, in discovery order, as `(path, ply, first-derivation
    /// round)`.
    #[must_use]
    pub fn order(&self) -> &[(String, usize, u32)] {
        &self.order
    }

    /// The last fixpoint pass count (set by [`Self::solve`]).
    #[must_use]
    pub fn rounds(&self) -> u32 {
        self.rounds
    }

    /// Replay `path` from the DB's root (fresh position per call).
    ///
    /// # Errors
    /// A malformed FEN or an illegal move on the path.
    pub(super) fn replay(&self, path: &str) -> Result<Position, String> {
        let mut pos = Position::from_fen(&self.root_fen).map_err(|e| e.to_string())?;
        for uci in path.split(' ').filter(|s| !s.is_empty()) {
            let mv = uci_to_move(uci, &pos)
                .ok_or_else(|| format!("illegal move {uci:?} on path {path:?}"))?;
            pos.do_move(mv);
        }
        Ok(pos)
    }
}

/// Space-joined child path (the DB's path-key convention).
fn child_path(path: &str, uci: &str) -> String {
    if path.is_empty() {
        uci.to_string()
    } else {
        format!("{path} {uci}")
    }
}
