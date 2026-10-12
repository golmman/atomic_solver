//! Cross-racer solved-fact channel (plan11, `parallel` backlog #12; the
//! low-tax carrier rework of plan10's mechanism, backlog #6).
//!
//! A cooperative portfolio racer exports its *near-root* solved Win/Loss
//! facts to an append-only binary file (`--facts-out <file>`) and imports
//! the sibling racers' facts into its transposition table
//! (`--facts-in <dir>`). The channel carries exactly the TT's
//! path-independent solved class, keyed by the full Zobrist hash
//! (piece/side/castling/en-passant **plus the halfmove clock**), so a fact
//! is valid exactly at the (position, clock) it was proven at — the
//! report10 cross-clock reuse channel is closed by the key.
//!
//! # Carrier shape (plan11; what changed vs plan10)
//!
//! The report10 wall tax was attributed to three plumbing artifacts, all of
//! which are structurally absent here:
//!
//! - **Key-only emission.** The export path is the one-method
//!   [`crate::search::dfpn::FactExporter`] hook, a pure observer called at
//!   the same solved-result sites the proof events reach. It reads only
//!   values already computed (hash, outcome, depth, `move_stack.len()`), so
//!   no `move_stack` clone and no `ProofEvent` ever happens on the facts
//!   path; the proof-event sender stays `None` and the `ProofEvent`
//!   protocol is untouched. The search's trajectory is bit-identical with
//!   the exporter wired or not.
//! - **No threads.** The exporter writes synchronously on the search thread
//!   into a process-owned [`std::io::BufWriter`] (the search is
//!   single-threaded per process: no lock contention, no writer thread), and
//!   the plan10 reader thread, pending queue, and condvar are deleted: the
//!   [`FactsImporter`] reads the sibling `facts-*.bin` files directly, on
//!   the search thread, at work-chunk boundaries (see the
//!   [`crate::search::dfpn::FactImporter`] hook contract — the I/O location
//!   moved to the hook calls, the boundaries did not change).
//! - **Admission filter.** The exporter emits a record **iff
//!   `path_len <= PATH_LEN_CAP`** (plies from the racer's root). All racers
//!   search the same root with near-identical static ordering, so the
//!   near-root region is the shared mass — and report10's measured value
//!   came precisely from near-root subtree dedup — while deep-path records
//!   are trajectory-specific noise that pollutes the consumer TT. Import
//!   stays import-all within the emitted stream (every checksum-verified
//!   Win/Loss record).
//!
//! # Record format (24 B fixed, v2)
//!
//! ```text
//! bytes 0..8   key       u64 LE  (full Zobrist key incl. halfmove clock)
//! byte  8      outcome   u8      (1 = Win, 2 = Loss; nothing else is encoded)
//! byte  9      path_len  u8      (saturating cast of the real root distance)
//! bytes 10..12 padding   zero
//! bytes 12..16 depth     u32 LE
//! bytes 16..20 crc32     u32 LE  (CRC-32/ISO-HDLC over bytes 0..16)
//! bytes 20..24 padding   zero
//! ```
//!
//! **Checksum-verified or dropped**: a record whose CRC or outcome tag
//! fails verification is skipped, never misread (a flipped outcome byte
//! would be a false-fact injection; the CRC closes that channel). A torn
//! tail (fewer than 24 bytes available) is left for the next hook call —
//! the writer only ever appends whole records, but a `BufWriter` capacity
//! flush is not record-aligned, so offsets stay at the last *complete*
//! record and torn tails are expected and retried.
//!
//! Not compatible with plan10 v1 files (no persisted consumers; facts dirs
//! are per-race and ephemeral).
//!
//! # Soundness contract (plan10 surface, plan11 restatement)
//!
//! - Import stores through the same `TranspositionTable::store` solved path
//!   a self-proven result would take (`remaining_depth = u32::MAX`, `depth`
//!   = record depth, `best_move = Move::NONE`, `work = 0`), after probing:
//!   an already-solved entry is never overwritten (a native entry keeps its
//!   best-move hint), so reuse semantics are bit-identical to self-proven.
//! - No Draw ever crosses a process boundary; the GHI-sensitive class stays
//!   per-process. Imported entries are ordinary solved entries: eviction
//!   scoring treats `work = 0` as the cheapest (preferred eviction victim,
//!   protecting the racer's own entries).
//! - Trust base: same binary, same root FEN, peer set = the driver's
//!   spawned racers, one machine. A dropped fact is a lost optimization,
//!   never unsound.
//! - **Synchronous I/O on the search thread** is bounded per hook call:
//!   export flushes every `FLUSH_RECORDS` records (~one syscall per 256
//!   events) and at the CLI pre-exit; import reads at most
//!   [`IMPORT_READ_CAP`] bytes per call (~1 MiB ≈ 1 ms). A blocking-I/O
//!   stall is a bounded latency cost, never a correctness cost. A write
//!   failure stops exporting silently (lost optimization, never a search
//!   failure).
//! - The [`crate::search::dfpn::FactExporter`] hook is a pure observer: it
//!   reads nothing but values the solved sites already computed, so the
//!   search trajectory is bit-identical with it on or off.
//! - RAM with the facts options on = TT + [`IMPORT_READ_CAP`] (importer
//!   read cap and scratch) + one exporter `BufWriter` (~8 KiB) — smaller
//!   than plan10's ~2 MiB (no queue, no threads). Without them: RAM = TT
//!   only.
//! - The plan10 A2 escalation arm (`ATOMIC_SOLVER_FACTS_LOSS_ONLY`) and its
//!   env var are deleted; there is no escalation arm.
//!
//! File size: above the 10 KiB guideline because the record format, its
//! alignment/CRC invariants, the exporter's flush discipline, and the
//! importer's incremental capped reads share one set of soundness
//! invariants (checksum-or-drop, record alignment, Draw exclusion, near-root
//! admission); splitting them would scatter those invariants across files.
//! Unit tests are split out (`tests.rs`).

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use atomic_movegen::types::Move;

