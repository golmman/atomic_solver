//! `proofdb_flip` — the plan8 §4.5 flip analysis (read-only, D2): over the
//! grown DB, for every open row, are all legal replies stored rows with
//! proven outcomes? Each complete row's implied outcome + bound is
//! computed, iterated to fixpoint (a flip can imply its ancestors), and
//! every reported flip is recomputed by an independent recursive verifier
//! (fresh replay + movegen + completeness + bound arithmetic — the
//! "recomputed independently" contract; a flip claim without
//! AND-completeness is a defect and exits non-zero).
//!
//! Outcome polarity: DB outcomes are side-to-move, so a row at even ply
//! with `win` (or odd ply with `loss`) is a root-player win. Implied
//! bounds (plan8 §4.5): AND-loss (all replies root-player wins) → bound =
//! max(reply bound) + 1; OR-win (∃ root-player-loss reply) → bound =
//! min over the winning replies (child bound) + 1; the duals at odd plies.
//!
//! File-size justification: 12 KB — the fixpoint pass, the independent
//! recursive verifier, and the JSON emission share one indexing scheme and
//! one polarity convention; splitting would duplicate the reply-status
//! logic. The measurement runs it offline over the grown DB only.
//! Output: JSON at `--out` (default `data/proofdb/flip.json`, plan14;
//! `flips` + `root` + counts); exit 1 on any unverified flip, 0 otherwise.
//! This tool materializes nothing — flips stay derived (decision 10), the
//! DB ships the raw tree only.

use std::collections::HashMap;
use std::path::PathBuf;

use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::{Outcome, Position};

mod proofdb;

use proofdb::db::load_db_rows;
use proofdb::{DEFAULT_DB, DEFAULT_FLIP_OUT, DEFAULT_MANIFEST, create_parent_dir, read_manifest};

fn fail(msg: &str) -> ! {
    eprintln!("proofdb_flip: {msg}");
    std::process::exit(1);
}

fn usage() -> ! {
    eprintln!(
        "usage: proofdb_flip [--db <grown.db>] [--manifest <manifest.json>] \
         [--out <flip_analysis.json>]  \
         (defaults: --db data/proofdb/proofdb.db \
         --manifest data/proofdb/shards/manifest.json --out data/proofdb/flip.json)"
    );
    std::process::exit(1);
}

/// One implied row: side-to-move outcome + bound (parity converted to
/// root-player polarity only at output).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Flip {
    /// `true` = a win for the row's side to move.
    stm_win: bool,
    bound: u32,
}

impl Flip {
    /// Root-player polarity for the output (parity conversion).
    fn root_label(&self, ply: usize) -> &'static str {
        let root_win = self.stm_win == ply.is_multiple_of(2);
        if root_win { "root-win" } else { "root-loss" }
    }
}

struct Analysis<'a> {
    rows: &'a [proofdb::db::DbRow],
    by_path: HashMap<String, usize>,
    /// Current fixpoint state (path → implied).
    implied: HashMap<String, Flip>,
    /// Flip discovery order: (path, ply, fixpoint round).
    round: Vec<(String, usize, u32)>,
}

impl<'a> Analysis<'a> {
    /// Resolved status of a reply path: proven (DB) or implied (fixpoint),
    /// in side-to-move terms of the reply's own position.
    fn status(&self, path: &str, idx: usize) -> Option<Flip> {
        let row = &self.rows[idx];
        if let Some(o) = row.outcome {
            return Some(Flip {
                stm_win: o == Outcome::Win,
                bound: row.depth_bound.unwrap_or(0),
            });
        }
        self.implied.get(path).copied()
    }

