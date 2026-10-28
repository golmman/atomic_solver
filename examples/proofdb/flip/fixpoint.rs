//! The converged claim fixpoint (plan26 D1).
//!
//! Each pass recomputes **every** open row's claim from the *previous* pass's
//! state — never from the pass in progress, which keeps passes
//! order-independent — and the loop stops when the whole claim map (value,
//! bound, coverage, bound mode, diagnostics) is unchanged. A claim first
//! derived in an early pass therefore tightens as later passes resolve its
//! witnesses: derive-once would emit a bound the independent verifier cannot
//! reproduce (the latent defect this module fixes). Monotonicity — claims are
//! never dropped, bounds never increase, polarity is invariant — is
//! `debug_assert`ed and named in the defect errors. The pass ceiling is
//! `open rows + 8`: a tripwire for a broken state machine, not a work bound.

use std::collections::HashMap;

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Outcome;

use super::super::merge::DepthStatus;
use super::{Analysis, BoundMode, Claim, Coverage, Flip, child_path};

/// A reply's resolved status, in the reply position's own side-to-move terms.
#[derive(Debug, Clone, Copy)]
struct Resolved {
    loss: bool,
    bound: u32,
    exact: bool,
}

enum ReplyStatus {
    Resolved(Resolved),
    Unresolved,
}

impl Analysis<'_> {
    /// Converge the claim map on values and metadata; returns the pass count.
    ///
    /// # Errors
    /// A replay failure, a monotonicity violation (a dropped claim or a
    /// raised bound — a defect tripwire, not a work bound), or
    /// non-convergence within `open rows + 8` passes.
    pub fn solve(&mut self) -> Result<u32, String> {
        let open = self.open_rows();
        let ceiling = (open + 8) as u32;
        let mut state: HashMap<String, Claim> = HashMap::new();
        self.order.clear();
        let mut rounds = 0u32;
        loop {
            rounds += 1;
            let mut next: HashMap<String, Claim> = HashMap::new();
            for row in self.rows.iter().filter(|r| r.outcome.is_none()) {
                let path = row.path.join(" ");
                if let Some(claim) = self.classify(&path, &state)? {
                    next.insert(path, claim);
                }
            }
            // D1 monotonicity: claims are never dropped, bounds never increase.
            for (path, old) in &state {
                match next.get(path) {
                    None => {
                        return Err(format!("fixpoint dropped claim {path:?} (not monotone)"));
                    }
                    Some(new) if new.value.bound > old.value.bound => {
                        return Err(format!(
                            "fixpoint raised the bound of {path:?}: {} -> {} (not monotone)",
                            old.value.bound, new.value.bound
                        ));
                    }
                    Some(new) => debug_assert_eq!(
                        new.value.stm_win, old.value.stm_win,
                        "a claim's polarity is invariant: {path:?}"
                    ),
                }
            }
            for path in next.keys() {
                if !state.contains_key(path) {
                    let idx = self.by_path[path];
                    self.order.push((path.clone(), self.rows[idx].ply, rounds));
                }
            }
            if next == state {
                self.claims = state;
                self.rounds = rounds;
                return Ok(rounds);
            }
            if rounds >= ceiling {
                return Err(format!(
                    "fixpoint did not converge in {rounds} passes (open rows {open})"
                ));
            }
            state = next;
        }
    }

    /// Classify one open row from the previous pass's state (order-independent).
    pub(super) fn classify(
        &self,
        path: &str,
        prev: &HashMap<String, Claim>,
    ) -> Result<Option<Claim>, String> {
        let pos = self.replay(path)?;
        let moves = pos.legal_moves_vec();
        if moves.is_empty() {
            return Ok(None); // rule-terminal draw: undecided in this model
        }
        let mut witnesses: Vec<(u32, bool)> = Vec::new();
        let mut max_bound = 0u32;
        let mut all_exact = true;
        let mut unresolved = 0u32;
        for mv in moves {
            let key = child_path(path, &move_to_uci(mv));
            match self.reply_status(&key, prev) {
                ReplyStatus::Resolved(r) => {
                    max_bound = max_bound.max(r.bound);
                    all_exact &= r.exact;
                    if r.loss {
                        witnesses.push((r.bound, r.exact));
                    }
                }
                ReplyStatus::Unresolved => unresolved += 1,
            }
        }
        Ok(if !witnesses.is_empty() {
            let min = witnesses.iter().map(|w| w.0).min().unwrap_or(0);
            let complete = unresolved == 0;
            let exact = complete && witnesses.iter().filter(|w| w.0 == min).all(|w| w.1);
            Some(Claim {
                value: Flip {
                    stm_win: true,
                    bound: min.saturating_add(1),
                },
                coverage: if complete {
                    Coverage::AndComplete
                } else {
                    Coverage::OrWitness
                },
                bound_mode: if exact {
                    BoundMode::Exact
                } else {
                    BoundMode::Upper
                },
                loss_witnesses: witnesses.len() as u32,
                unresolved_replies: unresolved,
            })
        } else if unresolved == 0 {
            Some(Claim {
                value: Flip {
                    stm_win: false,
                    bound: max_bound.saturating_add(1),
                },
                coverage: Coverage::AndComplete,
                bound_mode: if all_exact {
                    BoundMode::Exact
                } else {
                    BoundMode::Upper
                },
                loss_witnesses: 0,
                unresolved_replies: 0,
            })
        } else {
            None
        })
    }

    /// Resolved status of one reply path (proven DB row, else implied claim).
    fn reply_status(&self, key: &str, prev: &HashMap<String, Claim>) -> ReplyStatus {
        if let Some(&idx) = self.by_path.get(key) {
            let row = &self.rows[idx];
            if let Some(outcome) = row.outcome {
                return ReplyStatus::Resolved(Resolved {
                    loss: outcome == Outcome::Loss,
                    bound: row.depth_bound.unwrap_or(0),
                    exact: row.depth_status == Some(DepthStatus::Exact),
                });
            }
        }
        match prev.get(key) {
            Some(claim) => ReplyStatus::Resolved(Resolved {
                loss: !claim.value.stm_win,
                bound: claim.value.bound,
                exact: claim.bound_mode == BoundMode::Exact,
            }),
            None => ReplyStatus::Unresolved,
        }
    }
}
