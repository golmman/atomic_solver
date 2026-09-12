//! Per-search cache of repetition-dependent draw results (`docs/plans/dfpn/
//! plan9.md`).
//!
//! The first-player-loss shortcut (report7) stores repetition-dependent draws
//! as unsolved `(1, 1)` TT entries, so the whole chain of draw proofs above a
//! repetition is uncacheable in the transposition table: every re-entry within
//! a run re-walks the region down to the repetition leaves (96% redundancy on
//! the stress case, see `docs/plans/dfpn/research_repetition_cache.md` §2).
//!
//! This cache is the fix scoped by that research (§3–§5 are normative):
//! under the solver's repetition semantics, a node's value is a deterministic
//! function `f(P, A)` of the position hash `P` (including rule50) and the
//! *set* of ancestor repetition keys `A`. Only `Outcome::Draw` results whose
//! proof was repetition-dependent are stored, keyed by exactly `(P, A)`:
//!
//! 1. Only draws enter and leave the cache — wins/losses never do. A cached
//!    draw can therefore never manufacture a false decisive outcome; the
//!    worst possible defect is a false draw.
//! 2. A hit requires the full ancestor set to match (order-independent), so
//!    no cross-context reuse exists — the plan5 false-win failure chain is
//!    excluded by construction.
//! 3. A hit is a semantic identity, not a guess: on an exact key match the
//!    reuse returns precisely the value the search would recompute.
//! 4. Entries are cleared once per run in `Search::begin_run` and are never
//!    serialized into TT snapshots, proof events, or any other artifact.

use std::collections::HashMap;

/// Order-independent mixing of one repetition key for the context hash.
///
/// The same SplitMix64 finalizer that generates the Zobrist keys: a bijection
/// on `u64`, so raw-key cancellation patterns do not survive mixing. The
/// context hash is the wrapping *sum* of the mixed keys — a multiset hash,
/// order-independent by construction (required for transposition reuse: the
/// same board reached via permuted move sequences has an equal ancestor set),
/// and still correct if the no-duplicate path-stack invariant ever changed.
pub(super) fn mix_key(key: u64) -> u64 {
    crate::zobrist::mix(key)
}

/// Cache of repetition-dependent draw proofs for one search run.
///
/// Key: `(tt_key, context_hash)` where `tt_key` is the full position hash
/// (including the rule50 component — boards sharing a repetition key can
/// differ in clock and value) and `context_hash` is the order-independent
/// multiset hash of the repetition keys strictly above the node. Payload: the
/// proven depth only; the outcome is `Draw` by contract.
pub(super) struct RepetitionCache {
    map: HashMap<(u64, u64), u32>,
    /// Hard entry bound. When full, new inserts are dropped — deterministic
    /// no-eviction semantics (no randomness, no churn). The stress case's
    /// measured working set (see report9) is far below the default, so the
    /// cap only guards pathological memory growth.
    capacity: usize,
    /// Probe hits since the last clear. Test instrumentation only (`#[cfg(test)]`):
    /// lets the dfpn unit tests prove that a second bounded search within one
    /// run actually reuses cached entries.
    #[cfg(test)]
    pub(super) hits: u64,
}

impl RepetitionCache {
    /// Default capacity: `1 << 18` entries (~8 MiB worst case at ~32 bytes
    /// per entry). The stress case's measured working set is 1,643 entries
    /// (report9) — two orders of magnitude below this bound — so the cap
    /// never binds there while keeping the per-search cache a small fraction
    /// of the default 128 MiB TT. (`1 << 20` was the plan9 starting point,
    /// shrunk after the measurement.)
    const DEFAULT_CAPACITY: usize = 1 << 18;

