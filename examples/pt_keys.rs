//! Campaign measurement helper (`solve` initiative plan2): dump the Zobrist
//! key of every proof-tree node by replaying the tree from its root FEN.
//!
//! The `proof_tree.bin` dump format does not store node hashes (loaded trees
//! carry `hash == 0`), but the M1 secondary metric needs the node-key set of
//! the committed `reconstructed_d4d5_p30.bin` / `_p32.bin` trees to intersect
//! with TT-snapshot solved sections, and the SSFP store-merge tool needs the
//! node structure to derive Win best moves. This tool replays the tree
//! exactly like the offline validator (`proof_tree::validate`) —
//! `Position::do_move` per node in DFS pre-order — and prints one record per
//! node.
//!
//! This is measurement/campaign tooling, not part of the product solver
//! surface.
//!
//! Usage: `pt_keys <proof_tree.bin>`; stdout is one whitespace-separated
//! record per node in DFS pre-order:
//! `id parent_id key outcome depth mv_bits`
//! with `id` = 0 for the root, `parent_id` = -1 for the root, `outcome` ∈
//! {win, loss, draw}, and `mv_bits` = the 16-bit move code (0 for the root).

use atomic_solver::notation::move_to_bits;
use atomic_solver::position::{Outcome, Position};
use atomic_solver::proof_tree::ProofTree;
use std::fs;

fn main() {
    let mut path: Option<String> = None;
    for arg in std::env::args().skip(1) {
        if arg.starts_with('-') {
            panic!("unknown option '{arg}' (usage: pt_keys <proof_tree.bin>)");
        }
        if path.is_some() {
            panic!("unexpected extra argument '{arg}'");
        }
        path = Some(arg);
    }
    let Some(path) = path else {
        panic!("usage: pt_keys <proof_tree.bin>");
    };
    let data = fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let tree = ProofTree::from_bin(&mut &data[..]).unwrap_or_else(|e| panic!("parse {path}: {e}"));

    let mut pos = Position::from_fen(&tree.root_fen)
        .unwrap_or_else(|e| panic!("invalid root FEN {:?}: {e}", tree.root_fen));

    // Iterative DFS pre-order with explicit undo frames (mirrors the
    // validator's replay mechanics; deep trees must not overflow the stack).
    enum Frame {
        Enter { id: usize },
        Exit { mv: atomic_movegen::types::Move },
    }
    let mut stack: Vec<Frame> = vec![Frame::Enter { id: 0 }];
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Exit { mv } => {
                if mv != atomic_movegen::types::Move::NONE {
                    pos.undo_move(mv);
                }
            }
            Frame::Enter { id } => {
                let node = &tree.nodes[id];
                let outcome = node.outcome.unwrap_or(Outcome::Draw).as_str();
                // `node.parent` stores raw_dump_id + 1 (NonZeroU32); the root
                // has no parent and prints -1.
                let parent_id = if id == 0 {
                    -1
                } else {
                    node.parent.unwrap().get() as i64 - 1
                };
                println!(
                    "{} {parent_id} {} {outcome} {} {}",
                    id,
                    pos.hash(),
                    node.depth,
                    move_to_bits(node.mv)
                );
                stack.push(Frame::Exit { mv: node.mv });
                if node.mv != atomic_movegen::types::Move::NONE {
                    pos.do_move(node.mv);
                }
                for child in tree.children(id).collect::<Vec<_>>().into_iter().rev() {
                    stack.push(Frame::Enter { id: child });
                }
            }
        }
    }
}
