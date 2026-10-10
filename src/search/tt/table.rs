//! Transposition table storage and lookup.
//!
//! This file is slightly larger than 10 KiB because the storage layout, the
//! salted bucket-index mapping, the architecture-specific prefetch, and the
//! eviction policy all share one indexing scheme and its invariants; unit
//! tests are split out (`tests.rs`).

use crate::position::Outcome;
use crate::zobrist::INF;
use atomic_movegen::types::Move;

use super::entry::TtEntry;

pub struct TranspositionTable {
    table: Vec<[TtEntry; 2]>,
    mask: usize,
    current_generation: u32,
    /// Bucket-index salt (see [`TranspositionTable::set_salt`]).
    salt: u64,
}

impl TranspositionTable {
    #[cfg(test)]
    pub(crate) fn bucket_count(&self) -> usize {
        self.table.len()
    }

    /// Construct a table with the given number of buckets (rounded up to the
    /// next power of two). This is `pub(crate)` so unit tests can force
    /// collisions and eviction deterministically.
    #[cfg(test)]
    pub(crate) fn with_capacity(buckets: usize) -> Self {
        let buckets = buckets.next_power_of_two().max(1);
        Self {
            table: vec![[TtEntry::default(); 2]; buckets],
            mask: buckets - 1,
            current_generation: 1,
            salt: 0,
        }
    }

    #[must_use]
    pub fn with_mb(mb: usize) -> Self {
        let bytes = mb.saturating_mul(1024 * 1024);
        let entries = (bytes / std::mem::size_of::<TtEntry>()).next_power_of_two();
        let entries = entries.max(32);
        let buckets = entries.max(2) / 2;
        Self {
            table: vec![[TtEntry::default(); 2]; buckets],
            mask: buckets - 1,
            current_generation: 1,
            salt: 0,
        }
    }

    /// Set the bucket-index salt.
    ///
    /// With `salt = 0` (the default) the bucket index is `key & mask`, exactly
    /// the shipped behavior. With `salt = s > 0` the index becomes
    /// `((key ^ s) * 0x9E3779B97F4A7C15 >> 17) & mask`: a semantics-neutral
    /// remap of which keys share a bucket (and therefore of the eviction
    /// pattern). Full-key verification is unchanged, so probe/store hit-miss
    /// semantics are identical for every salt.
    ///
    /// # Contract
    ///
    /// The salt must be set once, before the first `probe`/`store`/`prefetch`
    /// on this table (i.e. at construction time, before any search run). It is
    /// not reseeded mid-run and must not be changed between runs that are
    /// meant to share table contents. This is the noise-channel knob behind
    /// the salt-seeded statistical gate
    /// (`docs/plans/research/gate_methodology.md`).
    pub fn set_salt(&mut self, salt: u64) {
        self.salt = salt;
    }

    /// The bucket-index salt in effect (0 = shipped indexing).
    #[must_use]
    pub fn salt(&self) -> u64 {
        self.salt
    }

    #[inline]
    fn index(&self, key: u64) -> usize {
        if self.salt == 0 {
            // Shipped path, bit-identical: the mixing formula below would NOT
            // reduce to `key & mask` at s = 0, so it must stay gated.
            (key as usize) & self.mask
        } else {
            (((key ^ self.salt).wrapping_mul(0x9E37_79B9_7F4A_7C15)) >> 17) as usize & self.mask
        }
    }

    /// The bucket index `key` maps to (test visibility only).
    #[cfg(test)]
    pub(crate) fn index_for_test(&self, key: u64) -> usize {
        self.index(key)
    }

    /// Hint the CPU to start loading the bucket `key` maps to.
    ///
    /// Pure performance hint: no observable effect on table contents or on
    /// any probe/store result. Issues one prefetch for the first and one
    /// for the last byte of the bucket, because a 112-byte bucket can
    /// straddle two cache lines (line-aligning the table was measured as no
    /// gain, lean plan11 phase 0). Measured win (lean plan11, spike V10):
    /// prefetching every child's bucket in a pre-pass before
    /// `evaluate_all_children`'s eval loop removes the one serial DRAM miss
    /// per evaluated child and roughly halves wall on the memory-bound
    /// workloads (−48–55%), stdout byte-identical. No prefetch on other
    /// architectures: this is then a no-op.
    #[inline(always)]
    pub fn prefetch(&self, key: u64) {
        // The index is masked to bounds, so the addresses below always lie
        // inside the live `table` allocation; no `unsafe` is needed to form
        // them.
        let bucket = &self.table[self.index(key)];
        let first = bucket.as_ptr().cast::<u8>();
        let last = first.wrapping_add(std::mem::size_of::<[TtEntry; 2]>() - 1);

        #[cfg(target_arch = "aarch64")]
        // SAFETY: a prefetch has no architectural side effect and never
        // faults (even on an invalid address); both addresses lie inside the
        // live `table` allocation. The measured win is recorded in the doc
        // comment above (lean plan11).
        unsafe {
            core::arch::asm!(
                "prfm pldl1keep, [{ptr0}]",
                "prfm pldl1keep, [{ptr1}]",
                ptr0 = in(reg) first,
                ptr1 = in(reg) last,
                options(nostack, readonly, preserves_flags),
            );
        }

        #[cfg(target_arch = "x86_64")]
        // SAFETY: as on aarch64 — hint-only, fault-free, and both addresses
        // lie inside the live `table` allocation.
        unsafe {
            core::arch::x86_64::_mm_prefetch::<{ core::arch::x86_64::_MM_HINT_T0 }>(
                first.cast::<i8>(),
            );
            core::arch::x86_64::_mm_prefetch::<{ core::arch::x86_64::_MM_HINT_T0 }>(
                last.cast::<i8>(),
            );
        }

        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        let _ = (first, last);
    }

