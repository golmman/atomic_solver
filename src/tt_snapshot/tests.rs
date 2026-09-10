//! Tests for the TT snapshot writer and strict reader.

use super::read_tt_snapshot;
use super::{SolvedRecord, UnsolvedRecord, write_tt_snapshot};
use crate::position::Outcome;
use crate::search::tt::TranspositionTable;
use atomic_movegen::types::{Move, Square};
use std::io::Cursor;

fn store_solved(tt: &mut TranspositionTable, key: u64, outcome: Outcome, depth: u32) {
    tt.store(
        key,
        Move::make_move(Square::E2, Square::E4),
        u8::MAX,
        100,
        Some(outcome),
        0,
        crate::zobrist::INF,
        depth,
        u32::MAX,
    );
}

fn store_unsolved(tt: &mut TranspositionTable, key: u64, pn: u64, dn: u64, work: u64) {
    tt.store(
        key,
        Move::make_move(Square::D2, Square::D4),
        3,
        work,
        None,
        pn,
        dn,
        7,
        5,
    );
}

fn sample_table() -> TranspositionTable {
    let mut tt = TranspositionTable::with_capacity(4);
    store_solved(&mut tt, 0x101, Outcome::Win, 9);
    store_solved(&mut tt, 0x102, Outcome::Loss, 13);
    store_unsolved(&mut tt, 0x103, 5, 8, 1000);
    tt
}

#[test]
fn round_trip_preserves_every_field() {
    let tt = sample_table();
    let mut buf = Vec::new();
    let summary = write_tt_snapshot(&tt, "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1", 128, &mut buf)
        .expect("write should succeed");

    let (header, solved, unsolved) =
        read_tt_snapshot(&mut Cursor::new(&buf)).expect("read should succeed");

    assert_eq!(header.root_fen, "4k3/8/8/8/8/8/8/4KRR1 w - - 0 1");
    assert_eq!(header.tt_size_mb, 128);
    assert_eq!(header.generation, tt.current_generation());
    assert_eq!(header.solved_count, 2);
    assert_eq!(header.unsolved_count, 1);
    assert_eq!(summary.solved, 2);
    assert_eq!(summary.unsolved, 1);
    assert_eq!(summary.bytes, buf.len() as u64);

    assert_eq!(solved.len(), 2);
    let e2e4 = Move::make_move(Square::E2, Square::E4);
    assert!(solved.contains(&SolvedRecord {
        key: 0x101,
        outcome: Outcome::Win,
        depth: 9,
        best_move: e2e4,
    }));
    assert!(solved.contains(&SolvedRecord {
        key: 0x102,
        outcome: Outcome::Loss,
        depth: 13,
        best_move: e2e4,
    }));

    assert_eq!(
        unsolved,
        vec![UnsolvedRecord {
            key: 0x103,
            pn: 5,
            dn: 8,
            depth: 7,
            remaining_depth: 5,
            best_move: Move::make_move(Square::D2, Square::D4),
            work: 1000,
        }]
    );
}

#[test]
fn generation_policy_solved_all_unsolved_current_only() {
    let mut tt = TranspositionTable::with_capacity(8);
    store_solved(&mut tt, 0x201, Outcome::Win, 3); // stale solved
    store_unsolved(&mut tt, 0x202, 1, 2, 10); // stale unsolved
    tt.new_generation();
    store_solved(&mut tt, 0x203, Outcome::Loss, 6); // current solved
    store_unsolved(&mut tt, 0x204, 3, 4, 20); // current unsolved

    let mut buf = Vec::new();
    write_tt_snapshot(&tt, "test", 1, &mut buf).unwrap();
    let (_, solved, unsolved) = read_tt_snapshot(&mut Cursor::new(&buf)).unwrap();

    let solved_keys: Vec<u64> = solved.iter().map(|r| r.key).collect();
    let unsolved_keys: Vec<u64> = unsolved.iter().map(|r| r.key).collect();
    assert!(solved_keys.contains(&0x201), "stale solved must be kept");
    assert!(solved_keys.contains(&0x203));
    assert!(!unsolved_keys.contains(&0x202), "stale unsolved must drop");
    assert!(unsolved_keys.contains(&0x204));
}

