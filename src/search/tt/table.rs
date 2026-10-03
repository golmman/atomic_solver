//! Transposition table storage and lookup.
//!
//! # Concurrency (inert prerequisite for the parallel prototype)
//!
//! The bucket array is sharded into contiguous power-of-two chunks, each
//! behind a [`std::sync::RwLock`]; every mutating operation takes `&self`
//! (interior mutability), so the table is `Send + Sync` by construction.
//! The sequential N=1 behavior is observably identical to the pre-sharding
//! table: the bucket index (`key & mask`), the store semantics (solved
//! overwrites, unsolved never downgrades a solved entry, `work` monotone,
//! k=2 replacement score), the generation policy, and the native bucket
//! order consumed by `for_each_entry` (the snapshot byte-identity contract)
//! are all unchanged.
//!
//! # Critical sections
//!
//! One bucket's read-modify-write stays inside one write-locked critical
//! section of the shard owning the bucket (`store`), so a concurrent
//! `probe` can never observe a torn or half-updated entry — `probe` copies
//! the matched entry under a read lock and returns it by value. Lock
//! poisoning propagates as a panic: a panic while holding a lock crashes
//! loudly instead of hiding a broken invariant.
//!
//! # Generation counter
//!
//! The generation counter is a single shared `AtomicU32` with the
//! *unchanged* sequential policy: `new_generation()` bumps exactly as
//! before; `clear()` resets it to 1. `Relaxed`-style atomicity is not
//! relied on beyond atomicity itself: all entry-content ordering flows
//! through the per-shard locks, and the loads/stores use `Acquire`/`Release`
//! for conservatism at no measurable cost.
//!
//! This file is larger than 10 KB because the store/probe hot path, the
//! parallel-only `refresh_bounds`/`bump_work` helpers, and the snapshot/stats
//! iterators share the shard layout and generation policy invariants.
//!
//! # Sequential-owner contract
//!
//! `clear()` and `new_generation()` are called between search phases by the
//! single owner thread in the sequential solver; calling them *while other
//! workers run* is not part of this table's contract (the parallel
//! prototype owns that policy and its documentation).

use std::sync::RwLock;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::position::Outcome;
use crate::zobrist::INF;
use atomic_movegen::types::Move;

use super::entry::TtEntry;
use super::shard::{Shard, layout};

pub struct TranspositionTable {
    shards: Vec<RwLock<Shard>>,
    buckets_per_shard: usize,
    shard_shift: u32,
    mask: usize,
    current_generation: AtomicU32,
}

// Static assertion: the table is safe to share across worker threads.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TranspositionTable>();
};

impl TranspositionTable {
    #[cfg(test)]
    pub(crate) fn bucket_count(&self) -> usize {
        self.shards.len() * self.buckets_per_shard
    }

    /// Construct a table with the given number of buckets (rounded up to the
    /// next power of two). This is `pub(crate)` so unit tests can force
    /// collisions and eviction deterministically.
    #[cfg(test)]
    pub(crate) fn with_capacity(buckets: usize) -> Self {
        let buckets = buckets.next_power_of_two().max(1);
        Self::with_bucket_count(buckets)
    }

    #[must_use]
    pub fn with_mb(mb: usize) -> Self {
        let bytes = mb.saturating_mul(1024 * 1024);
        let entries = (bytes / std::mem::size_of::<TtEntry>()).next_power_of_two();
        let entries = entries.max(32);
        let buckets = entries.max(2) / 2;
        Self::with_bucket_count(buckets)
    }

    fn with_bucket_count(bucket_count: usize) -> Self {
        let (shards, buckets_per_shard, shard_shift) = layout(bucket_count);
        Self {
            shards: (0..shards)
                .map(|_| RwLock::new(Shard::new(buckets_per_shard)))
                .collect(),
            buckets_per_shard,
            shard_shift,
            mask: bucket_count - 1,
            current_generation: AtomicU32::new(1),
        }
    }

    #[inline]
    fn index(&self, key: u64) -> usize {
        (key as usize) & self.mask
    }

    /// The generation counter at the time of the call.
    ///
    /// Used by snapshot/debug tooling to partition entries by generation.
    #[must_use]
    pub fn current_generation(&self) -> u32 {
        self.current_generation.load(Ordering::Acquire)
    }

