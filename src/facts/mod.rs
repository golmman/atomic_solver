//! Cross-racer solved-fact channel (plan10, `parallel` backlog #6).
//!
//! A cooperative portfolio racer exports every *solved* Win/Loss fact it
//! proves to an append-only binary file (`--facts-out <file>`) and imports
//! the sibling racers' facts into its transposition table
//! (`--facts-in <dir>`). The channel carries exactly the TT's path-independent solved
//! class: [`ProofEvent::NodeProven`] events with a decisive outcome (Draws
//! never emit — see [`Search::emit_proof_node`]), keyed by the full Zobrist
//! hash (piece/side/castling/en-passant **plus the halfmove clock**), so a
//! fact is valid exactly at the (position, clock) it was proven at — the
//! report10 cross-clock reuse channel is closed by the key.
//!
//! # Record format (24 B fixed)
//!
//! ```text
//! bytes 0..8   key      u64 LE  (full Zobrist key incl. halfmove clock)
//! byte  8      outcome  u8      (1 = Win, 2 = Loss; nothing else is encoded)
//! bytes 9..12  padding  zero
//! bytes 12..16 depth    u32 LE
//! bytes 16..20 crc32    u32 LE  (CRC-32/ISO-HDLC over bytes 0..16)
//! bytes 20..24 padding  zero
//! ```
//!
//! **Checksum-verified or dropped**: a record whose CRC or outcome tag
//! fails verification is skipped, never misread (a flipped outcome byte
//! would be a false-fact injection; the CRC closes that channel). A torn
//! tail (fewer than 24 bytes available) is left for the next poll — the
//! writer only ever appends whole records (its flushes are record-aligned),
//! so file offsets stay record-aligned.
//!
//! # Threads
//!
//! One writer thread per exporter: drains the `ProofEvent` channel
//! (fire-and-forget from the search side; unbounded channel, no
//! backpressure) and appends records, flushing every `FLUSH_RECORDS`
//! records and at channel disconnect. One reader thread per importer: tails
//! every `facts-*.bin` file in the facts dir at incremental offsets (never
//! re-read), verifies checksums, and pushes verified records into a bounded
//! pending queue (~`MAX_PENDING_RECORDS` records ≈ 1 MiB). The search
//! drains the queue **on the search thread** between work chunks via the
//! [`crate::search::dfpn::FactImporter`] hook — this module never touches
//! the search itself.
//!
//! # Soundness contract (plan10)
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
//! - Admission rule A1 = import-all (every checksum-verified Win/Loss
//!   record, no depth floor, no direction filter). The pre-registered A2
//!   escalation (Loss-only import) is selected by the environment variable
//!   `ATOMIC_SOLVER_FACTS_LOSS_ONLY=1` at construction — deliberately not a
//!   CLI option, so the product surface stays exactly the two facts flags.
//! - RAM with the facts options on = TT + the bounded buffers below
//!   (~2 MiB); without them, RAM = TT only.
//!
//! Files under `docs/plans/parallel/plan10.md` carry the pre-registered
//! measurement contract; nothing here is measured-adaptive.
//!
//! File size: above the 10 KiB guideline because the record format, its
//! alignment/CRC invariants, the writer's flush discipline, the reader's
//! incremental-offset tailing, and the bounded queue the importer drains
//! share one set of soundness invariants (checksum-or-drop, record
//! alignment, Draw exclusion); splitting them would scatter those
//! invariants across files. Unit tests are split out (`tests.rs`).

use std::collections::{BTreeMap, VecDeque};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use atomic_movegen::types::Move;

use crate::position::Outcome;
use crate::proof_event::ProofEvent;
use crate::search::dfpn::{FactImportCounters, FactImporter};
use crate::search::tt::TranspositionTable;

/// Fixed record length in bytes.
pub const RECORD_LEN: usize = 24;

/// Outcome tag for a proven Win.
const TAG_WIN: u8 = 1;
/// Outcome tag for a proven Loss.
const TAG_LOSS: u8 = 2;

