//! Master-side merge verification for the campaign prototype
//! (`docs/plans/solve/campaign_architecture.md` §3, per-fact on receipt).
//!
//! A worker-decisive result is trusted only after this pass: the job path is
//! replayed from the campaign root in global context, the finalized subtree
//! is checked structurally in the master's own replay (legality, terminal
//! agreement, Win = one Loss child, Loss = all replies Win), and every fact
//! is re-emitted as a global-path `NodeProven` with the recomputed Zobrist
//! hash. Any failure rejects the whole result — never patched.
//!
//! This is the campaign-side analog of `proof_tree::validate`'s replay
//! discipline, restricted to one incoming subtree; the global aggregation is
//! validated again at artifact finalization.

use atomic_movegen::board::StateInfo;
use atomic_movegen::types::{Move, MoveList};
use atomic_solver::notation::{move_to_uci, uci_to_move};
use atomic_solver::position::{Outcome, Position};

use super::EventRec;

/// Re-emit callback: (global path, recomputed Zobrist hash, outcome, depth).
type EmitFn<'a> = dyn FnMut(&[Move], u64, Outcome, u32) + 'a;

struct TrieNode {
    outcome: Option<Outcome>,
    depth: u32,
    /// Children keyed by the UCI string exactly as exported; resolved
    /// against the replayed position's legal moves during the DFS, so
    /// en-passant / castling encodings cannot diverge from movegen.
    children: Vec<(String, usize)>,
}

/// Verify and merge a worker-decisive result.
///
/// `emit` receives one call per verified fact: (global path, hash, outcome,
/// depth). Returns the job root's proven `(outcome, depth)`.
pub fn verify_and_merge(
    root_fen: &str,
    job_path_uci: &[String],
    events: &[EventRec],
    emit: &mut EmitFn<'_>,
) -> Result<(Outcome, u32), String> {
    let mut pos = Position::from_fen(root_fen).map_err(|e| format!("root FEN: {e}"))?;
    let mut job_path: Vec<Move> = Vec::new();
    for uci in job_path_uci {
        let mv = uci_to_move(uci, &pos)
            .ok_or_else(|| format!("job path move {uci} illegal during master replay"))?;
        pos.do_move(mv);
        job_path.push(mv);
    }

    let trie = build_trie(events)?;
    if trie[0].outcome.is_none() {
        return Err("decisive result without a root event".to_string());
    }
    let mut path: Vec<Move> = Vec::new();
    visit_trie(&trie, 0, &mut pos, &job_path, &mut path, emit)
}

fn build_trie(events: &[EventRec]) -> Result<Vec<TrieNode>, String> {
    let mut trie: Vec<TrieNode> = vec![TrieNode {
        outcome: None,
        depth: 0,
        children: Vec::new(),
    }];
    for ev in events {
        let outcome: Outcome = ev
            .outcome
            .parse()
            .map_err(|e: String| format!("bad outcome in event: {e}"))?;
        let mut idx = 0usize;
        for (i, uci) in ev.path.iter().enumerate() {
            idx = match trie[idx].children.iter().find(|(u, _)| u == uci) {
                Some(&(_, c)) => c,
                None => {
                    let c = trie.len();
                    trie.push(TrieNode {
                        outcome: None,
                        depth: 0,
                        children: Vec::new(),
                    });
                    trie[idx].children.push((uci.clone(), c));
                    c
                }
            };
            if i + 1 < ev.path.len() && trie[idx].outcome.is_some() && trie[idx].depth == 0 {
                return Err("event path descends below a terminal node".to_string());
            }
        }
        let node = &mut trie[idx];
        match node.outcome {
            None => {
                node.outcome = Some(outcome);
                node.depth = ev.depth;
            }
            Some(prev) if prev == outcome => node.depth = node.depth.min(ev.depth),
            Some(prev) => {
                return Err(format!(
                    "contradictory outcomes for one path: {prev:?} vs {outcome:?}"
                ));
            }
        }
    }
    Ok(trie)
}

