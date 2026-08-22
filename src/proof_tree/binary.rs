//! Compact binary serialization for the in-memory proof tree.
//!
//! This file is larger than 10 KiB because the bit-packing routines for moves,
//! node adjacency, the compact binary format, and the round-trip tests all share
//! the same encoding constants.
//!
//! The binary dump is driver-free and stores only parent ids, 16-bit move
//! codes, and the recorded `work` value.  External loaders derive `outcome`,
//! `depth`, `terminal`, and the UCI move string from the adjacency list and the
//! root outcome stored in the header.
//!
//! Version 2 records carry an 8-byte `work` field per node.  Version 1 records
//! (6 bytes) are still read, with `work == 0` for every node.

use std::io::{self, Read, Write};
use std::num::NonZeroU32;

use atomic_movegen::types::{Move, MoveType, PROMOTION_PIECES, Square};

use super::{ProofNode, ProofTree};
use crate::position::Outcome;

const MAGIC: &[u8; 8] = b"ATOMTREE";
const VERSION: u8 = 2;
const ROOT_PARENT: u32 = u32::MAX;
const RECORD_SIZE_V2: usize = 4 + 2 + 8;
const RECORD_SIZE_V1: usize = 4 + 2;

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
///
/// The tree is expected to be finalized so every node carries a real outcome.
pub fn write_proof_tree<W: Write>(tree: &ProofTree, writer: &mut W) -> io::Result<()> {
    writer.write_all(MAGIC)?;
    writer.write_all(&[VERSION])?;
    writeln!(writer, "{}", tree.root_fen)?;

    let root = &tree.nodes[0];
    let root_outcome = root.outcome.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "cannot dump a proof tree with an unrealized root",
        )
    })?;
    writer.write_all(&[outcome_to_u8(root_outcome)])?;
    writer.write_all(&root.depth.to_le_bytes())?;

    for node in &tree.nodes {
        if node.outcome.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cannot dump a proof tree containing dummy nodes",
            ));
        }
        let parent_id = node
            .parent
            .map_or(ROOT_PARENT, |p| p.get().saturating_sub(1));
        writer.write_all(&parent_id.to_le_bytes())?;
        writer.write_all(&move_to_bits(node.mv).to_le_bytes())?;
        writer.write_all(&node.work.to_le_bytes())?;
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
    let record_size = match version[0] {
        1 => RECORD_SIZE_V1,
        2 => RECORD_SIZE_V2,
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported proof-tree version {}", other),
            ));
        }
    };

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
    if payload.len() % record_size != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated node records",
        ));
    }

    let node_count = payload.len() / record_size;
    if node_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing root node",
        ));
    }
    if node_count > u32::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "proof tree exceeds u32 node-id limit",
        ));
    }

    let mut nodes: Vec<ProofNode> = Vec::with_capacity(node_count);
    let mut parents: Vec<usize> = Vec::with_capacity(node_count);

    for i in 0..node_count {
        let off = i * record_size;
        let parent_id = u32::from_le_bytes([
            payload[off],
            payload[off + 1],
            payload[off + 2],
            payload[off + 3],
        ]);
        let move_code = u16::from_le_bytes([payload[off + 4], payload[off + 5]]);
        let work = if record_size == RECORD_SIZE_V2 {
            u64::from_le_bytes([
                payload[off + 6],
                payload[off + 7],
                payload[off + 8],
                payload[off + 9],
                payload[off + 10],
                payload[off + 11],
                payload[off + 12],
                payload[off + 13],
            ])
        } else {
            0
        };

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
            NonZeroU32::new(parent_id + 1)
        };
        let mv = bits_to_move(move_code)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid move_code"))?;

        nodes.push(ProofNode {
            parent,
            first_child: None,
            next_sibling: None,
            mv,
            hash: 0,
            outcome: Some(Outcome::Draw),
            depth: 0,
            work,
        });
        parents.push(parent_id as usize);
    }

    // Reconstruct the intrusive first-child / next-sibling list from parent links.
    for i in 1..node_count {
        let p = parents[i];
        nodes[i].next_sibling = nodes[p].first_child;
        nodes[p].first_child = NonZeroU32::new(i as u32);
    }

    // Derive per-node outcomes from the root outcome and graph depth parity.
    let mut graph_depths = vec![0u32; node_count];
    for i in 1..node_count {
        graph_depths[i] = graph_depths[parents[i]] + 1;
    }

    for i in 0..node_count {
        nodes[i].outcome = Some(if root_outcome == Outcome::Draw {
            Outcome::Draw
        } else if graph_depths[i] % 2 == 0 {
            root_outcome
        } else {
            root_outcome.flip()
        });
    }

    // Derive proven depths by a post-order traversal. Records are written in
    // creation order, so every parent precedes its children; iterating in
    // reverse visits children before parents.
    for i in (0..node_count).rev() {
        if nodes[i].first_child.is_none() {
            nodes[i].depth = 0;
            continue;
        }

        let child_depths: Vec<u32> = node_children(&nodes, i).map(|c| nodes[c].depth).collect();

        nodes[i].depth = match nodes[i].outcome {
            Some(Outcome::Win) => 1 + child_depths.iter().min().copied().unwrap_or(0),
            Some(Outcome::Loss) => 1 + child_depths.iter().max().copied().unwrap_or(0),
            _ => 0,
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

    Ok(ProofTree {
        root_fen: fen,
        nodes,
    })
}

fn node_children(nodes: &[ProofNode], node_id: usize) -> impl Iterator<Item = usize> + '_ {
    let mut next = nodes[node_id].first_child.map(|nz| nz.get() as usize);
    std::iter::from_fn(move || {
        let id = next?;
        next = nodes[id].next_sibling.map(|nz| nz.get() as usize);
        Some(id)
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

    #[test]
    fn reads_version_one_dump_with_zero_work() {
        use crate::position::Outcome;

        let mut buf = Vec::new();
        buf.extend(super::MAGIC);
        buf.push(1); // version 1
        buf.extend(b"x\n");
        buf.push(1); // root_outcome: Win
        buf.extend(2u32.to_le_bytes()); // root_depth
        // root record
        buf.extend(u32::MAX.to_le_bytes());
        buf.extend(0u16.to_le_bytes());
        // e2e4 under root
        buf.extend(0u32.to_le_bytes());
        buf.extend(796u16.to_le_bytes());
        // e7e5 under e2e4
        buf.extend(1u32.to_le_bytes());
        buf.extend(3364u16.to_le_bytes());

        let tree = super::read_proof_tree(&mut &buf[..]).unwrap();
        assert_eq!(tree.nodes.len(), 3);
        assert!(tree.nodes.iter().all(|n| n.work == 0));
        assert_eq!(tree.nodes[0].outcome, Some(Outcome::Win));
        assert_eq!(tree.nodes[1].outcome, Some(Outcome::Loss));
        assert_eq!(tree.nodes[1].depth, 1);
        assert_eq!(tree.nodes[2].depth, 0);
    }

    #[test]
    fn rejects_unknown_version() {
        let mut buf = Vec::new();
        buf.extend(super::MAGIC);
        buf.push(3);
        let err = super::read_proof_tree(&mut &buf[..]).expect_err("version 3 should be rejected");
        assert!(err.to_string().contains("unsupported proof-tree version 3"));
    }
}