/// Records the writer buffers before an explicit flush (keeps the reader's
/// poll latency bounded; flushes are whole-buffer, so record alignment is
/// preserved).
const FLUSH_RECORDS: usize = 256;

/// Pending-queue bound (~1 MiB of records): the reader blocks when full
/// instead of growing without limit (the documented RAM restatement).
const MAX_PENDING_RECORDS: usize = (1 << 20) / RECORD_LEN;

/// Reader poll interval: file discovery + tail pass cadence.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(2000);

/// Per-iteration tail cap per file, so one fast exporter cannot starve the
/// others (the loop continues at the next poll).
const TAIL_CHUNK_BYTES: u64 = 256 * 1024;

/// One solved fact: a proven Win/Loss at an exact Zobrist key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FactsRecord {
    /// Full Zobrist key (piece/side/castling/en-passant + halfmove clock).
    pub key: u64,
    /// Proven outcome (never `Draw` — draws are filtered at emission).
    pub outcome: Outcome,
    /// Proven depth, copied verbatim from the `NodeProven` event.
    pub depth: u32,
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

/// Export statistics reported by the writer thread at channel disconnect.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FactsExportStats {
    /// Decisive records appended (duplicates included — no exporter dedup;
    /// re-delivery is the mechanism for facts evicted from a consumer TT).
    pub records: u64,
    /// Bytes appended (always `records * RECORD_LEN`).
    pub bytes: u64,
}

/// Spawn the export writer: appends one record per decisive `NodeProven`
/// event to `path` (created/truncated). Fire-and-forget on the search side:
/// send errors are ignored by the search, and channel disconnect (the
/// `Search` being dropped) flushes and finishes the thread.
///
/// # Errors
///
/// Returns the I/O error if the file cannot be created.
pub fn spawn_writer(
    path: &Path,
    rx: std::sync::mpsc::Receiver<ProofEvent>,
) -> std::io::Result<JoinHandle<FactsExportStats>> {
    let file = std::fs::File::create(path)?;
    let handle = std::thread::Builder::new()
        .name("facts-writer".into())
        .spawn(move || {
            let mut stats = FactsExportStats::default();
            let mut buf = std::io::BufWriter::new(file);
            let mut since_flush = 0usize;
            for event in rx.iter() {
                let ProofEvent::NodeProven(proven) = event else {
                    continue;
                };
                if proven.outcome == Outcome::Draw {
                    // Defense in depth on top of `emit_proof_node`: draws
                    // never enter the channel.
                    continue;
                }
                let record = FactsRecord {
                    key: proven.hash,
                    outcome: proven.outcome,
                    depth: proven.depth,
                };
                if buf.write_all(&record.encode()).is_err() {
                    // A failed export is a lost optimization, never a search
                    // failure; stop writing (the search never observes this).
                    break;
                }
                stats.records += 1;
                stats.bytes += RECORD_LEN as u64;
                since_flush += 1;
                if since_flush >= FLUSH_RECORDS {
                    if buf.flush().is_err() {
                        break;
                    }
                    since_flush = 0;
                }
            }
            let _ = buf.flush();
            stats
        })?;
    Ok(handle)
}

/// Shared importer state: reader-thread counters + the bounded pending
/// queue the search thread drains.
pub struct FactsShared {
    queue: Mutex<VecDeque<FactsRecord>>,
    space_available: Condvar,
    /// Records verified and queued (cumulative).
    records_read: AtomicU64,
    /// Records dropped for checksum/tag failure (cumulative).
    checksum_failures: AtomicU64,
    /// Records accepted into the TT (cumulative, drain-side).
    accepted: AtomicU64,
    /// Records skipped because the TT already held a solved entry at the key
    /// (cumulative, drain-side).
    skipped_solved: AtomicU64,
    /// Records skipped by the admission rule (A2 Loss-only; cumulative).
    skipped_filtered: AtomicU64,
    /// Facts files seen in the directory.
    files_seen: AtomicU64,
    /// Stop flag for the reader thread (set by the CLI at pre-exit).
    stop_flag: AtomicBool,
}

