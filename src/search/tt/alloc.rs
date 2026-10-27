//! Zero-initialized bucket allocation for the transposition table.
//!
//! The table's bucket array is by far the largest allocation in the program
//! (224 MiB at the default `--tt-size 256` label) and the vast majority of a
//! process's TtEntry slots are never touched: a mate-in-1 needs a handful,
//! a deep search touches most of them. Building the array with
//! `vec![[TtEntry::default(); 4]; buckets]` writes every byte up front — a
//! non-zero `Default` (see [`super::entry::TtEntry::default`]) forces the
//! allocator to hand back *committed* anonymous memory and to fault in all
//! ~57k pages of the default label before the search starts (performance
//! initiative item #5; measured at ~100 ms and ~230 MB RSS per process on the
//! reference container, which was the entire short-solve latency).
//!
//! [`zeroed_buckets`] uses [`std::alloc::alloc_zeroed`] instead. For large
//! requests that call is served by fresh `mmap`ed zero pages on the shipped
//! targets, so construction costs microseconds and pages become resident only
//! on first touch: RSS is bounded by the label rather than equal to it. The
//! deep-search cost of demand faulting those pages inside the search is
//! measured (small, +1–5% wall) and pre-registered in
//! `docs/plans/performance/plan2.md`.
//!
//! # Why this is safe (the representation invariant)
//!
//! An all-zero byte pattern *is* a valid `TtEntry`:
//!
//! - `key: 0`, `generation: 0`, `work: 0`, `pn: 0`, `dn: 0`, `depth: 0`,
//!   `remaining_depth: 0` — integers accept any bit pattern;
//! - `valid: false` — `false` is the zero bit pattern of `bool`;
//! - `best_move: Move(0)` (= `Move::NONE`), `best_child: 0` — all bit
//!   patterns of the underlying integers are valid;
//! - `outcome: Option<Outcome>` — `Outcome`'s niche places `None` at a
//!   *non-zero* byte value, so a zeroed `outcome` reads as `Some(Loss)`,
//!   not `None`.
//!
//! That last point is the crux: **a zeroed slot may read `outcome ==
//! Some(..)`**, so `outcome` is not a liveness signal. `valid == false` is
//! the single liveness signal, and every read path in [`super::table`] filters
//! on it (`probe`, `entries`, `stats`, `best_child_counts`) plus, where it
//! scans rather than probes, on the current generation. Non-live slots'
//! remaining field values are therefore *unspecified* and must never be
//! observed. With that contract the lazy allocation is a
//! representation-only change: live-slot transitions (`store`, generation
//! bumps, `clear`) are untouched, so table contents and search trajectories
//! are bit-identical to the eager construction
//! (`zeroed_allocator_matches_eager_defaults`).
//!
//! `std::mem::zeroed::<TtEntry>()` being valid is pinned by the unit test
//! `zeroed_bytes_form_an_unoccupied_entry` below.
//!
//! This file is slightly larger than 10 KiB because the safety invariant (one
//! normative description), the allocator, and the empty-table/equivalence
//! tests that pin the invariant all live together; splitting them would
//! duplicate the invariant statement.

use super::entry::TtEntry;

/// Allocate `buckets` zero-initialized four-slot buckets.
///
/// Backed by [`std::alloc::alloc_zeroed`], which for the default label
/// (224 MiB) is served by `mmap`ed zero pages: the allocation costs
/// microseconds and pages become resident on first touch. Capacity and layout
/// are exactly those of `vec![[TtEntry::default(); 4]; buckets]`; the contents
/// differ only in the unspecified fields of non-live slots (see the module
/// docs).
///
/// # Panics
///
/// Panics if the byte size of `buckets` four-slot buckets overflows `usize`
/// (impossible in practice: `usize::MAX / 224` buckets is unreachable memory).
pub(crate) fn zeroed_buckets(buckets: usize) -> Vec<[TtEntry; 4]> {
    if buckets == 0 {
        return Vec::new();
    }
    let layout = std::alloc::Layout::array::<[TtEntry; 4]>(buckets)
        .expect("tt allocation layout overflows usize");
    // SAFETY: `alloc_zeroed` returns a pointer to `layout.size()` bytes of
    // zeroed memory (or null, reported via `handle_alloc_error`). All-zero
    // bytes form a valid `TtEntry` whose `valid` flag is `false`
    // (`zeroed_bytes_form_an_unoccupied_entry`), so the memory may be read as
    // `[TtEntry; 4]`; `Vec::from_raw_parts` gets the exact layout and
    // capacity that was requested, from the same global allocator that
    // `Vec`'s own growth uses, and the vector is the sole owner.
    unsafe {
        let ptr = std::alloc::alloc_zeroed(layout).cast::<[TtEntry; 4]>();
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Vec::from_raw_parts(ptr, buckets, buckets)
    }
}