/// DFS-verify the trie over a real replayed `Position`; emits every fact in
/// global context. Returns the job root's `(outcome, depth)`.
fn visit_trie(
    trie: &[TrieNode],
    idx: usize,
    pos: &mut Position,
    job_path: &[Move],
    path: &mut Vec<Move>,
    emit: &mut EmitFn<'_>,
) -> Result<(Outcome, u32), String> {
    let node = &trie[idx];
    let outcome = node
        .outcome
        .ok_or_else(|| "event trie has an unrealized interior node".to_string())?;

    let mut moves = MoveList::new();
    let mut state = StateInfo::new();
    pos.legal_moves_with_state(&mut moves, &mut state);
    let static_outcome = pos.outcome_from_state(&state, &moves);

    match static_outcome {
        Some(so) => {
            if so != outcome {
                return Err(format!(
                    "terminal fact mismatch: worker says {outcome:?}, replay says {so:?}"
                ));
            }
            if node.depth != 0 {
                return Err("terminal fact with depth > 0".to_string());
            }
        }
        None => {
            if outcome == Outcome::Draw {
                return Err("draw fact inside a decisive subtree".to_string());
            }
            let legal: Vec<Move> = moves.as_slice().to_vec();
            if outcome == Outcome::Win && node.children.len() != 1 {
                return Err(format!(
                    "non-terminal Win node at {} has {} children",
                    atomic_solver::notation::moves_to_uci_path(path),
                    node.children.len()
                ));
            }
            if outcome == Outcome::Loss {
                let legal_uci: Vec<String> = legal.iter().map(|m| move_to_uci(*m)).collect();
                for reply in &legal_uci {
                    if !node.children.iter().any(|(u, _)| u == reply) {
                        return Err(format!("Loss node missing reply {reply}"));
                    }
                }
            }
            for (uci, child) in &node.children {
                let child = *child;
                let Some(mv) = uci_to_move(uci.as_str(), pos) else {
                    return Err(format!(
                        "child move {uci} at {} not legal in replay",
                        atomic_solver::notation::moves_to_uci_path(path)
                    ));
                };
                let _ = &legal;
                let expected = match outcome {
                    Outcome::Win => Outcome::Loss,
                    _ => Outcome::Win,
                };
                if trie[child].outcome != Some(expected) {
                    return Err(format!(
                        "child of {outcome:?} node has outcome {:?}",
                        trie[child].outcome
                    ));
                }
                pos.do_move(mv);
                path.push(mv);
                visit_trie(trie, child, pos, job_path, path, emit)?;
                path.pop();
                pos.undo_move(mv);
            }
        }
    }

    let global: Vec<Move> = job_path
        .iter()
        .copied()
        .chain(path.iter().copied())
        .collect();
    emit(&global, pos.hash(), outcome, node.depth);
    Ok((outcome, node.depth))
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomic_solver::proof_event::{NodeProven, ProofEvent};
    use std::sync::mpsc;

    const WIN_IN_1_FEN: &str = "k7/p7/8/8/8/8/1P4K1/R7 w - - 0 1";

    fn events(pairs: &[(&[&str], &str, u32)]) -> Vec<EventRec> {
        pairs
            .iter()
            .map(|(path, outcome, depth)| EventRec {
                path: path.iter().map(|s| s.to_string()).collect(),
                outcome: outcome.to_string(),
                depth: *depth,
            })
            .collect()
    }

    fn verify(root: &str, job: &[&str], evs: &[EventRec]) -> Result<Vec<NodeProven>, String> {
        let job_path: Vec<String> = job.iter().map(|s| s.to_string()).collect();
        let (tx, rx) = mpsc::channel::<NodeProven>();
        let result = {
            let mut emit = move |global: &[Move], hash: u64, outcome: Outcome, depth: u32| {
                let _ = tx.send(NodeProven::new(global.to_vec(), hash, outcome, depth));
            };
            let r = verify_and_merge(root, &job_path, evs, &mut emit);
            drop(emit); // drops tx: the recv loop below terminates
            r
        };
        let mut out = Vec::new();
        while let Ok(ev) = rx.recv() {
            out.push(ev);
        }
        result.map(|_| out)
    }

    #[test]
    fn accepts_sound_win_in_1_subtree() {
        // After Ra7xP the exploding rook destroys the black king's last
        // commoner: the position is terminal Loss for black (side to move),
        // i.e. a depth-0 Loss fact under a Win root.
        let evs = events(&[(&["a1a7"], "loss", 0), (&[], "win", 1)]);
        let facts = verify(WIN_IN_1_FEN, &[], &evs).expect("verification must pass");
        assert_eq!(facts.len(), 2);
        assert!(
            facts
                .iter()
                .any(|f| f.path.is_empty() && f.outcome == Outcome::Win)
        );
        let pos = Position::from_fen(WIN_IN_1_FEN).unwrap();
        let root_key = facts
            .iter()
            .find(|f| f.path.is_empty())
            .map(|f| f.hash)
            .unwrap();
        assert_eq!(root_key, pos.hash());
    }

    #[test]
    fn rejects_win_node_without_loss_child() {
        // a1a2 is quiet; claiming a Win there without any child is rejected.
        let evs = events(&[(&["a1a2"], "loss", 1), (&[], "win", 2)]);
        let err = verify(WIN_IN_1_FEN, &[], &evs).unwrap_err();
        assert!(err.contains("Loss node missing reply"), "{err}");
    }

    #[test]
    fn rejects_terminal_depth() {
        // a1a7 is statically terminal: a depth-2 claim there is rejected.
        let evs = events(&[(&["a1a7"], "loss", 2), (&[], "win", 3)]);
        let err = verify(WIN_IN_1_FEN, &[], &evs).unwrap_err();
        assert!(err.contains("depth"), "{err}");
    }

    #[test]
    fn rejects_terminal_mismatch() {
        // a1a7 is statically Loss (for black); claiming Win fails.
        let evs = events(&[(&["a1a7"], "win", 0), (&[], "win", 1)]);
        let err = verify(WIN_IN_1_FEN, &[], &evs).unwrap_err();
        assert!(
            err.contains("terminal fact mismatch") || err.contains("child of"),
            "{err}"
        );
    }

    #[test]
    fn rejects_missing_root_event() {
        let evs = events(&[(&["a1a7"], "win", 0)]);
        let err = verify(WIN_IN_1_FEN, &[], &evs).unwrap_err();
        assert!(err.contains("without a root event"), "{err}");
    }

    #[test]
    fn rejects_illegal_job_path() {
        let evs = events(&[(&[], "win", 1)]);
        let err = verify(WIN_IN_1_FEN, &["a7a6"], &evs).unwrap_err();
        assert!(err.contains("illegal"), "{err}");
    }

    #[test]
    fn rejects_contradictory_outcomes() {
        let evs = events(&[(&[], "win", 1), (&[], "loss", 1)]);
        let err = verify(WIN_IN_1_FEN, &[], &evs).unwrap_err();
        assert!(err.contains("contradictory"), "{err}");
    }
}
