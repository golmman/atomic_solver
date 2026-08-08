//! Compact binary serialization for the in-memory proof tree.
//!
//! This file is larger than 10 KiB because the bit-packing routines for moves,
//! node adjacency, the compact binary format, and the round-trip tests all share
//! the same encoding constants.
//!
//! The binary dump is driver-free and stores only parent ids and 16-bit move
//! codes.  External loaders derive `outcome`, `depth`, `terminal`, and the
//! UCI move string from the adjacency list and the root outcome stored in the
//! header.

use std::io::{self, Read, Write};

use atomic_movegen::types::{Move, MoveType, PROMOTION_PIECES, Square};

use super::{ProofNode, ProofTree};
use crate::notation::move_to_uci;
use crate::position::Outcome;

const MAGIC: &[u8; 8] = b"ATOMTREE";
const VERSION: u8 = 1;
const ROOT_PARENT: u32 = u32::MAX;

/// Encode an `atomic_movegen` `Move` into a 16-bit code using only the public
/// API.
///
/// The bit layout matches `Move`'s documented encoding:
/// - bits 0-5: `to_sq`
/// - bits 6-11: `from_sq`
/// - bits 12-13: move type
/// - bits 14-15: promotion piece index
#[must_use]
pub fn move_to_bits(mv: Move) -> u16 {
    let to = (mv.to_sq() as u16) & 0x3f;
    let from = ((mv.from_sq() as u16) & 0x3f) << 6;
    let type_bits = match mv.move_type() {
        MoveType::Normal => 0u16,
        MoveType::Promotion => 1u16 << 12,
        MoveType::EnPassant => 2u16 << 12,
        MoveType::Castling => 3u16 << 12,
        _ => unreachable!(),
    };
    let promotion_bits = if mv.move_type() == MoveType::Promotion {
        let idx = PROMOTION_PIECES
            .iter()
            .position(|&pt| pt == mv.promotion_type())
            .unwrap_or(0) as u16;
        idx << 14
    } else {
        0u16
    };
    from | to | type_bits | promotion_bits
}

/// Decode a 16-bit move code back into a `Move` using only the public API.
///
/// Returns `None` for codes whose promotion index is out of range.
pub fn bits_to_move(code: u16) -> Option<Move> {
    let to = Square::from_u8((code & 0x3f) as u8);
    let from = Square::from_u8(((code >> 6) & 0x3f) as u8);
    let move_type_bits = (code >> 12) & 0x3;
    let promotion_idx = ((code >> 14) & 0x3) as usize;

    match move_type_bits {
        0 => Some(Move::make_move(from, to)),
        1 => {
            let pt = *PROMOTION_PIECES.get(promotion_idx)?;
            Some(Move::make_promotion(from, to, pt))
        }
        2 => Some(Move::make_enpassant(from, to)),
        3 => Some(Move::make_castling(from, to)),
        _ => unreachable!(),
    }
}

fn outcome_to_u8(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::Draw => 0,
        Outcome::Win => 1,
        Outcome::Loss => 2,
    }
}

fn outcome_from_u8(value: u8) -> Option<Outcome> {
    match value {
        0 => Some(Outcome::Draw),
        1 => Some(Outcome::Win),
        2 => Some(Outcome::Loss),
        _ => None,
    }
}

/// Write `tree` to `writer` in the compact `proof_tree.bin` format.
pub fn write_proof_tree<W: Write>(tree: &ProofTree, writer: &mut W) -> io::Result<()> {
    writer.write_all(MAGIC)?;
    writer.write_all(&[VERSION])?;
    writeln!(writer, "{}", tree.root_fen)?;

    let root = &tree.nodes[0];
    writer.write_all(&[outcome_to_u8(root.outcome)])?;
    writer.write_all(&root.depth.to_le_bytes())?;

    for node in &tree.nodes {
        let parent_id = node.parent.map_or(ROOT_PARENT, |p| p as u32);
        writer.write_all(&parent_id.to_le_bytes())?;
        writer.write_all(&move_to_bits(node.mv).to_le_bytes())?;
    }

    Ok(())
}