    /// One fixpoint pass over the row tree (DFS with replay); returns the
    /// number of NEW flips. `round` = current iteration number.
    fn pass(&mut self, round: u32) -> usize {
        let mut new = 0;
        let mut stack: Vec<(usize, Position)> = Vec::new();
        let root = match self.rows.iter().position(|r| r.ply == 0) {
            Some(r) => r,
            None => fail("DB has no root row"),
        };
        let pos = Position::from_fen(Position::STARTPOS_FEN).unwrap();
        stack.push((root, pos));
        while let Some((idx, pos)) = stack.pop() {
            let row = &self.rows[idx];
            let path = row.path.join(" ");
            if row.outcome.is_none() && !self.implied.contains_key(&path) {
                // Classify this open row from its replies' resolved status.
                let mut replies: Vec<Option<Flip>> = Vec::new();
                let mut any = false;
                for mv in pos.legal_moves_vec() {
                    any = true;
                    let uci = move_to_uci(mv);
                    let mut child_path = row.path.clone();
                    child_path.push(uci.clone());
                    let key = child_path.join(" ");
                    let st = self.by_path.get(&key).and_then(|&i| self.status(&key, i));
                    replies.push(st);
                }
                // Completion semantics (side-to-move): the row is a win
                // for its mover iff some reply position is a loss for the
                // reply's side to move (bound = that reply + 1, min over
                // the winners); it is a loss iff all replies are resolved
                // and all win for their side to move (bound = max + 1 —
                // the defender's longest resistance). A row with an
                // unresolved reply stays open (AND-completeness; the
                // verifier rejects anything else).
                let flip = if !any {
                    None // rule-terminal draw: undecided in this model
                } else if let Some(b) = replies
                    .iter()
                    .filter_map(|r| r.as_ref())
                    .filter(|f| !f.stm_win)
                    .map(|f| f.bound)
                    .reduce(u32::min)
                {
                    Some(Flip {
                        stm_win: true,
                        bound: b.saturating_add(1),
                    })
                } else if replies.iter().all(|r| r.is_some()) {
                    let max = replies
                        .iter()
                        .filter_map(|r| r.as_ref())
                        .map(|f| f.bound)
                        .max()
                        .unwrap_or(0);
                    Some(Flip {
                        stm_win: false,
                        bound: max.saturating_add(1),
                    })
                } else {
                    None
                };
                if let Some(f) = flip {
                    let ply = row.ply;
                    self.round.push((path.clone(), ply, round));
                    self.implied.insert(path, f);
                    new += 1;
                }
            }
            for mv in pos.legal_moves_vec() {
                let uci = move_to_uci(mv);
                let mut child_path = row.path.clone();
                child_path.push(uci.clone());
                if let Some(&i) = self.by_path.get(&child_path.join(" ")) {
                    let mut child_pos = pos.clone();
                    child_pos.do_move(mv);
                    stack.push((i, child_pos));
                }
            }
        }
        new
    }

    /// Independent verifier: recursive, replay-from-scratch per flip, no
    /// shared state with the fixpoint pass.
    fn verify(&self, path: &str, want: Flip, depth: usize) -> Result<(), String> {
        if depth > 200 {
            return Err("verification recursion limit".to_string());
        }
        let idx = *self
            .by_path
            .get(path)
            .ok_or_else(|| format!("flip path {path:?} not a DB row"))?;
        let row = &self.rows[idx];
        let pos = replay_fresh(path)?;
        if row.outcome.is_some() {
            return Err(format!("flip at proven row {path:?}"));
        }
        let mut min_losing = u32::MAX;
        let mut max_bound = 0u32;
        let mut n = 0;
        for mv in pos.legal_moves_vec() {
            n += 1;
            let uci = move_to_uci(mv);
            let mut child_path = row.path.clone();
            child_path.push(uci.clone());
            let key = child_path.join(" ");
            let child = self
                .by_path
                .get(&key)
                .ok_or_else(|| format!("flip {path:?}: reply {uci} has no DB row"))?;
            let st = self
                .status(&key, *child)
                .ok_or_else(|| format!("flip {path:?}: reply {uci} unresolved"))?;
            if self.rows[*child].outcome.is_none() {
                let imp = self.implied[&key];
                self.verify(&key, imp, depth + 1)?;
            }
            if !st.stm_win {
                min_losing = min_losing.min(st.bound);
            }
            max_bound = max_bound.max(st.bound);
        }
        if n == 0 {
            return Err(format!("flip {path:?}: no legal replies (terminal)"));
        }
        let expect = if min_losing < u32::MAX {
            Flip {
                stm_win: true,
                bound: min_losing.saturating_add(1),
            }
        } else {
            Flip {
                stm_win: false,
                bound: max_bound + 1,
            }
        };
        if expect != want {
            return Err(format!("flip {path:?}: want {want:?}, verified {expect:?}"));
        }
        Ok(())
    }
}