    pub(super) fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity.min(4096)),
            capacity,
            #[cfg(test)]
            hits: 0,
        }
    }

    /// Order-independent context hash of an ancestor repetition-key set.
    pub(super) fn context_hash(keys: &[u64]) -> u64 {
        keys.iter().fold(0, |acc, &k| acc.wrapping_add(mix_key(k)))
    }

    /// Probe a proven repetition-dependent draw; `Some(depth)` on a hit.
    pub(super) fn probe(&mut self, tt_key: u64, context_hash: u64) -> Option<u32> {
        let hit = self.map.get(&(tt_key, context_hash)).copied();
        #[cfg(test)]
        if hit.is_some() {
            self.hits += 1;
        }
        hit
    }

    /// Store a proven repetition-dependent draw at depth `depth`.
    ///
    /// Deterministic when full: the insert is dropped, no eviction.
    pub(super) fn store(&mut self, tt_key: u64, context_hash: u64, depth: u32) {
        if self.map.len() >= self.capacity {
            return;
        }
        self.map.insert((tt_key, context_hash), depth);
    }

    /// Drop all entries. Called once per run in `Search::begin_run` — never
    /// per work chunk, never per refinement round (within-run reuse across
    /// chunks is a main benefit).
    pub(super) fn clear(&mut self) {
        self.map.clear();
        #[cfg(test)]
        {
            self.hits = 0;
        }
    }

    /// Number of live entries (never decreases within a run: no eviction).
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.map.len()
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;
    use atomic_movegen::types::{Move, Square};

    #[test]
    fn insert_probe_round_trip() {
        let mut cache = RepetitionCache::with_capacity(16);
        assert_eq!(cache.probe(0x1234, 0xabcd), None);

        cache.store(0x1234, 0xabcd, 7);
        assert_eq!(cache.probe(0x1234, 0xabcd), Some(7));

        // Different position or different context must miss.
        assert_eq!(cache.probe(0x1235, 0xabcd), None);
        assert_eq!(cache.probe(0x1234, 0xabce), None);

        // Overwrite updates the depth.
        cache.store(0x1234, 0xabcd, 3);
        assert_eq!(cache.probe(0x1234, 0xabcd), Some(3));
    }

    #[test]
    fn capacity_stop_deterministic() {
        let mut cache = RepetitionCache::with_capacity(2);
        cache.store(1, 0, 10);
        cache.store(2, 0, 20);
        assert_eq!(cache.len(), 2);
        // Third insert is dropped (no eviction), first two stay resolvable.
        cache.store(3, 0, 30);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.probe(1, 0), Some(10));
        assert_eq!(cache.probe(2, 0), Some(20));
        assert_eq!(cache.probe(3, 0), None);
    }

    #[test]
    fn context_hash_is_order_independent() {
        let keys = [0xdead_beef, 0x1234_5678, 0x0f0f_0f0f, 42];
        let a = RepetitionCache::context_hash(&keys);
        let b = RepetitionCache::context_hash(&[keys[2], keys[0], keys[3], keys[1]]);
        let c = RepetitionCache::context_hash(&[keys[3], keys[2], keys[1], keys[0]]);
        assert_eq!(a, b, "same keys, different order must give the same hash");
        assert_eq!(a, c);
        // The empty ancestor set hashes to 0.
        assert_eq!(RepetitionCache::context_hash(&[]), 0);
        // Distinct sets must differ (a wrapping sum of mixed keys; equal
        // hashes would require mix(k) == 0 for some key).
        assert_ne!(
            RepetitionCache::context_hash(&keys[..1]),
            RepetitionCache::context_hash(&keys[..2])
        );
        // Raw keys are not XORed: the sum must not cancel on repetition of a
        // mixed value in a multiset (sum is multiset-safe even with
        // duplicates, which the path stack never produces today).
        let d = RepetitionCache::context_hash(&[keys[0], keys[0]]);
        let e = RepetitionCache::context_hash(&[keys[0]]);
        assert_ne!(d, e);
    }

    #[test]
    fn distinct_clock_boards_map_to_distinct_entries() {
        // A reversible rook/king shuffle returns the same board with a
        // different rule50 counter: the repetition key stays equal while the
        // full hash changes (the hash is the node identity, per
        // research_repetition_cache.md §5).
        let start = Position::from_fen("8/8/8/8/2k5/8/8/4KR2 w - - 0 1").unwrap();
        let mut pos = start.clone();
        for &(from, to) in &[
            (Square::F1, Square::G1),
            (Square::C4, Square::B4),
            (Square::G1, Square::F1),
            (Square::B4, Square::C4),
        ] {
            pos.do_move(Move::make_move(from, to));
        }
        let hash0 = start.hash();
        let hash1 = pos.hash();
        assert_ne!(hash0, hash1, "rule50 must separate the clocks");
        assert_eq!(
            pos.repetition_key(),
            start.repetition_key(),
            "the repetition key must ignore the clock"
        );

        let ctx = 0x5150;
        let mut cache = RepetitionCache::with_capacity(16);
        cache.store(hash0, ctx, 4);
        assert_eq!(
            cache.probe(hash1, ctx),
            None,
            "clock-distinct boards must not share entries"
        );
        assert_eq!(cache.probe(hash0, ctx), Some(4));
    }

    #[test]
    fn clear_drops_entries_and_hits() {
        let mut cache = RepetitionCache::with_capacity(16);
        cache.store(9, 9, 2);
        assert_eq!(cache.probe(9, 9), Some(2));
        assert_eq!(cache.hits, 1);
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.probe(9, 9), None);
        assert_eq!(cache.hits, 0);
    }
}