use crate::position::Outcome;
use crate::search::dfpn::{FactExporter, FactImporter};
use crate::search::tt::TranspositionTable;

/// Fixed record length in bytes.
pub const RECORD_LEN: usize = 24;

/// Outcome tag for a proven Win.
const TAG_WIN: u8 = 1;
/// Outcome tag for a proven Loss.
const TAG_LOSS: u8 = 2;

/// Records the exporter buffers before an explicit flush (~6 KiB ≈ one
/// syscall per 256 events; flushes are whole-record writes, so any torn tail
/// a reader sees is bounded and retried).
const FLUSH_RECORDS: usize = 256;

/// Admission filter (plan11 fixed constant, not a knob): the exporter emits
/// a record iff the record's root distance is at most this many plies. The
/// filter compares the real length; the u8 record field is a saturating
/// cast, which cannot change the filter outcome because the cap is far
/// below `u8::MAX`.
pub const PATH_LEN_CAP: u8 = 24;

/// Per-call import read cap across all files (~1 MiB ≈ 1 ms of I/O; keeps
/// the `FactImporter` hook's "must return promptly" contract).
pub const IMPORT_READ_CAP: u64 = 1024 * 1024;

/// Directory-rescan cadence: new `facts-*.bin` files are discovered at the
/// first hook call and then no more often than once per second of search
/// time (files are created at racer startup, so the first discovery already
/// sees the whole peer set in practice).
const RESCAN_INTERVAL: Duration = Duration::from_secs(1);

/// One solved fact: a proven Win/Loss at an exact Zobrist key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FactsRecord {
    /// Full Zobrist key (piece/side/castling/en-passant + halfmove clock).
    pub key: u64,
    /// Proven outcome (never `Draw` — draws are filtered at emission).
    pub outcome: Outcome,
    /// Proven depth, copied verbatim from the solved result.
    pub depth: u32,
    /// Plies from the racer's root (saturating cast of the real distance).
    pub path_len: u8,
}

impl FactsRecord {
    /// Encode into the fixed 24-byte record layout.
    #[must_use]
    pub fn encode(&self) -> [u8; RECORD_LEN] {
        let mut buf = [0u8; RECORD_LEN];
        buf[0..8].copy_from_slice(&self.key.to_le_bytes());
        buf[8] = match self.outcome {
            Outcome::Win => TAG_WIN,
            Outcome::Loss => TAG_LOSS,
            // Draws never enter the channel; encoding one is a contract
            // violation, not a data format.
            Outcome::Draw => unreachable!("draws never enter the facts channel"),
        };
        buf[9] = self.path_len;
        buf[12..16].copy_from_slice(&self.depth.to_le_bytes());
        let crc = crc32(&buf[0..16]);
        buf[16..20].copy_from_slice(&crc.to_le_bytes());
        buf
    }

