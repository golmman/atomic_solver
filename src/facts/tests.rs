//! Unit tests for the facts channel (plan11): record v2 codec invariants,
//! the `PATH_LEN_CAP` admission filter, the synchronous exporter's flush and
//! silent-stop behavior, the threadless importer's capped incremental reads
//! (torn tail, corrupt record, read-cap catch-up, discovery), import
//! idempotence, `resolved_from_entry` equivalence, and export-inertness of
//! the hook wiring.
//!
//! File size: above the 10 KiB guideline because the tests exercise private
//! invariants of `super` (the shared writer state, the fixed constants); a
//! separate integration-test crate could not reach them without widening the
//! public surface.

use std::sync::Arc;

use atomic_movegen::types::{Move, Square};

use super::*;
use crate::position::{Outcome, Position};
use crate::search::dfpn::{FactImporter, Search};

fn record(key: u64, outcome: Outcome, depth: u32, path_len: u8) -> FactsRecord {
    FactsRecord {
        key,
        outcome,
        depth,
        path_len,
    }
}

fn encode_all(records: &[FactsRecord]) -> Vec<u8> {
    records.iter().flat_map(|r| r.encode()).collect()
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("facts11_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn codec_round_trip() {
    for (outcome, depth, path_len) in [
        (Outcome::Win, 0, 0),
        (Outcome::Loss, 1, 24),
        (Outcome::Win, 75, 255),
        (Outcome::Loss, u32::MAX, 12),
    ] {
        let rec = record(0x1234_5678_9abc_def0, outcome, depth, path_len);
        let buf = rec.encode();
        assert_eq!(buf.len(), RECORD_LEN);
        assert_eq!(FactsRecord::decode(&buf), Some(rec));
    }
    // Padding bytes are zero (byte 9 carries path_len).
    let buf = record(7, Outcome::Win, 5, 3).encode();
    assert_eq!(buf[9], 3);
    assert!(buf[10..12].iter().all(|&b| b == 0));
    assert!(buf[20..24].iter().all(|&b| b == 0));
}

#[test]
fn checksum_rejection_drops_corrupt_records() {
    let buf = record(42, Outcome::Loss, 9, 5).encode();
    for idx in [0usize, 8, 9, 13, 17] {
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

/// The admission filter is a fixed constant (`PATH_LEN_CAP = 24`): the
/// boundary record (24) is emitted, one ply beyond is filtered and counted.
#[test]
fn exporter_filters_at_path_len_cap() {
    let dir = temp_dir("filter");
    let shared = FactsExportShared::create(&dir.join("facts-s0.bin")).unwrap();
    let exporter = FactsExporter::new(Arc::clone(&shared));

    // Draws never enter the channel and are not counted (the emission sites
    // never call with one; this is defense in depth).
    exporter.export(1, Outcome::Draw, 3, 0);
    exporter.export(2, Outcome::Win, 7, PATH_LEN_CAP);
    exporter.export(3, Outcome::Loss, 11, PATH_LEN_CAP + 1);
    exporter.export(4, Outcome::Win, 13, 200);
    exporter.export(5, Outcome::Win, 0, PATH_LEN_CAP.saturating_sub(1));

    shared.finish();
    let s = shared.stats();
    assert_eq!(
        (s.records_seen, s.emitted, s.filtered),
        (4, 2, 2),
        "23 and 24 plies emitted, 25 and 200 filtered; the draw call never reaches the counters"
    );
    assert_eq!(s.bytes, 2 * RECORD_LEN as u64);

    let bytes = std::fs::read(dir.join("facts-s0.bin")).unwrap();
    assert_eq!(bytes.len(), 2 * RECORD_LEN);
    let got: Vec<FactsRecord> = bytes
        .as_chunks::<RECORD_LEN>()
        .0
        .iter()
        .map(FactsRecord::decode)
        .collect::<Option<Vec<_>>>()
        .unwrap();
    assert_eq!(
        got,
        vec![
            record(2, Outcome::Win, 7, PATH_LEN_CAP),
            record(5, Outcome::Win, 0, PATH_LEN_CAP - 1),
        ],
        "23 and 24 plies pass the cap, everything beyond is filtered"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The exporter buffers (`FLUSH_RECORDS`) and the pre-exit `finish()` puts
/// everything on disk, record-aligned.
#[test]
fn exporter_buffers_then_flushes_everything_at_finish() {
    let dir = temp_dir("flush");
    let shared = FactsExportShared::create(&dir.join("facts-s0.bin")).unwrap();
    let exporter = FactsExporter::new(Arc::clone(&shared));

    for i in 0..(FLUSH_RECORDS + 10) {
        exporter.export(i as u64, Outcome::Win, i as u32, 1);
    }
    // The automatic flush at the FLUSH_RECORDS boundary is already on disk.
    let mid = std::fs::metadata(dir.join("facts-s0.bin")).unwrap().len();
    assert_eq!(mid, FLUSH_RECORDS as u64 * RECORD_LEN as u64);

    shared.finish();
    let after = std::fs::read(dir.join("facts-s0.bin")).unwrap();
    assert_eq!(after.len(), (FLUSH_RECORDS + 10) * RECORD_LEN);
    // finish() is idempotent.
    shared.finish();
    assert_eq!(
        std::fs::read(dir.join("facts-s0.bin")).unwrap().len(),
        (FLUSH_RECORDS + 10) * RECORD_LEN
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The threadless importer discovers `facts-*.bin` files (only), reads them
/// incrementally, and respects a per-call read cap with catch-up.
#[test]
fn importer_reads_incrementally_with_cap_catchup() {
    let dir = temp_dir("cap");
    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let records: Vec<FactsRecord> = (0..50)
        .map(|i| record(pos.hash() ^ i, Outcome::Win, i as u32, (i % 30) as u8))
        .collect();
    std::fs::write(dir.join("facts-s1.bin"), encode_all(&records)).unwrap();
    // A foreign file is never discovered.
    std::fs::write(dir.join("other.bin"), encode_all(&records)).unwrap();

    // Read cap of 10 records per call forces three calls for 50 records.
    let mut importer = FactsImporter::with_limits(&dir, 10 * RECORD_LEN as u64, RESCAN_INTERVAL);
    let mut tt = crate::search::tt::TranspositionTable::with_mb(1);

    let mut accepted = 0u64;
    for _ in 0..5 {
        let counters = importer.import(&mut tt);
        accepted += counters.accepted;
        if accepted == 50 {
            break;
        }
    }
    assert_eq!(accepted, 50, "persistent offsets catch up across the cap");
    assert!(tt.probe(pos.hash() ^ 49).unwrap().outcome.is_some());
    assert_eq!(tt.probe(0xdead_beef).map(|e| e.outcome), None);
    let s = importer.stats();
    assert_eq!(s.files_seen, 1, "only facts-*.bin is discovered");
    assert_eq!(s.records_read, 50);
    assert_eq!(s.checksum_failures, 0);
    assert_eq!(s.bytes_read, 50 * RECORD_LEN as u64);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A torn tail (a writer flush that is not record-aligned) is left for the
/// next hook call and completed records are never re-read.
#[test]
fn importer_retries_torn_tail() {
    let dir = temp_dir("torn");
    let path = dir.join("facts-s2.bin");
    let records = [
        record(1, Outcome::Win, 3, 1),
        record(2, Outcome::Loss, 4, 2),
    ];
    // Two complete records + 10 bytes of a third.
    let mut bytes = encode_all(&records);
    bytes.extend_from_slice(&encode_all(&[record(3, Outcome::Win, 5, 3)])[..10]);
    std::fs::write(&path, &bytes).unwrap();

    let mut importer = FactsImporter::new(&dir);
    let mut tt = crate::search::tt::TranspositionTable::with_mb(1);
    let counters = importer.import(&mut tt);
    assert_eq!(counters.accepted, 2, "only complete records are consumed");

    // The writer appends the rest of the third record.
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    use std::io::Write;
    file.write_all(&encode_all(&[record(3, Outcome::Win, 5, 3)])[10..])
        .unwrap();
    drop(file);

    let counters = importer.import(&mut tt);
    assert_eq!(counters.accepted, 1, "the torn tail completed and was read");
    assert!(tt.probe(3).unwrap().outcome.is_some());
    let s = importer.stats();
    assert_eq!(s.records_read, 3);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A checksum failure skips exactly one record stride; alignment and later
/// records survive.
#[test]
fn importer_skips_corrupt_record_and_keeps_alignment() {
    let dir = temp_dir("crc");
    let good = record(0xaaaa, Outcome::Win, 3, 1);
    let mut corrupt = record(0xbbbb, Outcome::Win, 3, 1).encode();
    corrupt[0] ^= 0xff; // payload corruption, stale CRC
    let mut bytes = corrupt.to_vec();
    bytes.extend_from_slice(&good.encode());
    std::fs::write(dir.join("facts-s3.bin"), &bytes).unwrap();

    let mut importer = FactsImporter::new(&dir);
    let mut tt = crate::search::tt::TranspositionTable::with_mb(1);
    let counters = importer.import(&mut tt);
    assert_eq!(
        counters.accepted, 1,
        "the good record after the corrupt one was still imported"
    );
    let s = importer.stats();
    assert_eq!(s.checksum_failures, 1, "the corrupt record was dropped");
    assert_eq!(s.records_read, 1, "only the good record was admitted");
    assert!(tt.probe(good.key).is_some_and(|e| e.outcome.is_some()));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Import idempotence and the exact self-proven store path.
#[test]
fn import_is_idempotent_and_stores_like_self_proven() {
    let dir = temp_dir("idem");
    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let win = record(pos.hash(), Outcome::Win, 15, 2);
    let loss = record(pos.hash() ^ 1, Outcome::Loss, 22, 3);
    std::fs::write(dir.join("facts-s9.bin"), encode_all(&[win, loss])).unwrap();

    let mut importer = FactsImporter::new(&dir);
    let mut tt = crate::search::tt::TranspositionTable::with_mb(1);
    let counters = importer.import(&mut tt);
    assert_eq!(
        (counters.accepted, counters.skipped),
        (2, 0),
        "import-all within the emitted stream"
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

    // Idempotence: re-delivering the same facts skips solved keys.
    std::fs::write(dir.join("facts-s9.bin"), encode_all(&[win, loss])).unwrap();
    importer.offsets.clear();
    importer.discovered = false;
    let counters = importer.import(&mut tt);
    assert_eq!(
        (counters.accepted, counters.skipped),
        (0, 2),
        "already-solved keys are skipped"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Discovery respects the rescan cadence: a file created after the first
/// hook call is found only after the interval elapses.
#[test]
fn importer_rescan_is_throttled() {
    let dir = temp_dir("rescan");
    let pos = Position::from_fen("4k3/8/8/8/8/8/8/4R1K1 w - - 0 1").unwrap();
    let rec = record(pos.hash(), Outcome::Win, 3, 1);

    let mut importer = FactsImporter::with_limits(&dir, IMPORT_READ_CAP, Duration::from_millis(80));
    let mut tt = crate::search::tt::TranspositionTable::with_mb(1);
    assert_eq!(importer.import(&mut tt).accepted, 0, "empty dir");

    std::fs::write(dir.join("facts-late.bin"), encode_all(&[rec])).unwrap();
    assert_eq!(
        importer.import(&mut tt).accepted,
        0,
        "new file is not discovered before the rescan interval"
    );
    std::thread::sleep(Duration::from_millis(120));
    assert_eq!(
        importer.import(&mut tt).accepted,
        1,
        "discovered after the interval"
    );
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

/// Export-inertness (H4a/H4e at the API level): wiring the facts exporter
/// (proof-event sender stays `None`, as the CLI does) must not perturb the
/// search's outcome, PV, nodes, or work.
#[test]
fn export_wiring_is_inert() {
    let fen = "r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35";
    let solve = |with_facts: bool| {
        let dir = temp_dir("inert");
        let mut search = Search::new(4);
        let mut shared = None;
        if with_facts {
            shared = Some(FactsExportShared::create(&dir.join("facts-s0.bin")).unwrap());
            search.set_fact_exporter(Some(Arc::new(FactsExporter::new(Arc::clone(
                shared.as_ref().unwrap(),
            )))));
        }
        let mut pos = Position::from_fen(fen).unwrap();
        search.set_first_outcome_only(true);
        let result = (
            search.solve(&mut pos),
            search.child_evaluations(),
            search.nodes(),
        );
        if let Some(s) = shared {
            s.finish();
        }
        drop(search);
        std::fs::remove_dir_all(&dir).unwrap();
        result
    };
    let plain = solve(false);
    let wired = solve(true);
    assert_eq!(plain, wired, "export wiring must be search-inert");
}
