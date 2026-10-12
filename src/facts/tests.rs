//! Unit tests for the facts channel (plan10): codec invariants, writer
//! no-Draw invariant, import idempotence, `resolved_from_entry` equivalence,
//! and export-inertness of the proof-event sender.
//!
//! File size: above the 10 KiB guideline because the tests exercise private
//! invariants of `super` (the shared queue, the reader-thread lifecycle, the
//! A2 admission arm); a separate integration-test crate could not reach
//! them without widening the public surface.

use std::sync::mpsc;

use atomic_movegen::types::{Move, Square};

use super::*;
use crate::position::{Outcome, Position};
use crate::search::dfpn::Search;

fn record(key: u64, outcome: Outcome, depth: u32) -> FactsRecord {
    FactsRecord {
        key,
        outcome,
        depth,
    }
}

#[test]
fn codec_round_trip() {
    for (outcome, depth) in [
        (Outcome::Win, 0),
        (Outcome::Loss, 1),
        (Outcome::Win, 75),
        (Outcome::Loss, u32::MAX),
    ] {
        let rec = record(0x1234_5678_9abc_def0, outcome, depth);
        let buf = rec.encode();
        assert_eq!(buf.len(), RECORD_LEN);
        assert_eq!(FactsRecord::decode(&buf), Some(rec));
    }
    // Padding bytes are zero.
    let buf = record(7, Outcome::Win, 5).encode();
    assert!(buf[9..12].iter().all(|&b| b == 0));
    assert!(buf[20..24].iter().all(|&b| b == 0));
}

#[test]
fn checksum_rejection_drops_corrupt_records() {
    let buf = record(42, Outcome::Loss, 9).encode();
    for idx in [0usize, 8, 13, 17] {
        let mut corrupt = buf;
        corrupt[idx] ^= 0x01;
        assert!(
            FactsRecord::decode(&corrupt).is_none(),
            "corrupt byte {idx} must be rejected"
        );
    }
    // Unknown outcome tag rejected even with a recomputed payload CRC.
    let mut bogus = buf;
    bogus[8] = 3;
    let crc = crc32(&bogus[0..16]);
    bogus[16..20].copy_from_slice(&crc.to_le_bytes());
    assert!(FactsRecord::decode(&bogus).is_none());
}