impl FactsShared {
    /// Tell the reader thread to stop; it exits at its next poll (the
    /// process exit reaps it).
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Release);
        // Wake a reader blocked on the queue bound so it can observe `stop`.
        let _guard = self.queue.lock().expect("facts queue poisoned");
        self.space_available.notify_all();
    }

    /// Snapshot of the channel counters.
    #[must_use]
    pub fn snapshot(&self) -> FactsChannelStats {
        FactsChannelStats {
            records_read: self.records_read.load(Ordering::Relaxed),
            checksum_failures: self.checksum_failures.load(Ordering::Relaxed),
            accepted: self.accepted.load(Ordering::Relaxed),
            skipped_solved: self.skipped_solved.load(Ordering::Relaxed),
            skipped_filtered: self.skipped_filtered.load(Ordering::Relaxed),
            files_seen: self.files_seen.load(Ordering::Relaxed),
            pending: self.queue.lock().map(|q| q.len()).unwrap_or(0),
        }
    }

    fn push(&self, record: FactsRecord) {
        let mut queue = self.queue.lock().expect("facts queue poisoned");
        loop {
            if self.stop_flag.load(Ordering::Acquire) {
                return;
            }
            if queue.len() < MAX_PENDING_RECORDS {
                queue.push_back(record);
                return;
            }
            // Bounded backpressure: wait for the search thread to drain.
            let (guard, _timeout) = self
                .space_available
                .wait_timeout(queue, POLL_INTERVAL)
                .expect("facts queue poisoned");
            queue = guard;
        }
    }

    fn drain(&self) -> VecDeque<FactsRecord> {
        let mut queue = self.queue.lock().expect("facts queue poisoned");
        std::mem::take(&mut *queue)
    }
}

/// Snapshot of the facts-channel counters, for the CLI's pre-exit report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FactsChannelStats {
    /// Checksum-verified records delivered by the reader thread.
    pub records_read: u64,
    /// Records dropped for checksum/tag failure.
    pub checksum_failures: u64,
    /// Records stored into the TT.
    pub accepted: u64,
    /// Records skipped: the TT already held a solved entry at that key.
    pub skipped_solved: u64,
    /// Records skipped by the admission rule (A2 Loss-only).
    pub skipped_filtered: u64,
    /// Distinct `facts-*.bin` files discovered in the directory.
    pub files_seen: u64,
    /// Records currently pending (queue length at snapshot time).
    pub pending: usize,
}

/// The search-side importer: drains the bounded pending queue into the TT
/// between work chunks (see the [`FactImporter`] contract in `search::dfpn`).
pub struct FactsImporter {
    shared: Arc<FactsShared>,
    /// A2 escalation arm: import Loss records only.
    loss_only: bool,
}

impl FactsImporter {
    /// Spawn the reader thread tailing `dir`'s `facts-*.bin` files.
    ///
    /// Returns the importer (move into the search) plus the shared stats
    /// handle (keep in the CLI for the pre-exit report). The A2 admission
    /// arm is selected by `ATOMIC_SOLVER_FACTS_LOSS_ONLY=1` in the
    /// environment (see the module docs).
    ///
    /// # Errors
    ///
    /// Returns the I/O error if `dir` cannot be read.
    pub fn spawn(dir: &Path) -> std::io::Result<(Self, Arc<FactsShared>)> {
        let loss_only = std::env::var("ATOMIC_SOLVER_FACTS_LOSS_ONLY")
            .map(|v| v == "1")
            .unwrap_or(false);
        let shared = Arc::new(FactsShared {
            queue: Mutex::new(VecDeque::new()),
            space_available: Condvar::new(),
            records_read: AtomicU64::new(0),
            checksum_failures: AtomicU64::new(0),
            accepted: AtomicU64::new(0),
            skipped_solved: AtomicU64::new(0),
            skipped_filtered: AtomicU64::new(0),
            files_seen: AtomicU64::new(0),
            stop_flag: AtomicBool::new(false),
        });
        let reader_shared = Arc::clone(&shared);
        let dir = dir.to_path_buf();
        std::thread::Builder::new()
            .name("facts-reader".into())
            .spawn(move || reader_loop(dir, reader_shared))?;
        Ok((
            Self {
                shared: Arc::clone(&shared),
                loss_only,
            },
            shared,
        ))
    }

