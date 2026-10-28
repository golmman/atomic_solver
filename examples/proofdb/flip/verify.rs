//! The independent verifier over the converged claim map.
//!
//! Every reported flip is *recomputed*, not trusted: each claim is verified by
//! a recursive walk that replays the row from scratch (fresh `Position`, fresh
//! movegen), re-derives the claim from the final state — including its
//! coverage, bound mode and diagnostic counts — and compares the whole record.
//! It shares only the immutable row index and the converged claim map with the
//! fixpoint in [`super`]; it never consults the fixpoint's intermediate state.
//!
//! The asymmetry of the claim polarities is enforced here (see the module
//! header of [`super`] for the normative text): a side-to-move-**loss** claim
//! requires and-complete coverage and names the offending reply, while a
//! side-to-move-**win** claim recurses only into its loss witnesses and needs
//! at least one of them.

use atomic_solver::notation::move_to_uci;
use atomic_solver::position::Outcome;

use super::{Analysis, BoundMode, Claim, Coverage, DepthStatus, Flip};

/// Recursion guard (a claim chain deeper than this is a defect).
const MAX_DEPTH: usize = 200;

/// One reply of a row being verified.
enum Reply {
    /// A proven DB row, or an implied claim (`implied` = its path).
    Resolved {
        loss: bool,
        bound: u32,
        exact: bool,
        implied: Option<String>,
    },
    /// No DB row at this path (e.g. a censored sibling).
    NoRow,
    /// An open DB row with no converged claim.
    Unresolved,
}

impl Analysis<'_> {
    /// Independently verify one claim (value and metadata).
    ///
    /// # Errors
    /// A replay failure, a flip at a proven row, a terminal row, a
    /// loss-claim coverage violation (naming the reply), a win claim without
    /// a loss witness, a nested claim that fails, or a mismatch between the
    /// stored and the recomputed claim.
    pub fn verify(&self, path: &str, want: &Claim, depth: usize) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err("verification recursion limit".to_string());
        }
        let idx = *self
            .by_path
            .get(path)
            .ok_or_else(|| format!("flip path {path:?} not a DB row"))?;
        if self.rows[idx].outcome.is_some() {
            return Err(format!("flip at proven row {path:?}"));
        }
        let pos = self.replay(path)?;
        let moves = pos.legal_moves_vec();
        if moves.is_empty() {
            return Err(format!("flip {path:?}: no legal replies (terminal)"));
        }
        let mut replies: Vec<(String, Reply)> = Vec::with_capacity(moves.len());
        for mv in moves {
            let uci = move_to_uci(mv);
            let key = if path.is_empty() {
                uci.clone()
            } else {
                format!("{path} {uci}")
            };
            replies.push((uci, self.reply(&key)));
        }
        if !want.value.stm_win {
            // The strict polarity: and-complete coverage is mandatory.
            for (uci, r) in &replies {
                match r {
                    Reply::NoRow => {
                        return Err(format!(
                            "flip {path:?}: reply {uci} has no DB row \
                             (a loss claim requires and-complete coverage)"
                        ));
                    }
                    Reply::Unresolved => {
                        return Err(format!(
                            "flip {path:?}: reply {uci} unresolved \
                             (a loss claim requires and-complete coverage)"
                        ));
                    }
                    Reply::Resolved {
                        implied: Some(key), ..
                    } => {
                        let child = self
                            .claim(key)
                            .ok_or_else(|| format!("flip {path:?}: reply {uci} unresolved"))?;
                        self.verify(key, child, depth + 1)?;
                    }
                    Reply::Resolved { .. } => {}
                }
            }
        }
        let expect = self.expected(path, &replies, depth)?;
        if expect != *want {
            return Err(format!("flip {path:?}: want {want:?}, verified {expect:?}"));
        }
        Ok(())
    }

    /// Verify every converged claim (value *and* metadata); returns the count.
    ///
    /// # Errors
    /// The first defect, as reported by [`Self::verify`].
    pub fn verify_all(&self) -> Result<usize, String> {
        let mut verified = 0usize;
        for (path, _, _) in self.order() {
            let want = self
                .claim(path)
                .ok_or_else(|| format!("flip {path:?} has no converged claim"))?;
            self.verify(path, want, 0)?;
            verified += 1;
        }
        Ok(verified)
    }

    /// The resolved status of one reply path (final state, no recursion).
    fn reply(&self, key: &str) -> Reply {
        if let Some(&idx) = self.by_path.get(key) {
            let row = &self.rows[idx];
            if let Some(outcome) = row.outcome {
                return Reply::Resolved {
                    loss: outcome == Outcome::Loss,
                    bound: row.depth_bound.unwrap_or(0),
                    exact: row.depth_status == Some(DepthStatus::Exact),
                    implied: None,
                };
            }
        }
        match self.claim(key) {
            Some(claim) => Reply::Resolved {
                loss: !claim.value.stm_win,
                bound: claim.value.bound,
                exact: claim.bound_mode == BoundMode::Exact,
                implied: Some(key.to_string()),
            },
            None => {
                if self.by_path.contains_key(key) {
                    Reply::Unresolved
                } else {
                    Reply::NoRow
                }
            }
        }
    }

    /// Recompute a claim from a reply census, recursing into implied witnesses
    /// (for a win claim: the loss witnesses, as the OR rule prescribes).
    fn expected(
        &self,
        path: &str,
        replies: &[(String, Reply)],
        depth: usize,
    ) -> Result<Claim, String> {
        let mut witnesses: Vec<(u32, bool, Option<&str>)> = Vec::new();
        let mut max_bound = 0u32;
        let mut all_exact = true;
        let mut unresolved = 0u32;
        for (_, r) in replies {
            match r {
                Reply::Resolved {
                    loss,
                    bound,
                    exact,
                    implied,
                } => {
                    max_bound = max_bound.max(*bound);
                    all_exact &= *exact;
                    if *loss {
                        witnesses.push((*bound, *exact, implied.as_deref()));
                    }
                }
                _ => unresolved += 1,
            }
        }
        if witnesses.is_empty() {
            return if unresolved == 0 {
                Ok(Claim {
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
                Err(format!(
                    "flip {path:?}: no loss witness \
                     (a win claim requires at least one resolved loss reply)"
                ))
            };
        }
        for (_, _, implied) in &witnesses {
            if let Some(key) = implied {
                let child = self.claim(key).ok_or_else(|| {
                    format!("flip {path:?}: witness {key:?} has no converged claim")
                })?;
                self.verify(key, child, depth + 1)?;
            }
        }
        let min = witnesses.iter().map(|w| w.0).min().unwrap_or(0);
        let complete = unresolved == 0;
        let exact = complete && witnesses.iter().filter(|w| w.0 == min).all(|w| w.1);
        Ok(Claim {
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
    }
}
