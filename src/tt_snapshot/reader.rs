//! Strict reader for the TT snapshot format (`v1`).
//!
//! See the parent module for the format specification and section policy.

use std::io::{self, Read};

use crate::proof_tree::binary::outcome_from_u8;

use super::{
    MAGIC, SnapshotHeader, SolvedRecord, UnsolvedRecord, VERSION, invalid_data, move_from_u16,
};

/// Read a snapshot into its header and record vectors.
///
/// Strict: rejects wrong magic, unknown versions, nonzero flags, invalid
/// outcome or move encodings, truncated streams, and trailing bytes beyond
/// the last record.
pub fn read_tt_snapshot<R: Read>(
    r: &mut R,
) -> io::Result<(SnapshotHeader, Vec<SolvedRecord>, Vec<UnsolvedRecord>)> {
    let mut magic = [0u8; 8];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(invalid_data("bad magic: expected ATOMTTSN"));
    }

    let mut version = [0u8; 1];
    r.read_exact(&mut version)?;
    if version[0] != VERSION {
        return Err(invalid_data(&format!(
            "unsupported tt-snapshot version {}",
            version[0]
        )));
    }

    let mut flags = [0u8; 1];
    r.read_exact(&mut flags)?;
    if flags[0] != 0 {
        return Err(invalid_data(&format!(
            "unsupported tt-snapshot flags {:#x}",
            flags[0]
        )));
    }

    let mut fen_bytes = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        r.read_exact(&mut byte)?;
        if byte[0] == b'\n' {
            break;
        }
        fen_bytes.push(byte[0]);
    }
    let root_fen =
        String::from_utf8(fen_bytes).map_err(|_| invalid_data("root FEN is not valid UTF-8"))?;

    let mut buf8 = [0u8; 8];
    let mut buf4 = [0u8; 4];
    r.read_exact(&mut buf4)?;
    let tt_size_mb = u32::from_le_bytes(buf4);
    r.read_exact(&mut buf4)?;
    let generation = u32::from_le_bytes(buf4);
    r.read_exact(&mut buf8)?;
    let solved_count = u64::from_le_bytes(buf8);
    r.read_exact(&mut buf8)?;
    let unsolved_count = u64::from_le_bytes(buf8);

    let mut solved = Vec::new();
    let solved_cap: usize = solved_count
        .try_into()
        .map_err(|_| invalid_data("solved_count too large"))?;
    solved
        .try_reserve_exact(solved_cap)
        .map_err(|_| invalid_data("solved_count too large to buffer"))?;
    for _ in 0..solved_count {
        r.read_exact(&mut buf8)?;
        let key = u64::from_le_bytes(buf8);
        let mut one = [0u8; 1];
        r.read_exact(&mut one)?;
        let outcome =
            outcome_from_u8(one[0]).ok_or_else(|| invalid_data("invalid solved outcome byte"))?;
        r.read_exact(&mut buf4)?;
        let depth = u32::from_le_bytes(buf4);
        let mut two = [0u8; 2];
        r.read_exact(&mut two)?;
        let best_move = move_from_u16(u16::from_le_bytes(two))?;
        solved.push(SolvedRecord {
            key,
            outcome,
            depth,
            best_move,
        });
    }

    let mut unsolved = Vec::new();
    let unsolved_cap: usize = unsolved_count
        .try_into()
        .map_err(|_| invalid_data("unsolved_count too large"))?;
    unsolved
        .try_reserve_exact(unsolved_cap)
        .map_err(|_| invalid_data("unsolved_count too large to buffer"))?;
    for _ in 0..unsolved_count {
        r.read_exact(&mut buf8)?;
        let key = u64::from_le_bytes(buf8);
        r.read_exact(&mut buf8)?;
        let pn = u64::from_le_bytes(buf8);
        r.read_exact(&mut buf8)?;
        let dn = u64::from_le_bytes(buf8);
        r.read_exact(&mut buf4)?;
        let depth = u32::from_le_bytes(buf4);
        r.read_exact(&mut buf4)?;
        let remaining_depth = u32::from_le_bytes(buf4);
        let mut two = [0u8; 2];
        r.read_exact(&mut two)?;
        let best_move = move_from_u16(u16::from_le_bytes(two))?;
        r.read_exact(&mut buf8)?;
        let work = u64::from_le_bytes(buf8);
        unsolved.push(UnsolvedRecord {
            key,
            pn,
            dn,
            depth,
            remaining_depth,
            best_move,
            work,
        });
    }

    // No trailing bytes are allowed beyond the last record.
    let mut trailing = [0u8; 1];
    if r.read(&mut trailing)? != 0 {
        return Err(invalid_data("trailing bytes after last record"));
    }

    Ok((
        SnapshotHeader {
            root_fen,
            tt_size_mb,
            generation,
            solved_count,
            unsolved_count,
        },
        solved,
        unsolved,
    ))
}