    /// Decode and verify a record. Returns `None` on any checksum failure,
    /// unknown outcome tag, or (defensively) a Draw payload.
    #[must_use]
    pub fn decode(buf: &[u8; RECORD_LEN]) -> Option<Self> {
        let expected = u32::from_le_bytes(buf[16..20].try_into().ok()?);
        if crc32(&buf[0..16]) != expected {
            return None;
        }
        let outcome = match buf[8] {
            TAG_WIN => Outcome::Win,
            TAG_LOSS => Outcome::Loss,
            _ => return None,
        };
        Some(Self {
            key: u64::from_le_bytes(buf[0..8].try_into().ok()?),
            outcome,
            depth: u32::from_le_bytes(buf[12..16].try_into().ok()?),
            path_len: buf[9],
        })
    }
}

/// CRC-32 (ISO-HDLC, reflected, poly `0xEDB88320`) over the record payload.
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    const fn make_table() -> [u32; 256] {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut c = i as u32;
            let mut k = 0;
            while k < 8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
                k += 1;
            }
            table[i] = c;
            i += 1;
        }
        table
    }
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(make_table);
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// Shared export state: the process-owned buffered writer plus counters.
///
/// The search is single-threaded per process, so the `Mutex` is
/// uncontended bookkeeping (needed because the [`FactExporter`] hook takes
/// `&self`); it never blocks on I/O from another thread. A write failure
/// permanently disables the writer: a lost export is a lost optimization,
/// never a search failure.
pub struct FactsExportShared {
    /// Writer state; `None` once a write/flush failed (silent stop) or the
    /// final flush ran.
    writer: Mutex<Option<WriterState>>,
    records_seen: AtomicU64,
    emitted: AtomicU64,
    filtered: AtomicU64,
    bytes: AtomicU64,
}

struct WriterState {
    writer: std::io::BufWriter<std::fs::File>,
    since_flush: usize,
}

/// Snapshot of the export counters, for the CLI's pre-exit report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FactsExportStats {
    /// Decisive export calls seen (emitted + filtered + lost-to-I/O).
    pub records_seen: u64,
    /// Records appended to the file (post-filter).
    pub emitted: u64,
    /// Decisive calls dropped by the `PATH_LEN_CAP` admission filter.
    pub filtered: u64,
    /// Bytes appended (always `emitted * RECORD_LEN`).
    pub bytes: u64,
}

impl FactsExportShared {
    /// Create/truncate the export file and the shared writer state.
    ///
    /// # Errors
    ///
    /// Returns the I/O error if the file cannot be created.
    pub fn create(path: &Path) -> std::io::Result<Arc<Self>> {
        let file = std::fs::File::create(path)?;
        Ok(Arc::new(Self {
            writer: Mutex::new(Some(WriterState {
                writer: std::io::BufWriter::new(file),
                since_flush: 0,
            })),
            records_seen: AtomicU64::new(0),
            emitted: AtomicU64::new(0),
            filtered: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
        }))
    }

    /// Snapshot of the export counters.
    #[must_use]
    pub fn stats(&self) -> FactsExportStats {
        FactsExportStats {
            records_seen: self.records_seen.load(Ordering::Relaxed),
            emitted: self.emitted.load(Ordering::Relaxed),
            filtered: self.filtered.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
        }
    }

    /// Flush any buffered records and disable further writes (idempotent).
    /// Called at the CLI pre-exit; also runs on the last `Arc` drop.
    pub fn finish(&self) {
        let mut guard = match self.writer.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if let Some(state) = guard.as_mut() {
            let _ = state.writer.flush();
        }
        *guard = None;
    }