#[test]
fn crc32_matches_known_vectors() {
    assert_eq!(crc32(b""), 0x0000_0000);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn writer_never_emits_draw_and_appends_decisive_records() {
    let dir = std::env::temp_dir().join(format!("facts_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("facts-test.bin");

    let (tx, rx) = mpsc::channel();
    let handle = spawn_writer(&path, rx).unwrap();

    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let win_key = pos.hash();
    tx.send(ProofEvent::NodeProven(crate::proof_event::NodeProven::new(
        Vec::new(),
        win_key,
        Outcome::Draw,
        3,
    )))
    .unwrap();
    tx.send(ProofEvent::NodeProven(crate::proof_event::NodeProven::new(
        Vec::new(),
        win_key,
        Outcome::Win,
        7,
    )))
    .unwrap();
    drop(tx);
    let stats = handle.join().unwrap();
    assert_eq!(stats.records, 1, "draws never enter the channel");
    assert_eq!(stats.bytes, RECORD_LEN as u64);

    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(bytes.len(), RECORD_LEN);
    let decoded = FactsRecord::decode(&bytes[..].try_into().unwrap()).unwrap();
    assert_eq!(decoded, record(win_key, Outcome::Win, 7));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn import_stores_solved_entries_and_is_idempotent() {
    let dir = std::env::temp_dir().join(format!("facts_import_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("facts-s9.bin");
    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let key = pos.hash();
    let win = record(key, Outcome::Win, 15);
    let loss = record(key ^ 1, Outcome::Loss, 22);
    std::fs::write(&path, encode_all(&[win, loss])).unwrap();

    let (mut importer, stats) = FactsImporter::spawn(&dir).unwrap();
    let mut tt = crate::search::tt::TranspositionTable::with_mb(1);
    // The reader thread discovers and tails asynchronously: wait until the
    // two records are verified and queued before draining.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while stats.snapshot().records_read < 2 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let counters = importer.import(&mut tt);
    assert_eq!(
        (counters.accepted, counters.skipped),
        (2, 0),
        "A1 imports every verified record"
    );

    // Imported entries reuse exactly like self-proven solved entries.
    let entry = tt.probe(win.key).unwrap();
    let resolved = Search::test_resolved_from_entry(entry, u32::MAX);
    assert_eq!(resolved, Some((Outcome::Win, 15)));
    assert_eq!(entry.remaining_depth, u32::MAX);
    assert_eq!(entry.best_move, Move::NONE);
    assert_eq!(
        entry.work, 0,
        "imported entries are preferred eviction victims"
    );

    // Idempotence: re-delivering the same facts skips solved keys. The
    // reader re-delivers by design (duplicates are the mechanism for facts
    // evicted from a consumer TT); emulate by re-queueing the records.
    {
        let mut queue = importer.shared.queue.lock().unwrap();
        queue.push_back(win);
        queue.push_back(loss);
    }
    let counters = importer.import(&mut tt);
    assert_eq!(
        (counters.accepted, counters.skipped),
        (0, 2),
        "already-solved keys are skipped"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn reader_tails_export_file_incrementally() {
    let dir = std::env::temp_dir().join(format!("facts_tail_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("facts-s1.bin");

    let (tx, rx) = mpsc::channel();
    let writer = spawn_writer(&path, rx).unwrap();
    let (mut importer, stats) = FactsImporter::spawn(&dir).unwrap();
    let mut tt = crate::search::tt::TranspositionTable::with_mb(1);

    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let key = pos.hash();
    tx.send(ProofEvent::NodeProven(crate::proof_event::NodeProven::new(
        Vec::new(),
        key,
        Outcome::Loss,
        4,
    )))
    .unwrap();
    drop(tx);
    let _ = writer.join().unwrap();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut counters = FactImportCounters::default();
    while std::time::Instant::now() < deadline {
        counters = importer.import(&mut tt);
        if counters.accepted == 1 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(counters.accepted, 1, "the reader picked up the export");
    assert!(tt.probe(key).unwrap().outcome.is_some());
    let s = stats.snapshot();
    assert_eq!(s.records_read, 1);
    assert_eq!(s.checksum_failures, 0);
    assert_eq!(s.files_seen, 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn reader_skips_corrupt_record_and_keeps_alignment() {
    let dir = std::env::temp_dir().join(format!("facts_crc_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("facts-s2.bin");

    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let key = pos.hash();
    let good = record(key, Outcome::Win, 3);
    let mut corrupt = record(key ^ 2, Outcome::Win, 3).encode();
    corrupt[0] ^= 0xff; // payload corruption, stale CRC
    let mut bytes = corrupt.to_vec();
    bytes.extend_from_slice(&good.encode());
    std::fs::write(&path, &bytes).unwrap();

    let (mut importer, stats) = FactsImporter::spawn(&dir).unwrap();
    let mut tt = crate::search::tt::TranspositionTable::with_mb(1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        let counters = importer.import(&mut tt);
        if counters.accepted == 1 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let s = stats.snapshot();
    assert_eq!(s.checksum_failures, 1, "the corrupt record was dropped");
    assert_eq!(s.records_read, 1, "only the good record was queued");
    assert!(
        tt.probe(good.key).is_some_and(|e| e.outcome.is_some()),
        "the good record after the corrupt one was still imported"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn loss_only_arm_filters_win_records() {
    let dir = std::env::temp_dir().join(format!("facts_a2_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("facts-s3.bin");
    let loss = record(0xabcd, Outcome::Loss, 11);
    let win = record(0xdcba, Outcome::Win, 12);
    std::fs::write(&path, encode_all(&[win, loss])).unwrap();

    // The A2 arm is environment-selected; construct the importer directly
    // with the arm flag instead of going through the env var.
    let shared = Arc::new(super::FactsShared {
        queue: Mutex::new(VecDeque::from([win, loss])),
        space_available: Condvar::new(),
        records_read: AtomicU64::new(2),
        checksum_failures: AtomicU64::new(0),
        accepted: AtomicU64::new(0),
        skipped_solved: AtomicU64::new(0),
        skipped_filtered: AtomicU64::new(0),
        files_seen: AtomicU64::new(0),
        stop_flag: AtomicBool::new(false),
    });
    let mut importer = FactsImporter {
        shared: Arc::clone(&shared),
        loss_only: true,
    };
    let mut tt = crate::search::tt::TranspositionTable::with_mb(1);
    let counters = importer.import(&mut tt);
    assert_eq!(
        (counters.accepted, counters.skipped),
        (1, 0),
        "A2 imports Loss facts only"
    );
    assert!(tt.probe(loss.key).unwrap().outcome == Some(Outcome::Loss));
    assert!(
        tt.probe(win.key).is_none(),
        "Win record not admitted under A2"
    );
    assert_eq!(shared.skipped_filtered.load(Ordering::Relaxed), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// An imported solved entry and a self-proven solved entry at the same
/// key/depth resolve identically through the solved-entry reuse check.
#[test]
fn imported_entry_resolves_like_self_proven() {
    let mut pos = Position::from_fen("4k3/8/8/8/8/8/8/4KRR1 w - - 0 1").unwrap();
    // g1h1 is a quiet rook move to a non-terminal child.
    let mv = Move::make_move(Square::G1, Square::H1);
    pos.do_move(mv);
    let child_key = pos.hash();
    pos.undo_move(mv);

    let mut search = Search::new(1);
    // Store exactly what an imported fact stores (import path parameters).
    let (pn, dn) = Outcome::Loss.to_pn_dn();
    search.tt_mut().store(
        child_key,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Loss),
        pn,
        dn,
        9,
        u32::MAX,
    );
    let entry = search.tt().probe(child_key).unwrap();
    let resolved = Search::test_resolved_from_entry(entry, u32::MAX);
    assert_eq!(resolved, Some((Outcome::Loss, 9)));

    // And the depth guard keeps the entry unreusable below its proven depth.
    assert_eq!(
        Search::test_resolved_from_entry(entry, 8),
        None,
        "depth > max_depth must not resolve"
    );
}

/// Export-inertness (H4a/H4e at the API level): wiring a proof-event sender
/// plus a facts writer must not perturb the search's outcome, PV, or work.
#[test]
fn export_wiring_is_inert() {
    let fen = "r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35";
    let solve = |with_facts: bool| {
        let dir = std::env::temp_dir().join(format!("facts_inert_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut search = Search::new(4);
        let mut writer = None;
        if with_facts {
            let (tx, rx) = mpsc::channel();
            search.set_proof_event_sender(Some(tx));
            writer = Some(spawn_writer(&dir.join("facts-s0.bin"), rx).unwrap());
        }
        let mut pos = Position::from_fen(fen).unwrap();
        search.set_first_outcome_only(true);
        let result = (
            search.solve(&mut pos),
            search.child_evaluations(),
            search.nodes(),
        );
        drop(search);
        if let Some(w) = writer {
            let _ = w.join().unwrap();
        }
        std::fs::remove_dir_all(&dir).unwrap();
        result
    };
    let plain = solve(false);
    let wired = solve(true);
    assert_eq!(plain, wired, "export wiring must be search-inert");
}

fn encode_all(records: &[FactsRecord]) -> Vec<u8> {
    records.iter().flat_map(|r| r.encode()).collect()
}