/// The eager construction the lazy allocator must match (test visibility
/// only): every slot written with [`TtEntry::default`].
#[cfg(test)]
pub(crate) fn eager_buckets(buckets: usize) -> Vec<[TtEntry; 4]> {
    vec![[TtEntry::default(); 4]; buckets]
}

#[cfg(test)]
mod tests {
    use super::super::table::TranspositionTable;
    use super::*;
    use crate::position::Outcome;
    use atomic_movegen::types::Move;

    #[test]
    fn zeroed_bytes_form_an_unoccupied_entry() {
        // The safety justification of `zeroed_buckets`: an all-zero TtEntry
        // is a valid value and reads as *not live*. `outcome` may (and does)
        // read as `Some(..)` — `valid` is the only liveness signal.
        // SAFETY: `TtEntry` is a plain aggregate of integers, a `bool`, a
        // 16-bit `Move` and `Option<Outcome>`; every all-zero bit pattern is a
        // valid value of each field (see the module docs), so `zeroed` is
        // sound here. This is exactly the invariant the allocator relies on.
        let entry: TtEntry = unsafe { std::mem::zeroed() };
        assert!(!entry.valid, "a zeroed slot must read as not live");
        assert_eq!(entry.key, 0);
        assert_eq!(entry.generation, 0);
        assert_eq!(entry.work, 0);
        assert_eq!(entry.best_move, atomic_movegen::types::Move::NONE);
        // Not a liveness signal: document what a zeroed slot actually holds.
        assert_eq!(entry.outcome, Some(Outcome::Loss));
        assert_ne!(entry.outcome, None);
    }

    #[test]
    fn zeroed_buckets_zero_size_is_empty() {
        let buckets = zeroed_buckets(0);
        assert!(buckets.is_empty());
        assert_eq!(buckets.capacity(), 0);
    }

    #[test]
    fn zeroed_buckets_have_the_requested_length_and_are_not_live() {
        for buckets in [1usize, 3, 7, 1024] {
            let allocated = zeroed_buckets(buckets);
            assert_eq!(allocated.len(), buckets);
            assert!(allocated.iter().flatten().all(|e| !e.valid));
        }
    }

    #[test]
    fn zeroed_buckets_match_eager_defaults_on_non_live_fields() {
        // Fields the eager `Default` sets to non-zero values (best_child,
        // pn, dn, outcome) are the only ones that differ; the live/non-live
        // verdict and the key/generation must agree, because those are what
        // the table's read paths observe.
        let lazy = zeroed_buckets(64);
        let eager = eager_buckets(64);
        assert_eq!(lazy.len(), eager.len());
        for (a, b) in lazy.iter().zip(eager.iter()) {
            for (x, y) in a.iter().zip(b.iter()) {
                assert_eq!(x.valid, y.valid);
                assert!(!x.valid);
                assert_eq!(x.key, y.key);
                assert_eq!(x.generation, y.generation);
            }
        }
    }

    // --- lazy (zero-init) allocation (performance plan2, item #5) ---

    /// One live entry's observable fields, for content-equivalence checks.
    type EntryProjection = (u64, bool, u32, Option<Outcome>, u8, u64, u32);