    #[must_use]
    pub fn probe(&self, key: u64) -> Option<&TtEntry> {
        self.table[self.index(key)]
            .iter()
            .find(|e| e.valid && e.key == key && e.generation == self.current_generation)
    }

    pub fn clear(&mut self) {
        for bucket in &mut self.table {
            *bucket = [TtEntry::default(); 2];
        }
        self.current_generation = 1;
    }

    /// Mark every existing table entry as belonging to an older generation.
    pub fn new_generation(&mut self) {
        self.current_generation = self.current_generation.wrapping_add(1);
        if self.current_generation == 0 {
            self.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn store(
        &mut self,
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

        let idx = self.index(key);
        let mut existing = false;

        {
            let bucket = &mut self.table[idx];
            for slot in bucket.iter_mut() {
                if slot.valid && slot.key == key && slot.generation == self.current_generation {
                    existing = true;
                    slot.generation = self.current_generation;
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
        }

        if existing {
            return;
        }

        let new = TtEntry {
            key,
            valid: true,
            generation: self.current_generation,
            best_move,
            best_child,
            work,
            outcome,
            pn,
            dn,
            depth,
            remaining_depth,
        };

        self.insert_new(idx, new);
    }

    /// The generation counter at the time of the call.
    ///
    /// Used by snapshot/debug tooling to partition entries by generation.
    #[must_use]
    pub fn current_generation(&self) -> u32 {
        self.current_generation
    }

    /// Iterate over all `valid` entries in native bucket order, across all
    /// generations.
    ///
    /// This is for snapshot/debug use; the search itself only ever looks at
    /// entries via `probe` (current generation). Callers that need
    /// generation-dependent semantics must filter by `current_generation()`
    /// themselves.
    pub fn entries(&self) -> impl Iterator<Item = &TtEntry> {
        self.table
            .iter()
            .flat_map(|bucket| bucket.iter())
            .filter(|e| e.valid)
    }

    /// Return a distribution of stored `best_child` values among live entries.
    ///
    /// `u8::MAX` (the "unknown" sentinel) is excluded. This is useful for
    /// debugging proof-tree and GHI path-code usage.
    #[must_use]
    pub fn best_child_counts(&self) -> Vec<(u8, usize)> {
        let mut counts = std::collections::HashMap::new();
        for bucket in &self.table {
            for entry in bucket {
                if entry.valid
                    && entry.generation == self.current_generation
                    && entry.best_child != u8::MAX
                {
                    *counts.entry(entry.best_child).or_insert(0) += 1;
                }
            }
        }
        let mut v: Vec<_> = counts.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v
    }

    /// Return aggregate statistics about the current transposition table contents.
    ///
    /// Tuple fields are: `(buckets, live_entries, solved_entries, unsolved_entries, generation)`.
    #[must_use]
    pub fn stats(&self) -> (usize, usize, usize, usize, u32) {
        let mut live = 0;
        let mut solved = 0;
        for bucket in &self.table {
            for entry in bucket {
                if entry.valid && entry.generation == self.current_generation {
                    live += 1;
                    if entry.outcome.is_some() {
                        solved += 1;
                    }
                }
            }
        }
        let unsolved = live - solved;
        (
            self.table.len(),
            live,
            solved,
            unsolved,
            self.current_generation,
        )
    }

    /// Place a new entry into `idx`, preferring the two most valuable entries.
    fn insert_new(&mut self, idx: usize, new: TtEntry) {
        let current_generation = self.current_generation;
        let score = |e: &TtEntry| {
            let live = e.valid && e.generation == current_generation;
            let solved = e.outcome.is_some();
            (u8::from(live), u8::from(solved), e.work, e.generation)
        };

        let bucket = &mut self.table[idx];
        if !bucket[0].valid || bucket[0].generation != current_generation {
            bucket[0] = new;
        } else if !bucket[1].valid || bucket[1].generation != current_generation {
            bucket[1] = new;
        } else {
            let evict = usize::from(score(&bucket[0]) >= score(&bucket[1]));
            bucket[evict] = new;
        }
    }
}