    #[inline]
    #[must_use]
    pub fn probe(&self, key: u64) -> Option<TtEntry> {
        let generation = self.current_generation.load(Ordering::Acquire);
        let idx = self.index(key);
        // SAFETY: `index()` masks with `bucket_count - 1`, so the shard index
        // and the local bucket index are in bounds by construction.
        let shard = unsafe { self.shards.get_unchecked(idx >> self.shard_shift) }
            .read()
            .unwrap();
        unsafe {
            shard
                .buckets
                .get_unchecked(idx & (self.buckets_per_shard - 1))
        }
        .iter()
        .find(|e| e.valid && e.key == key && e.generation == generation)
        .copied()
    }

    pub fn clear(&self) {
        for shard in &self.shards {
            let mut shard = shard.write().expect("tt shard lock poisoned");
            for bucket in &mut shard.buckets {
                *bucket = [TtEntry::default(); 2];
            }
        }
        self.current_generation.store(1, Ordering::Release);
    }

    /// Mark every existing table entry as belonging to an older generation.
    pub fn new_generation(&self) {
        let previous = self.current_generation.fetch_add(1, Ordering::AcqRel);
        if previous == u32::MAX {
            self.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn store(
        &self,
        key: u64,
        best_move: Move,
        best_child: u8,
        work: u64,
        outcome: Option<Outcome>,
        pn: u64,
        dn: u64,
        depth: u32,
        remaining_depth: u32,
    ) {
        let mut pn = pn.min(INF);
        let mut dn = dn.min(INF);
        if outcome.is_none() && pn == INF && dn == INF {
            pn = 1;
            dn = 1;
        }

        let generation = self.current_generation.load(Ordering::Acquire);
        let idx = self.index(key);
        let mut existing = false;

        {
            // The bucket's read-modify-write stays inside one write-locked
            // critical section (shared-TT discipline at the per-shard level).
            let mut shard = self.shards[idx >> self.shard_shift]
                .write()
                .expect("tt shard lock poisoned");
            let bucket = &mut shard.buckets[idx & (self.buckets_per_shard - 1)];
            for slot in bucket.iter_mut() {
                if slot.valid && slot.key == key && slot.generation == generation {
                    existing = true;
                    slot.generation = generation;
                    slot.work = slot.work.max(work);
                    if let Some(o) = outcome {
                        // Solved, path-independent result: overwrite the base entry.
                        slot.best_move = best_move;
                        slot.best_child = best_child;
                        slot.outcome = Some(o);
                        slot.pn = pn;
                        slot.dn = dn;
                        slot.depth = depth;
                        slot.remaining_depth = remaining_depth;
                    } else if slot.outcome.is_none() {
                        // Unsolved node: update base bounds.
                        slot.best_move = best_move;
                        slot.best_child = best_child;
                        slot.outcome = None;
                        slot.pn = pn;
                        slot.dn = dn;
                        slot.depth = depth;
                        slot.remaining_depth = remaining_depth;
                    } else {
                        // Do not overwrite a solved base entry with unsolved bounds.
                    }
                    break;
                }
            }
            if !existing {
                let new = TtEntry {
                    key,
                    valid: true,
                    generation,
                    best_move,
                    best_child,
                    work,
                    outcome,
                    pn,
                    dn,
                    depth,
                    remaining_depth,
                };
                insert_new(bucket, generation, new);
            }
        }
    }

    /// Raise the stored `work` of a live current-generation entry (parallel
    /// SPDFPN only). Returns `false` when no live entry exists (the caller
    /// then inserts a fresh unsolved marker via `store`).
    pub fn bump_work(&self, key: u64, work: u64) -> bool {
        let generation = self.current_generation.load(Ordering::Acquire);
        let idx = self.index(key);
        let mut shard = self.shards[idx >> self.shard_shift]
            .write()
            .expect("tt shard lock poisoned");
        let bucket = &mut shard.buckets[idx & (self.buckets_per_shard - 1)];
        for slot in bucket.iter_mut() {
            if slot.valid && slot.key == key && slot.generation == generation {
                slot.work = slot.work.max(work);
                return true;
            }
        }
        false
    }

    /// Refresh the unsolved bounds of a live current-generation entry
    /// (parallel SPDFPN re-propagation only; never called on the sequential
    /// path, whose store semantics are untouched). Updates `pn`, `dn`,
    /// `best_move`, and `best_child` of an unsolved entry in one write-locked
    /// critical section; solved entries are never modified, missing entries
    /// are not inserted (a refresh never creates state), and
    /// `depth`/`remaining_depth`/`work` are left untouched so the entry's
    /// existing validity guards keep applying at the probe sites.
    pub fn refresh_bounds(&self, key: u64, best_move: Move, best_child: u8, pn: u64, dn: u64) {
        let pn = pn.clamp(1, INF);
        let dn = dn.clamp(1, INF);
        let generation = self.current_generation.load(Ordering::Acquire);
        let idx = self.index(key);
        let mut shard = self.shards[idx >> self.shard_shift]
            .write()
            .expect("tt shard lock poisoned");
        let bucket = &mut shard.buckets[idx & (self.buckets_per_shard - 1)];
        for slot in bucket.iter_mut() {
            if slot.valid
                && slot.key == key
                && slot.generation == generation
                && slot.outcome.is_none()
            {
                slot.pn = pn;
                slot.dn = dn;
                slot.best_move = best_move;
                slot.best_child = best_child;
                break;
            }
        }
    }

    /// Iterate over all `valid` entries in native bucket order, across all
    /// generations, locking one shard at a time in index order.
    ///
    /// This is for snapshot/debug use; the search itself only ever looks at
    /// entries via `probe` (current generation). Callers that need
    /// generation-dependent semantics must filter by `current_generation()`
    /// themselves.
    ///
    /// `f` must not call back into the table (lock re-entrancy is not
    /// supported: a `store` on the same shard inside `f` would deadlock).
    pub fn for_each_entry(&self, mut f: impl FnMut(&TtEntry)) {
        for shard in &self.shards {
            let shard = shard.read().expect("tt shard lock poisoned");
            for bucket in &shard.buckets {
                for entry in bucket {
                    if entry.valid {
                        f(entry);
                    }
                }
            }
        }
    }

    /// Return a distribution of stored `best_child` values among live entries.
    ///
    /// `u8::MAX` (the "unknown" sentinel) is excluded. This is useful for
    /// debugging proof-tree and GHI path-code usage.
    #[must_use]
    pub fn best_child_counts(&self) -> Vec<(u8, usize)> {
        let generation = self.current_generation();
        let mut counts = std::collections::HashMap::new();
        for_each_bucket(&self.shards, |bucket| {
            for entry in bucket {
                if entry.valid && entry.generation == generation && entry.best_child != u8::MAX {
                    *counts.entry(entry.best_child).or_insert(0) += 1;
                }
            }
        });
        let mut v: Vec<_> = counts.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v
    }

    /// Return aggregate statistics about the current transposition table contents.
    ///
    /// Tuple fields are: `(buckets, live_entries, solved_entries, unsolved_entries, generation)`.
    #[must_use]
    pub fn stats(&self) -> (usize, usize, usize, usize, u32) {
        let generation = self.current_generation();
        let mut live = 0;
        let mut solved = 0;
        for_each_bucket(&self.shards, |bucket| {
            for entry in bucket {
                if entry.valid && entry.generation == generation {
                    live += 1;
                    if entry.outcome.is_some() {
                        solved += 1;
                    }
                }
            }
        });
        let unsolved = live - solved;
        (
            self.shards.len() * self.buckets_per_shard,
            live,
            solved,
            unsolved,
            generation,
        )
    }
}

/// Lock each shard's buckets in index order and apply `f` (order-independent
/// aggregate helper for `stats`/`best_child_counts`).
fn for_each_bucket(shards: &[RwLock<Shard>], mut f: impl FnMut(&[TtEntry; 2])) {
    for shard in shards {
        let shard = shard.read().expect("tt shard lock poisoned");
        for bucket in &shard.buckets {
            f(bucket);
        }
    }
}

/// Place a new entry into `bucket`, preferring the two most valuable entries.
/// Called with the owning shard write-locked; `generation` is the value the
/// caller read for this store.
fn insert_new(bucket: &mut [TtEntry; 2], generation: u32, new: TtEntry) {
    let score = |e: &TtEntry| {
        let live = e.valid && e.generation == generation;
        let solved = e.outcome.is_some();
        (u8::from(live), u8::from(solved), e.work, e.generation)
    };

    if !bucket[0].valid || bucket[0].generation != generation {
        bucket[0] = new;
    } else if !bucket[1].valid || bucket[1].generation != generation {
        bucket[1] = new;
    } else {
        let evict = usize::from(score(&bucket[0]) >= score(&bucket[1]));
        bucket[evict] = new;
    }
}