    /// The full observable projection of a table, for content-equivalence checks.
    fn projection(tt: &TranspositionTable) -> Vec<EntryProjection> {
        tt.entries()
            .map(|e| {
                (
                    e.key,
                    e.valid,
                    e.generation,
                    e.outcome,
                    e.best_child,
                    e.work,
                    e.depth,
                )
            })
            .collect()
    }

    #[test]
    fn fresh_table_is_empty() {
        // Guards the zeroed-slot hazard: a zeroed entry reads `outcome ==
        // Some(..)`, so an empty table must still report no live entries and no
        // probe hits for any key.
        let tt = TranspositionTable::with_mb(1);
        let (buckets, live, solved, unsolved, generation) = tt.stats();
        assert_eq!((live, solved, unsolved, generation), (0, 0, 0, 1));
        assert_eq!(buckets, 4096);
        assert_eq!(tt.entries().count(), 0);
        assert!(tt.best_child_counts().is_empty());
        for key in [0u64, 1, 0xFFF, 4096, 0xDEAD_BEEF_CAFE_F00D, u64::MAX] {
            assert!(
                tt.probe(key).is_none(),
                "fresh table must not hit key {key}"
            );
        }
    }

    #[test]
    fn zeroed_allocator_matches_eager_defaults() {
        // The representation-only claim: a table built from `zeroed_buckets` and
        // one built from `TtEntry::default` react identically to the same
        // store/probe/generation sequence — same live contents, same stats, same
        // best_child census. Covers collisions with eviction, an outcome
        // overwrite, a generation bump and `clear()`.
        let lazy = zeroed_buckets(4);
        let eager = eager_buckets(4);
        assert_eq!(lazy.len(), eager.len());

        let mut lazy_tt = TranspositionTable::with_capacity(4);
        let mut eager_tt = TranspositionTable::with_capacity(4);
        assert_eq!(projection(&lazy_tt), projection(&eager_tt));
        assert_eq!(lazy_tt.stats(), eager_tt.stats());

        // Four slots of bucket 0 (keys ≡ 0 mod 4): fill them, then evict.
        let mut seq: Vec<(u64, Option<Outcome>, u64, u8, u32)> = Vec::new();
        for i in 0..6u64 {
            seq.push((i * 4, Some(Outcome::Win), i, i as u8, i as u32));
        }
        // An unsolved bound, then an outcome overwrite of the same key.
        seq.push((4, None, 100, u8::MAX, 0));
        seq.push((4, Some(Outcome::Loss), 200, 3, 9));
        // A key in another bucket, to keep the census non-trivial.
        seq.push((1, Some(Outcome::Draw), 7, 1, 2));

        for &(key, outcome, work, best_child, depth) in &seq {
            for tt in [&mut lazy_tt, &mut eager_tt] {
                tt.store(key, Move::NONE, best_child, work, outcome, 1, 1, depth, 3);
                tt.new_generation();
                tt.new_generation();
            }
        }
        assert_eq!(projection(&lazy_tt), projection(&eager_tt));
        assert_eq!(lazy_tt.stats(), eager_tt.stats());
        assert_eq!(lazy_tt.best_child_counts(), eager_tt.best_child_counts());

        // Store again in the current generation and probe: same hits.
        for &(key, outcome, work, best_child, depth) in &seq {
            for tt in [&mut lazy_tt, &mut eager_tt] {
                tt.store(key, Move::NONE, best_child, work, outcome, 1, 1, depth, 3);
            }
        }
        for key in 0..24u64 {
            assert_eq!(
                lazy_tt.probe(key).map(|e| e.outcome),
                eager_tt.probe(key).map(|e| e.outcome),
                "probe disagreement on key {key}"
            );
        }
        assert_eq!(projection(&lazy_tt), projection(&eager_tt));
        assert_eq!(lazy_tt.best_child_counts(), eager_tt.best_child_counts());

        lazy_tt.clear();
        eager_tt.clear();
        assert_eq!(projection(&lazy_tt), projection(&eager_tt));
        assert_eq!(lazy_tt.stats(), eager_tt.stats());
        assert_eq!(lazy_tt.entries().count(), 0);
    }
}