    fn export_record(&self, key: u64, outcome: Outcome, depth: u32, path_len: u8) {
        if outcome == Outcome::Draw {
            // Defense in depth on top of the emission sites: draws never
            // enter the channel (and are not counted — the sites never call
            // with a Draw).
            return;
        }
        self.records_seen.fetch_add(1, Ordering::Relaxed);
        if path_len > PATH_LEN_CAP {
            self.filtered.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let record = FactsRecord {
            key,
            outcome,
            depth,
            path_len,
        };
        let mut guard = match self.writer.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some(state) = guard.as_mut() else {
            // A previous write/flush failed: exporting is silently stopped
            // (a lost optimization, never a search failure).
            return;
        };
        if state.writer.write_all(&record.encode()).is_err() {
            *guard = None;
            return;
        }
        self.emitted.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(RECORD_LEN as u64, Ordering::Relaxed);
        state.since_flush += 1;
        if state.since_flush >= FLUSH_RECORDS {
            if state.writer.flush().is_err() {
                *guard = None;
                return;
            }
            state.since_flush = 0;
        }
    }
}

impl Drop for FactsExportShared {
    fn drop(&mut self) {
        self.finish();
    }
}

/// The search-side [`FactExporter`]: writes decisive solved facts to the
/// process-owned export file, synchronously on the search thread, filtered
/// by [`PATH_LEN_CAP`].
pub struct FactsExporter {
    shared: Arc<FactsExportShared>,
}

impl FactsExporter {
    /// Wrap a shared export state (kept by the CLI for the pre-exit report).
    #[must_use]
    pub fn new(shared: Arc<FactsExportShared>) -> Self {
        Self { shared }
    }
}

impl FactExporter for FactsExporter {
    fn export(&self, key: u64, outcome: Outcome, depth: u32, path_len: u8) {
        self.shared.export_record(key, outcome, depth, path_len);
    }
}

/// Snapshot of the import-side channel counters (the TT-side accepted /
/// skipped counts are the search's [`crate::search::dfpn::FactImportCounters`]
/// totals).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FactsImportStats {
    /// Checksum-verified records read from the sibling files.
    pub records_read: u64,
    /// Records dropped for checksum/tag failure.
    pub checksum_failures: u64,
    /// Bytes read from the sibling files (all calls, cumulative).
    pub bytes_read: u64,
    /// Distinct `facts-*.bin` files discovered in the directory.
    pub files_seen: u64,
}

#[derive(Default)]
pub struct ImportStats {
    records_read: AtomicU64,
    checksum_failures: AtomicU64,
    bytes_read: AtomicU64,
    files_seen: AtomicU64,
}

impl ImportStats {
    /// Snapshot of the import-side channel counters (for the CLI's
    /// pre-exit report).
    #[must_use]
    pub fn snapshot(&self) -> FactsImportStats {
        FactsImportStats {
            records_read: self.records_read.load(Ordering::Relaxed),
            checksum_failures: self.checksum_failures.load(Ordering::Relaxed),
            bytes_read: self.bytes_read.load(Ordering::Relaxed),
            files_seen: self.files_seen.load(Ordering::Relaxed),
        }
    }
}

/// The search-side importer (plan11): reads the sibling `facts-*.bin` files
/// directly, on the search thread, at the [`FactImporter`] hook calls
/// (work-chunk boundaries). No reader thread, no queue: per call, every
/// discovered file is stated and its new bytes are read incrementally at
/// persistent offsets, bounded by [`IMPORT_READ_CAP`] bytes across all
/// files; a torn tail is retried on the next call and a checksum failure
/// skips exactly one record stride.
pub struct FactsImporter {
    dir: PathBuf,
    /// Per-file read offsets (last complete record consumed).
    offsets: BTreeMap<PathBuf, u64>,
    discovered: bool,
    last_scan: Instant,
    read_cap: u64,
    rescan_interval: Duration,
    scratch: Vec<u8>,
    stats: Arc<ImportStats>,
}

impl FactsImporter {
    /// Build the importer for `dir` (the driver's per-race facts dir).
    /// Discovery is deferred to the first hook call, so the constructor
    /// cannot fail on I/O; unreadable directories simply yield nothing (a
    /// lost optimization, never unsound).
    #[must_use]
    pub fn new(dir: &Path) -> Self {
        Self::with_limits(dir, IMPORT_READ_CAP, RESCAN_INTERVAL)
    }

    /// Full constructor (test-only override of the fixed constants).
    #[must_use]
    pub fn with_limits(dir: &Path, read_cap: u64, rescan_interval: Duration) -> Self {
        Self {
            dir: dir.to_path_buf(),
            offsets: BTreeMap::new(),
            discovered: false,
            last_scan: Instant::now(),
            read_cap,
            rescan_interval,
            scratch: vec![0u8; read_cap as usize],
            stats: Arc::new(ImportStats::default()),
        }
    }

    /// Handle to the channel counters (keep in the CLI for the pre-exit
    /// report; the importer itself moves into the search).
    #[must_use]
    pub fn stats_handle(&self) -> Arc<ImportStats> {
        Arc::clone(&self.stats)
    }

    /// Snapshot of the channel counters.
    #[must_use]
    pub fn stats(&self) -> FactsImportStats {
        self.stats.snapshot()
    }