#[test]
fn move_none_sentinel_round_trips() {
    let mut tt = TranspositionTable::with_capacity(2);
    tt.store(
        0x301,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Draw),
        0,
        0,
        4,
        u32::MAX,
    );
    tt.store(0x302, Move::NONE, u8::MAX, 0, None, 1, 1, 0, 0);

    let mut buf = Vec::new();
    write_tt_snapshot(&tt, "test", 1, &mut buf).unwrap();
    let (_, solved, unsolved) = read_tt_snapshot(&mut Cursor::new(&buf)).unwrap();

    assert_eq!(solved[0].best_move, Move::NONE);
    assert_eq!(unsolved[0].best_move, Move::NONE);
}

#[test]
fn move_none_serializes_as_0xffff() {
    let mut tt = TranspositionTable::with_capacity(1);
    tt.store(
        0x401,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Draw),
        0,
        0,
        0,
        u32::MAX,
    );
    let mut buf = Vec::new();
    write_tt_snapshot(&tt, "test", 1, &mut buf).unwrap();
    // Solved record starts right after the 8-byte magic + version + flags
    // + "test\n" FEN line + 24-byte counts block.
    let record_start = 8 + 1 + 1 + 5 + 24;
    let best_move_off = record_start + 8 + 1 + 4;
    let bits = u16::from_le_bytes([buf[best_move_off], buf[best_move_off + 1]]);
    assert_eq!(bits, 0xFFFF);
}

#[test]
fn rejects_bad_magic_version_flags_and_truncation() {
    let tt = sample_table();
    let mut buf = Vec::new();
    write_tt_snapshot(&tt, "test", 1, &mut buf).unwrap();

    // Bad magic.
    let mut bad = buf.clone();
    bad[0] = b'X';
    assert!(read_tt_snapshot(&mut Cursor::new(&bad)).is_err());

    // Unknown version.
    let mut bad = buf.clone();
    bad[8] = 99;
    assert!(read_tt_snapshot(&mut Cursor::new(&bad)).is_err());

    // Nonzero flags.
    let mut bad = buf.clone();
    bad[9] = 1;
    assert!(read_tt_snapshot(&mut Cursor::new(&bad)).is_err());

    // Truncated stream (every proper prefix must be rejected).
    for len in 0..buf.len() {
        assert!(
            read_tt_snapshot(&mut Cursor::new(&buf[..len])).is_err(),
            "prefix of {len} bytes should be rejected"
        );
    }

    // Count mismatch: header says one more record than the file holds.
    let mut bad = buf.clone();
    let counts_off = 8 + 1 + 1 + 5 + 8; // solved_count offset
    let n = u64::from_le_bytes(bad[counts_off..counts_off + 8].try_into().unwrap());
    bad[counts_off..counts_off + 8].copy_from_slice(&(n + 1).to_le_bytes());
    assert!(read_tt_snapshot(&mut Cursor::new(&bad)).is_err());
}

#[test]
fn identical_tables_produce_identical_bytes() {
    let a = sample_table();
    let b = sample_table();
    let mut buf_a = Vec::new();
    let mut buf_b = Vec::new();
    write_tt_snapshot(&a, "fen", 4, &mut buf_a).unwrap();
    write_tt_snapshot(&b, "fen", 4, &mut buf_b).unwrap();
    assert_eq!(buf_a, buf_b);
}

#[test]
fn solved_records_appear_in_bucket_order() {
    let tt = sample_table();
    let mut buf = Vec::new();
    write_tt_snapshot(&tt, "test", 1, &mut buf).unwrap();
    let (_, solved, _) = read_tt_snapshot(&mut Cursor::new(&buf)).unwrap();

    let expected: Vec<u64> = tt
        .entries()
        .filter(|e| e.outcome.is_some())
        .map(|e| e.key)
        .collect();
    let actual: Vec<u64> = solved.iter().map(|r| r.key).collect();
    assert_eq!(actual, expected);
}

#[test]
fn empty_table_writes_header_only() {
    let tt = TranspositionTable::with_capacity(4);
    let mut buf = Vec::new();
    let summary = write_tt_snapshot(&tt, "k/8 w - - 0 1", 2, &mut buf).unwrap();
    assert_eq!(summary.solved, 0);
    assert_eq!(summary.unsolved, 0);
    let (header, solved, unsolved) = read_tt_snapshot(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(header.solved_count, 0);
    assert_eq!(header.unsolved_count, 0);
    assert!(solved.is_empty() && unsolved.is_empty());
}