/// Replay `path` from the startpos (fresh, per flip).
fn replay_fresh(path: &str) -> Result<Position, String> {
    let mut pos = Position::from_fen(Position::STARTPOS_FEN).map_err(|e| e.to_string())?;
    if !path.is_empty() {
        for uci in path.split(' ') {
            let mv = uci_to_move(uci, &pos).ok_or_else(|| format!("illegal {uci:?}"))?;
            pos.do_move(mv);
        }
    }
    Ok(pos)
}

fn main() {
    let mut db = PathBuf::from(DEFAULT_DB);
    let mut manifest_path = PathBuf::from(DEFAULT_MANIFEST);
    let mut out = PathBuf::from(DEFAULT_FLIP_OUT);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--db" => db = it.next().unwrap_or_else(|| usage()).into(),
            "--manifest" => manifest_path = it.next().unwrap_or_else(|| usage()).into(),
            "--out" => out = it.next().unwrap_or_else(|| usage()).into(),
            _ => usage(),
        }
    }
    let manifest = read_manifest(&manifest_path).unwrap_or_else(|e| fail(&e));
    let content = load_db_rows(&db, &manifest.sha256_hex).unwrap_or_else(|e| fail(&e));
    let mut an = Analysis {
        rows: &content.rows,
        by_path: content
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.path.join(" "), i))
            .collect(),
        implied: HashMap::new(),
        round: Vec::new(),
    };
    // Fixpoint (a flip can imply its ancestors; open rows ≤ a few hundred,
    // so 500 passes are a hard ceiling with a defect guard).
    let mut rounds = 0u32;
    for round in 1..=500u32 {
        rounds = round;
        if an.pass(round) == 0 {
            break;
        }
        if round == 500 {
            fail("fixpoint did not converge in 500 passes");
        }
    }
    // Independent recomputation of every reported flip.
    let mut verified = 0usize;
    for (path, _, _) in &an.round {
        let want = an.implied[path];
        if let Err(e) = an.verify(path, want, 0) {
            fail(&format!("FLIP VERIFICATION FAILED: {e}"));
        }
        verified += 1;
    }
    let root_flip = an.implied.get("").map(|f| (f.root_label(0), f.bound));
    if let Some((label, b)) = root_flip {
        eprintln!("proofdb_flip: ROOT implied {label} (bound {b})");
    }
    let json = serde_json::json!({
        "open_rows": content.rows.iter().filter(|r| r.outcome.is_none()).count(),
        "flips": an.round.iter().map(|(p, ply, r)| {
            let f = an.implied[p];
            serde_json::json!({
                "path": p, "ply": ply,
                "implied": f.root_label(*ply),
                "side_to_move": if f.stm_win { "win" } else { "loss" },
                "bound": f.bound, "fixpoint_round": r,
            })
        }).collect::<Vec<_>>(),
        "flips_total": an.round.len(),
        "flips_verified_independently": verified,
        "root_implied": root_flip.map(|(l, _)| l),
        "root_bound": root_flip.map(|(_, b)| b),
    });
    create_parent_dir(&out).unwrap_or_else(|e| fail(&e));
    std::fs::write(&out, serde_json::to_string_pretty(&json).unwrap() + "\n")
        .unwrap_or_else(|e| fail(&format!("cannot write {}: {e}", out.display())));
    println!(
        "flip_analysis: open_rows {} flips {} verified {} root {:?} (fixpoint {} rounds)",
        json["open_rows"],
        an.round.len(),
        verified,
        json["root_implied"],
        rounds
    );
}