    /// Snapshot of the channel counters.
    #[must_use]
    pub fn stats(&self) -> FactsChannelStats {
        self.shared.snapshot()
    }
}

impl FactImporter for FactsImporter {
    fn import(&mut self, tt: &mut TranspositionTable) -> FactImportCounters {
        let pending = self.shared.drain();
        let mut counters = FactImportCounters::default();
        for record in pending {
            if self.loss_only && record.outcome != Outcome::Loss {
                self.shared.skipped_filtered.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            // Idempotent admission: a key the TT already holds solved keeps
            // its native entry (best-move hint, work accounting); only
            // unsolved or missing keys take the imported fact.
            if let Some(entry) = tt.probe(record.key)
                && entry.outcome.is_some()
            {
                self.shared.skipped_solved.fetch_add(1, Ordering::Relaxed);
                counters.skipped += 1;
                continue;
            }
            // The same solved path a self-proven result takes: solved class,
            // `remaining_depth = u32::MAX` (reusable at every bound ≥ depth),
            // no move hint, `work = 0` (preferred eviction victim).
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
            self.shared.accepted.fetch_add(1, Ordering::Relaxed);
            counters.accepted += 1;
        }
        counters
    }
}

impl Drop for FactsImporter {
    fn drop(&mut self) {
        self.shared.stop();
    }
}

/// Reader-thread loop: discover `facts-*.bin` files, tail each at its
/// incremental offset, verify records, queue them.
fn reader_loop(dir: PathBuf, shared: Arc<FactsShared>) {
    let mut offsets: BTreeMap<PathBuf, u64> = BTreeMap::new();
    let mut last_scan = std::time::Instant::now() - POLL_INTERVAL;
    let mut scratch = vec![0u8; TAIL_CHUNK_BYTES as usize];
    loop {
        if shared.stop_flag.load(Ordering::Acquire) {
            return;
        }
        // Discover new files at the poll cadence; tail known files every
        // pass (cheap stat, and the tail itself is capped per iteration).
        if last_scan.elapsed() >= POLL_INTERVAL {
            last_scan = std::time::Instant::now();
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if is_facts_file(&path) && !offsets.contains_key(&path) {
                        offsets.insert(path, 0);
                        shared.files_seen.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
        for (path, offset) in offsets.iter_mut() {
            tail_file(path, offset, &shared, &mut scratch);
        }
        if shared.stop_flag.load(Ordering::Acquire) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
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

/// Read newly appended bytes of one file, parse whole records, advance the
/// offset past complete records only (a torn tail is retried on the next
/// poll; a checksum failure skips exactly one record-length stride).
fn tail_file(path: &Path, offset: &mut u64, shared: &Arc<FactsShared>, scratch: &mut [u8]) {
    let Ok(mut file) = std::fs::File::open(path) else {
        return;
    };
    let Ok(len) = file.seek(SeekFrom::End(0)) else {
        return;
    };
    let mut pos = *offset;
    while pos + RECORD_LEN as u64 <= len {
        let available = (len - pos).min(TAIL_CHUNK_BYTES);
        let whole = available - available % RECORD_LEN as u64;
        if whole == 0 {
            break;
        }
        if file.seek(SeekFrom::Start(pos)).is_err() {
            return;
        }
        let buf = &mut scratch[..whole as usize];
        if file.read_exact(buf).is_err() {
            return;
        }
        for chunk in buf.as_chunks::<RECORD_LEN>().0 {
            let record = FactsRecord::decode(chunk);
            match record {
                Some(record) => {
                    shared.records_read.fetch_add(1, Ordering::Relaxed);
                    shared.push(record);
                }
                None => {
                    shared.checksum_failures.fetch_add(1, Ordering::Relaxed);
                }
            }
            pos += RECORD_LEN as u64;
        }
    }
    *offset = pos;
}

#[cfg(test)]
mod tests;
