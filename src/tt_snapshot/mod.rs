//! Compact binary snapshot of the transposition table.
//!
//! The snapshot is the transfer artifact for the decoupled proof pipeline: a
//! separate reconstruction tool reads the root FEN plus this snapshot to
//! rebuild a proof tree without re-running the search. It is written once at
//! exit and is bounded by the TT RAM (worst case ≈ 0.75×, since records are
//! 15–42 bytes vs. the two-slot bucket occupancy of the live table).
//!
//! # Format (version 1)
//!
//! Little-endian throughout, fields packed with no padding. Mirrors the
//! `proof_tree::binary` conventions (8-byte magic, version byte,
//! newline-terminated FEN header line).
//!
//! ```text
//! offset  size  field
//! 0       8     magic  = "ATOMTTSN"
//! 8       1     version = 1
//! 9       1     flags = 0 (reserved; must be 0 in v1)
//! 10      n     root FEN, UTF-8, '\n'-terminated (the FEN the search solved)
//! +0      4     tt_size_mb: u32
//! +4      4     generation: u32 (table generation at dump time)
//! +8      8     solved_count: u64
//! +16     8     unsolved_count: u64
//!
//! solved record × solved_count          (15 bytes each)
//!   key: u64
//!   outcome: u8              (same encoding as the proof-tree dump's root
//!                             outcome byte: Draw=0, Win=1, Loss=2)
//!   depth: u32
//!   best_move: u16           (16-bit move code; 0xFFFF = Move::NONE)
//!
//! unsolved record × unsolved_count      (42 bytes each)
//!   key: u64
//!   pn: u64
//!   dn: u64
//!   depth: u32
//!   remaining_depth: u32
//!   best_move: u16           (0xFFFF = Move::NONE)
//!   work: u64                (cumulative child_evals under the subtree)
//! ```
//!
//! # Section policy
//!
//! - **Solved section**: every valid entry with a stored outcome, from *all*
//!   generations. Solved results are path-independent base entries (solved
//!   results are permanent; repetition-dependent results are never stored —
//!   the first-player-loss GHI shortcut), so stale-generation solved entries
//!   are free coverage against eviction.
//! - **Unsolved section**: valid entries with no stored outcome from the
//!   *current generation only*. Unsolved `pn`/`dn` bounds are only meaningful
//!   for the generation they were computed in. These records are best-effort
//!   context for the reconstruction's hole-filling searches (seed TT) and for
//!   debugging; the reconstruction must not rely on them.
//! - **Order**: native bucket order, filtered per section. Dumps are
//!   byte-identical for identical searches and cheap to write (no sort).
//!
//! # Lookup caveat
//!
//! Zobrist keys include the halfmove clock (`crate::zobrist`), so a snapshot
//! lookup by a consumer must use the exact key along its own path; clock
//! mismatches via transpositions are an anticipated hole class for the
//! reconstruction tool.
//!
//! # Strictness
//!
//! Readers must reject wrong magic, unknown versions, nonzero flags, and
//! truncated files. Counts are authoritative; no trailing bytes are allowed
//! beyond the last record. Bump the version rather than growing v1.

use std::io::{self, Write};

use atomic_movegen::types::Move;

use crate::position::Outcome;
use crate::proof_tree::binary::outcome_to_u8;
use crate::search::tt::TranspositionTable;

const MAGIC: &[u8; 8] = b"ATOMTTSN";
const VERSION: u8 = 1;
const FLAGS: u8 = 0;
/// Sentinel for `Move::NONE`. `move_to_bits` never produces it (its maximum
/// is `0x7FFF`), so the sentinel is unambiguous.
const MOVE_NONE_BITS: u16 = 0xFFFF;

const SOLVED_RECORD_BYTES: u64 = 15;
const UNSOLVED_RECORD_BYTES: u64 = 42;

/// Aggregate counts of a written snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotSummary {
    /// Solved (outcome-carrying) records written, all generations.
    pub solved: u64,
    /// Unsolved (bound-only) records written, current generation only.
    pub unsolved: u64,
    /// Total bytes written, header included.
    pub bytes: u64,
}

/// Header of a snapshot file (everything before the record sections).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotHeader {
    /// The FEN the search solved.
    pub root_fen: String,
    /// Transposition-table size in megabytes as configured for the search.
    pub tt_size_mb: u32,
    /// Table generation at dump time.
    pub generation: u32,
    /// Number of solved records following the header.
    pub solved_count: u64,
    /// Number of unsolved records following the solved section.
    pub unsolved_count: u64,
}

