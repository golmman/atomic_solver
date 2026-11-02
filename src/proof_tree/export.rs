//! Export a finalized proof tree to provari's JSON-lines checker format.
//!
//! The export is deliberately derived, never authoritative: the proof tree
//! (and the validated DB built from it) stays the checkable artifact, and
//! provari is a second, independent rules implementation
//! (`shakmaty`-based) that accepts the attacker's proof.
//!
//! The format is provari's `JSONL` reader path (`src/check.rs`):
//!
//! - the first non-empty line is `{"root":"<FEN>"}` (and *only* that field:
//!   a `provari` field would route the file to the compact reader);
//! - every following line is `{"fen":"<FEN>","move":"<UCI>"}` for one
//!   attacker position: the move the proof plays there;
//! - the checker loads all lines first (keyed by `(place, halfmove_clock)`),
//!   then walks from the root, generating every defender reply itself.
//!
//! Only root-`Win` trees can be exported: the JSON-lines reader assumes the
//! side to move is the attacker. Root-`Loss` export is out of scope.
//!
//! # Soundness contract
//!
//! The exporter validates the tree with [`validate_proof_tree`] before it
//! writes anything, and replays the tree with [`Position`] while walking it.
//! An invalid tree, a non-`Win` root, or a malformed node returns an error
//! and leaves the writer untouched. The walk is an explicit DFS stack
//! (`Enter`/`Exit`), so deep proof trees cannot overflow the Rust call stack.

use std::fmt;
use std::io::{self, Write};

use atomic_movegen::types::Move;

use crate::notation::move_to_uci;
use crate::position::{Outcome, Position};

use super::{ProofTree, TreeDefect, validate_proof_tree};

/// Why a proof tree could not be exported to the JSON-lines format.
#[derive(Debug)]
pub enum ExportError {
    /// The replay-based validator rejected the tree.
    Invalid(Vec<TreeDefect>),
    /// The root position is not a `Win`, so it cannot be exported as an
    /// attacker proof.
    RootNotWin(Outcome),
    /// The tree holds a node shape the exporter cannot express (for example
    /// a non-terminal `Win` node without exactly one child).
    Malformed(String),
    /// Writing the output failed.
    Io(io::Error),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Invalid(defects) => {
                write!(f, "invalid proof tree: {} defect(s)", defects.len())?;
                if let Some(first) = defects.first() {
                    write!(f, "; first: {first}")?;
                }
                Ok(())
            }
            ExportError::RootNotWin(outcome) => write!(
                f,
                "root outcome is {outcome}, but only a root-Win tree can be exported"
            ),
            ExportError::Malformed(detail) => write!(f, "malformed proof tree: {detail}"),
            ExportError::Io(error) => write!(f, "cannot write the export: {error}"),
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExportError::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ExportError {
    fn from(error: io::Error) -> Self {
        ExportError::Io(error)
    }
}

/// Write `tree` to `writer` in provari's JSON-lines format.
///
/// Returns the number of attacker lines emitted (the root header is not
/// counted). The tree must be a validated root-`Win` proof; see the module
/// docs for the soundness contract.
pub fn write_provari_jsonl<W: Write>(
    tree: &ProofTree,
    writer: &mut W,
) -> Result<usize, ExportError> {
    validate_proof_tree(tree).map_err(ExportError::Invalid)?;

    let root = tree
        .nodes
        .first()
        .ok_or_else(|| ExportError::Malformed("empty tree".to_string()))?;
    match root.outcome {
        Some(Outcome::Win) => {}
        Some(outcome) => return Err(ExportError::RootNotWin(outcome)),
        None => return Err(ExportError::Malformed("unrealized root".to_string())),
    }

    let mut pos = Position::from_fen(&tree.root_fen)
        .map_err(|e| ExportError::Malformed(format!("root FEN failed to parse: {e}")))?;

    writer.write_all(b"{\"root\":")?;
    write_json_string(writer, &tree.root_fen)?;
    writer.write_all(b"}\n")?;

    enum Frame {
        /// Visit `id`, reached by playing `mv` (`Move::NONE` for the root).
        Enter { id: usize, mv: Move },
        /// Leave the child reached by `mv`: undo it.
        Exit { mv: Move },
    }

    let mut stack: Vec<Frame> = vec![Frame::Enter {
        id: 0,
        mv: Move::NONE,
    }];
    let mut lines = 0usize;

    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Enter { id, mv } => {
                if mv != Move::NONE {
                    pos.do_move(mv);
                    stack.push(Frame::Exit { mv });
                }

                let node = &tree.nodes[id];
                match node.outcome {
                    Some(Outcome::Win) => {
                        // A terminal Win node needs no line: the checker
                        // settles it from the position itself.
                        if node.depth == 0 {
                            continue;
                        }
                        let mut children = tree.children(id);
                        let child = children.next().ok_or_else(|| {
                            ExportError::Malformed(format!(
                                "non-terminal Win node {id} has no child"
                            ))
                        })?;
                        if children.next().is_some() {
                            return Err(ExportError::Malformed(format!(
                                "non-terminal Win node {id} has more than one child"
                            )));
                        }
                        let child_mv = tree.nodes[child].mv;
                        writer.write_all(b"{\"fen\":")?;
                        write_json_string(writer, &pos.fen())?;
                        writer.write_all(b",\"move\":")?;
                        write_json_string(writer, &move_to_uci(child_mv))?;
                        writer.write_all(b"}\n")?;
                        lines += 1;
                        stack.push(Frame::Enter {
                            id: child,
                            mv: child_mv,
                        });
                    }
                    Some(Outcome::Loss) => {
                        // The checker generates every defender reply itself,
                        // so a Loss node emits nothing; its children are the
                        // proof of each reply and are walked in order.
                        if node.depth == 0 {
                            continue;
                        }
                        let children: Vec<usize> = tree.children(id).collect();
                        for &c in children.iter().rev() {
                            stack.push(Frame::Enter {
                                id: c,
                                mv: tree.nodes[c].mv,
                            });
                        }
                    }
                    Some(Outcome::Draw) => {
                        return Err(ExportError::Malformed(format!(
                            "draw node {id} is unreachable in a validated tree"
                        )));
                    }
                    None => {
                        return Err(ExportError::Malformed(format!(
                            "unrealized node {id} is unreachable in a validated tree"
                        )));
                    }
                }
            }
            Frame::Exit { mv } => pos.undo_move(mv),
        }
    }

    Ok(lines)
}

/// Write `value` as a JSON string literal, escaping `"`, `\`, and control
/// characters. FEN and UCI strings are JSON-safe by construction, so the
/// escaping is defensive (and keeps the writer honest for future callers).
fn write_json_string<W: Write>(writer: &mut W, value: &str) -> io::Result<()> {
    writer.write_all(b"\"")?;
    for ch in value.chars() {
        match ch {
            '"' => writer.write_all(b"\\\"")?,
            '\\' => writer.write_all(b"\\\\")?,
            '\n' => writer.write_all(b"\\n")?,
            '\r' => writer.write_all(b"\\r")?,
            '\t' => writer.write_all(b"\\t")?,
            c if (c as u32) < 0x20 => write!(writer, "\\u{:04x}", c as u32)?,
            c => write!(writer, "{c}")?,
        }
    }
    writer.write_all(b"\"")
}

#[cfg(test)]
mod tests;
