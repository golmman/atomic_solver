// Preserved record of plan4's `src/search/tt/shard.rs` (the sharded-TT
// attempt was reverted after the drift-gate kill; `git diff` cannot capture
// untracked files, so this sibling file completes
// `sharded_tt_attempt.patch`). Do not compile from here; see README.md.

//! Per-shard storage for the [`TranspositionTable`](super::TranspositionTable).
//!
//! # Shard/index invariant (pairing)
//!
//! The table's bucket array is split into contiguous, equally sized shard
//! ranges: shard `s` owns buckets `[s * buckets_per_shard,
//! (s + 1) * buckets_per_shard)`. [`super::TranspositionTable::index`]
//! (`key & mask` over the *total* bucket count) is the single mapping from
//! key to bucket; a key's shard is derived from its bucket index, never
//! from key bits. This keeps the probe path (lock the bucket's shard →
//! scan the bucket) correct without a second mapping, and preserves the
//! native bucket order that [`for_each_entry`](super::TranspositionTable::for_each_entry)
//! and the snapshot dump byte-identity contract depend on.

use super::entry::TtEntry;

/// Upper bound on the shard count. 256 extra lock words + `Vec` headers are
/// noise against the search CLI's "RAM = TT only" contract (the 128 MB
/// default TT keeps ~4 k buckets per shard).
pub(crate) const MAX_SHARDS: usize = 256;

/// One shard: a contiguous chunk of two-slot buckets, owned by exactly one
/// `RwLock` in the parent table. A bucket's read-modify-write must stay
/// inside one write-locked critical section of the shard that owns it.
pub(crate) struct Shard {
    pub(crate) buckets: Vec<[TtEntry; 2]>,
}

impl Shard {
    pub(crate) fn new(buckets: usize) -> Self {
        Self {
            buckets: vec![[TtEntry::default(); 2]; buckets],
        }
    }
}

/// Compute the shard layout for a table with `bucket_count` buckets
/// (both constructors keep it a power of two). Returns
/// `(shards, buckets_per_shard, shard_shift)` with
/// `shards * buckets_per_shard == bucket_count` and
/// `shards = min(bucket_count, MAX_SHARDS)` (a power of two).
#[must_use]
pub(crate) fn layout(bucket_count: usize) -> (usize, usize, u32) {
    debug_assert!(bucket_count.is_power_of_two(), "bucket count must be a power of two");
    let shards = bucket_count.min(MAX_SHARDS);
    debug_assert!(shards.is_power_of_two());
    let buckets_per_shard = bucket_count / shards;
    debug_assert!(shards * buckets_per_shard == bucket_count);
    (shards, buckets_per_shard, buckets_per_shard.trailing_zeros())
}