/// A solved TT record: a path-independent, position-truth result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolvedRecord {
    pub key: u64,
    pub outcome: Outcome,
    pub depth: u32,
    pub best_move: Move,
}

/// An unsolved TT record: generation-local pn/dn bounds, best-effort context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsolvedRecord {
    pub key: u64,
    pub pn: u64,
    pub dn: u64,
    pub depth: u32,
    pub remaining_depth: u32,
    pub best_move: Move,
    pub work: u64,
}

fn move_to_u16(mv: Move) -> u16 {
    if mv == Move::NONE {
        MOVE_NONE_BITS
    } else {
        crate::notation::move_to_bits(mv)
    }
}

fn move_from_u16(bits: u16) -> io::Result<Move> {
    if bits == MOVE_NONE_BITS {
        return Ok(Move::NONE);
    }
    crate::notation::bits_to_move(bits)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid move code"))
}

/// Write a snapshot of `tt` to `w`, partitioned per the section policy.
///
/// The table is scanned three times (count, solved pass, unsolved pass); every
/// scan is a linear pass over in-memory buckets, so no record buffering is
/// needed and the peak extra memory is O(1).
pub fn write_tt_snapshot<W: Write>(
    tt: &TranspositionTable,
    root_fen: &str,
    tt_size_mb: u32,
    w: &mut W,
) -> io::Result<SnapshotSummary> {
    let generation = tt.current_generation();
    let mut solved_count: u64 = 0;
    let mut unsolved_count: u64 = 0;
    tt.for_each_entry(|e| {
        if e.outcome.is_some() {
            solved_count += 1;
        } else if e.generation == generation {
            unsolved_count += 1;
        }
    });

    w.write_all(MAGIC)?;
    w.write_all(&[VERSION, FLAGS])?;
    writeln!(w, "{root_fen}")?;
    w.write_all(&tt_size_mb.to_le_bytes())?;
    w.write_all(&generation.to_le_bytes())?;
    w.write_all(&solved_count.to_le_bytes())?;
    w.write_all(&unsolved_count.to_le_bytes())?;

    let mut solved: u64 = 0;
    let mut unsolved: u64 = 0;
    // `f` may not re-enter the table, but may write to `w`; the first write
    // error is captured and returned after the pass (the closure cannot use
    // `?` because `for_each_entry` takes a plain `FnMut`).
    let mut err: Option<io::Error> = None;
    tt.for_each_entry(|e| {
        if err.is_some() {
            return;
        }
        if let Some(outcome) = e.outcome {
            let key = e.key.to_le_bytes();
            let outcome = [outcome_to_u8(outcome)];
            let depth = e.depth.to_le_bytes();
            let best_move = move_to_u16(e.best_move).to_le_bytes();
            match write_record(w, &[&key, &outcome, &depth, &best_move]) {
                Ok(()) => solved += 1,
                Err(e) => err = Some(e),
            }
        }
    });
    tt.for_each_entry(|e| {
        if err.is_some() {
            return;
        }
        if e.outcome.is_none() && e.generation == generation {
            let key = e.key.to_le_bytes();
            let pn = e.pn.to_le_bytes();
            let dn = e.dn.to_le_bytes();
            let depth = e.depth.to_le_bytes();
            let remaining_depth = e.remaining_depth.to_le_bytes();
            let best_move = move_to_u16(e.best_move).to_le_bytes();
            let work = e.work.to_le_bytes();
            match write_record(
                w,
                &[&key, &pn, &dn, &depth, &remaining_depth, &best_move, &work],
            ) {
                Ok(()) => unsolved += 1,
                Err(e) => err = Some(e),
            }
        }
    });
    if let Some(e) = err {
        return Err(e);
    }

    debug_assert_eq!(solved, solved_count);
    debug_assert_eq!(unsolved, unsolved_count);

    let bytes =
        header_bytes(root_fen) + solved * SOLVED_RECORD_BYTES + unsolved * UNSOLVED_RECORD_BYTES;
    Ok(SnapshotSummary {
        solved,
        unsolved,
        bytes,
    })
}

fn header_bytes(root_fen: &str) -> u64 {
    8 + 1 + 1 + root_fen.len() as u64 + 1 + 4 + 4 + 8 + 8
}

/// Write the given byte slices as one record (helper so the
/// `for_each_entry` closures can stay infallible).
fn write_record<W: Write>(w: &mut W, parts: &[&[u8]]) -> io::Result<()> {
    for p in parts {
        w.write_all(p)?;
    }
    Ok(())
}

fn invalid_data(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

mod reader;

#[cfg(test)]
mod tests;

pub use reader::read_tt_snapshot;