    fn discover(&mut self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if is_facts_file(&path) && !self.offsets.contains_key(&path) {
                self.offsets.insert(path, 0);
                self.stats.files_seen.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

impl FactImporter for FactsImporter {
    fn import(&mut self, tt: &mut TranspositionTable) -> crate::search::dfpn::FactImportCounters {
        // Discovery at the first hook call, then no more often than once per
        // rescan interval (files are created at racer startup, so the first
        // call usually sees the whole peer set).
        if !self.discovered || self.last_scan.elapsed() >= self.rescan_interval {
            self.discover();
            self.discovered = true;
            self.last_scan = Instant::now();
        }
        let mut counters = crate::search::dfpn::FactImportCounters::default();
        let mut budget = self.read_cap;
        let paths: Vec<PathBuf> = self.offsets.keys().cloned().collect();
        for path in paths {
            if budget == 0 {
                // The per-call read cap is spent; the remaining files (and
                // any unread bytes) continue at the next hook call — offsets
                // are persistent, so nothing is lost, only delayed.
                break;
            }
            let mut offset = self.offsets[&path];
            budget = self.tail_file(&path, &mut offset, budget, tt, &mut counters);
            self.offsets.insert(path, offset);
        }
        counters
    }
}

/// A file this racer should import from: the driver's naming convention
/// `facts-*.bin` inside the per-race facts dir.
fn is_facts_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.starts_with("facts-") && name.ends_with(".bin")
}

impl FactsImporter {
    /// Read newly appended bytes of one file, parse whole records, admit
    /// them into `tt`, and advance the offset past complete records only (a
    /// torn tail is retried on the next hook call; a checksum failure skips
    /// exactly one record stride). Returns the remaining read budget.
    fn tail_file(
        &mut self,
        path: &Path,
        offset: &mut u64,
        budget: u64,
        tt: &mut TranspositionTable,
        counters: &mut crate::search::dfpn::FactImportCounters,
    ) -> u64 {
        let Ok(mut file) = std::fs::File::open(path) else {
            return budget;
        };
        let Ok(len) = file.seek(SeekFrom::End(0)) else {
            return budget;
        };
        let mut pos = *offset;
        let mut remaining = budget;
        // Parsed records of the current chunk (`None` = checksum failure);
        // decoding happens while the scratch borrow is live, admission
        // after it ends.
        let mut parsed: Vec<Option<FactsRecord>> = Vec::new();
        while pos + RECORD_LEN as u64 <= len && remaining > 0 {
            let take = (len - pos).min(remaining).min(self.read_cap);
            let whole = take - take % RECORD_LEN as u64;
            if whole == 0 {
                break;
            }
            {
                let buf = &mut self.scratch[..whole as usize];
                if file.seek(SeekFrom::Start(pos)).is_err() {
                    return remaining;
                }
                if file.read_exact(buf).is_err() {
                    return remaining;
                }
                for chunk in buf.as_chunks::<RECORD_LEN>().0 {
                    parsed.push(FactsRecord::decode(chunk));
                }
            }
            self.stats.bytes_read.fetch_add(whole, Ordering::Relaxed);
            remaining -= whole;
            pos += whole;
            for decoded in parsed.drain(..) {
                match decoded {
                    Some(record) => {
                        self.stats.records_read.fetch_add(1, Ordering::Relaxed);
                        self.admit(record, tt, counters);
                    }
                    None => {
                        self.stats.checksum_failures.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
        *offset = pos;
        remaining
    }

    /// Idempotent admission: a key the TT already holds solved keeps its
    /// native entry (best-move hint, work accounting); only unsolved or
    /// missing keys take the imported fact — the same solved path a
    /// self-proven result takes (`remaining_depth = u32::MAX`, no move hint,
    /// `work = 0`: preferred eviction victim).
    fn admit(
        &self,
        record: FactsRecord,
        tt: &mut TranspositionTable,
        counters: &mut crate::search::dfpn::FactImportCounters,
    ) {
        if let Some(entry) = tt.probe(record.key)
            && entry.outcome.is_some()
        {
            counters.skipped += 1;
            return;
        }
        let (pn, dn) = record.outcome.to_pn_dn();
        tt.store(
            record.key,
            Move::NONE,
            u8::MAX,
            0,
            Some(record.outcome),
            pn,
            dn,
            record.depth,
            u32::MAX,
        );
        counters.accepted += 1;
    }
}

#[cfg(test)]
mod tests;