/// Read a `ProofTree` from `reader` in the compact `proof_tree.bin` format.
pub fn read_proof_tree<R: Read>(reader: &mut R) -> io::Result<ProofTree> {
    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bad magic: expected ATOMTREE",
        ));
    }

    let mut version = [0u8; 1];
    reader.read_exact(&mut version)?;
    if version[0] != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported proof-tree version {}", version[0]),
        ));
    }

    let mut fen = String::new();
    loop {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte)?;
        if byte[0] == b'\n' {
            break;
        }
        fen.push(byte[0] as char);
    }

    let mut outcome_buf = [0u8; 1];
    reader.read_exact(&mut outcome_buf)?;
    let root_outcome = outcome_from_u8(outcome_buf[0])
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid root_outcome"))?;

    let mut depth_buf = [0u8; 4];
    reader.read_exact(&mut depth_buf)?;
    let root_depth = u32::from_le_bytes(depth_buf);

    let mut payload = Vec::new();
    reader.read_to_end(&mut payload)?;
    if payload.len() % 6 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated node records",
        ));
    }

    let node_count = payload.len() / 6;
    if node_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing root node",
        ));
    }

    let mut nodes: Vec<ProofNode> = Vec::with_capacity(node_count);
    let mut parents: Vec<usize> = Vec::with_capacity(node_count);

    for i in 0..node_count {
        let off = i * 6;
        let parent_id = u32::from_le_bytes([
            payload[off],
            payload[off + 1],
            payload[off + 2],
            payload[off + 3],
        ]);
        let move_code = u16::from_le_bytes([payload[off + 4], payload[off + 5]]);

        if i == 0 && (parent_id != ROOT_PARENT || move_code != 0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "root record must have parent_id 0xFFFFFFFF and move_code 0",
            ));
        }
        if i != 0 && parent_id as usize >= i {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "parent_id must be smaller than child id",
            ));
        }

        let parent = if i == 0 {
            None
        } else {
            Some(parent_id as usize)
        };
        let mv = bits_to_move(move_code)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid move_code"))?;

        nodes.push(ProofNode {
            parent,
            mv,
            hash: 0,
            outcome: Outcome::Draw,
            depth: 0,
            children: Vec::new(),
        });
        parents.push(parent_id as usize);
    }

    // Reconstruct children from parent links.
    for i in 0..node_count {
        if let Some(p) = nodes[i].parent {
            nodes[p].children.push(i);
        }
    }

    // Derive per-node outcomes from the root outcome and graph depth parity.
    let mut graph_depths = vec![0u32; node_count];
    for i in 1..node_count {
        graph_depths[i] = graph_depths[parents[i]] + 1;
    }

    for i in 0..node_count {
        nodes[i].outcome = if root_outcome == Outcome::Draw {
            Outcome::Draw
        } else if graph_depths[i] % 2 == 0 {
            root_outcome
        } else {
            root_outcome.flip()
        };
    }

    // Derive proven depths by a post-order traversal.  Records are written in
    // creation order, so every parent precedes its children; iterating in
    // reverse visits children before parents.
    for i in (0..node_count).rev() {
        if nodes[i].children.is_empty() {
            nodes[i].depth = 0;
            continue;
        }

        let child_depths: Vec<u32> = nodes[i].children.iter().map(|&c| nodes[c].depth).collect();

        nodes[i].depth = match nodes[i].outcome {
            Outcome::Win => 1 + child_depths.iter().min().copied().unwrap_or(0),
            Outcome::Loss => 1 + child_depths.iter().max().copied().unwrap_or(0),
            Outcome::Draw => 0,
        };
    }

    if nodes[0].depth != root_depth {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "root depth mismatch: header {} != derived {}",
                root_depth, nodes[0].depth
            ),
        ));
    }

    // Rebuild the path index so that `add_node` and `path_for` work on a loaded
    // tree.  Because the dump is a tree (each node has exactly one parent),
    // paths are unique.
    let mut index = std::collections::HashMap::with_capacity(node_count);
    let mut paths = Vec::with_capacity(node_count);
    paths.push("root".to_string());
    index.insert(paths[0].clone(), 0);

    for (i, node) in nodes.iter().enumerate().skip(1) {
        let parent = node.parent.expect("non-root node has a parent");
        let uci = move_to_uci(node.mv);
        let path = format!("{}.{}", paths[parent], uci);
        paths.push(path.clone());
        index.insert(path, i);
    }

    Ok(ProofTree {
        root_fen: fen,
        nodes,
        index,
    })
}

#[cfg(test)]
mod tests {
    use atomic_movegen::types::{Move, PieceType, Square};

    use super::{bits_to_move, move_to_bits};

    #[test]
    fn move_to_bits_round_trips_normal() {
        let mv = Move::make_move(Square::E2, Square::E4);
        assert_eq!(bits_to_move(move_to_bits(mv)), Some(mv));
    }

    #[test]
    fn move_to_bits_round_trips_promotions() {
        for pt in [
            PieceType::Queen,
            PieceType::Rook,
            PieceType::Bishop,
            PieceType::Knight,
        ] {
            let mv = Move::make_promotion(Square::E7, Square::E8, pt);
            assert_eq!(
                bits_to_move(move_to_bits(mv)),
                Some(mv),
                "promotion to {pt:?}"
            );
        }
    }

    #[test]
    fn move_to_bits_round_trips_en_passant() {
        let mv = Move::make_enpassant(Square::C5, Square::D6);
        assert_eq!(bits_to_move(move_to_bits(mv)), Some(mv));
    }

    #[test]
    fn move_to_bits_round_trips_castling() {
        let mv = Move::make_castling(Square::E1, Square::H1);
        assert_eq!(bits_to_move(move_to_bits(mv)), Some(mv));
    }

    #[test]
    fn move_to_bits_none_is_zero() {
        assert_eq!(move_to_bits(Move::NONE), 0);
        assert_eq!(bits_to_move(0), Some(Move::NONE));
    }

    #[test]
    fn move_to_bits_matches_worked_example() {
        let e2e4 = Move::make_move(Square::E2, Square::E4);
        assert_eq!(move_to_bits(e2e4), 796);

        let e7e5 = Move::make_move(Square::E7, Square::E5);
        assert_eq!(move_to_bits(e7e5), 3364);

        let e7e8q = Move::make_promotion(Square::E7, Square::E8, PieceType::Queen);
        assert_eq!(move_to_bits(e7e8q), 7484);
    }
}
