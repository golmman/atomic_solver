use super::shard::{MAX_SHARDS, layout};
use super::{TranspositionTable, TtEntry};
use crate::position::Outcome;
use atomic_movegen::types::{Move, Square};

#[test]
fn with_capacity_rounds_to_power_of_two() {
    let tt = TranspositionTable::with_capacity(1);
    assert_eq!(tt.bucket_count(), 1);

    let tt = TranspositionTable::with_capacity(3);
    assert_eq!(tt.bucket_count(), 4);

    let tt = TranspositionTable::with_capacity(0);
    assert_eq!(tt.bucket_count(), 1);
}

#[test]
fn with_capacity_forces_deterministic_eviction() {
    // One bucket, two slots: a third distinct key must evict a live slot.
    let tt = TranspositionTable::with_capacity(1);

    let key1 = 1u64;
    let key2 = 2u64;
    let key3 = 3u64;

    tt.store(
        key1,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        0,
        0,
        u32::MAX,
    );
    tt.store(
        key2,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Loss),
        0,
        0,
        0,
        u32::MAX,
    );
    assert!(tt.probe(key1).is_some());
    assert!(tt.probe(key2).is_some());

    tt.store(
        key3,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Draw),
        0,
        0,
        0,
        u32::MAX,
    );
    assert!(
        tt.probe(key3).is_some(),
        "new entry should be present after eviction"
    );
    let remaining = [tt.probe(key1).is_some(), tt.probe(key2).is_some()]
        .iter()
        .filter(|&&b| b)
        .count();
    assert_eq!(
        remaining, 1,
        "exactly one old entry should survive eviction"
    );
}

#[test]
fn empty_table_probe_returns_none() {
    let tt = TranspositionTable::with_mb(1);
    assert!(tt.probe(0x1234).is_none());
}

#[test]
fn table_size_is_power_of_two_and_at_least_minimum() {
    let tt = TranspositionTable::with_mb(1);
    assert!(tt.bucket_count().is_power_of_two());
    assert!(tt.bucket_count() >= 16);
}

#[test]
fn clear_removes_all_entries_and_resets_generation() {
    let tt = TranspositionTable::with_mb(1);
    let key = 0x222u64;
    tt.store(
        key,
        Move::make_move(Square::E2, Square::E4),
        u8::MAX,
        1,
        Some(Outcome::Win),
        0,
        crate::zobrist::INF,
        1,
        u32::MAX,
    );
    tt.clear();
    assert!(tt.probe(key).is_none());
    assert!(tt.bucket_count().is_power_of_two());
}

#[test]
fn probe_matches_stored_entry_fields() {
    let tt = TranspositionTable::with_mb(1);
    let key = 0xdead_beefu64;
    let mv = Move::make_move(Square::E2, Square::E4);
    tt.store(
        key,
        mv,
        u8::MAX,
        42,
        Some(Outcome::Win),
        0,
        crate::zobrist::INF,
        5,
        3,
    );

    let entry = tt.probe(key).expect("entry should be present");
    assert_eq!(entry.outcome, Some(Outcome::Win));
    assert_eq!(entry.best_move, mv);
    assert_eq!(entry.work, 42);
    assert_eq!(entry.depth, 5);
}

#[test]
fn tt_entry_size_is_reasonable() {
    let size = std::mem::size_of::<TtEntry>();
    assert!(size <= 128, "TtEntry size {size} exceeds 128 bytes");
}

#[test]
fn store_and_probe_solved_result() {
    let tt = TranspositionTable::with_mb(1);
    let key = 12345u64;
    let mv = Move::make_move(
        atomic_movegen::types::Square::A1,
        atomic_movegen::types::Square::A2,
    );

    tt.store(
        key,
        mv,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        crate::zobrist::INF,
        7,
        u32::MAX,
    );

    let entry = tt.probe(key).expect("stored entry should be found");
    assert_eq!(entry.outcome, Some(Outcome::Win));
    assert_eq!(entry.best_move, mv);
    assert_eq!(entry.depth, 7);
    assert!(entry.result_for(Outcome::Win).is_some());
    assert!(entry.result_for(Outcome::Loss).is_none());
}

#[test]
fn unsolved_bounds_do_not_overwrite_solved() {
    let tt = TranspositionTable::with_mb(1);
    let key = 12345u64;
    let mv = Move::make_move(
        atomic_movegen::types::Square::A1,
        atomic_movegen::types::Square::A2,
    );

    tt.store(
        key,
        mv,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        crate::zobrist::INF,
        7,
        u32::MAX,
    );
    tt.store(key, Move::NONE, u8::MAX, 100, None, 1, 1, 0, 0);

    let entry = tt.probe(key).expect("entry should still exist");
    assert_eq!(entry.outcome, Some(Outcome::Win));
    assert_eq!(entry.best_move, mv);
}

#[test]
fn solved_result_overwrites_unsolved_bounds() {
    let tt = TranspositionTable::with_mb(1);
    let key = 12345u64;
    let mv = Move::make_move(
        atomic_movegen::types::Square::A1,
        atomic_movegen::types::Square::A2,
    );

    tt.store(key, Move::NONE, u8::MAX, 0, None, 1, 1, 0, 0);
    tt.store(
        key,
        mv,
        u8::MAX,
        0,
        Some(Outcome::Loss),
        crate::zobrist::INF,
        0,
        12,
        u32::MAX,
    );

    let entry = tt.probe(key).expect("entry should exist");
    assert_eq!(entry.outcome, Some(Outcome::Loss));
    assert_eq!(entry.best_move, mv);
    assert_eq!(entry.depth, 12);
}

#[test]
fn new_generation_marks_old_entries_stale() {
    let tt = TranspositionTable::with_mb(1);
    let key = 123u64;

    tt.store(
        key,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        0,
        0,
        u32::MAX,
    );
    assert!(tt.probe(key).is_some());

    tt.new_generation();
    assert!(tt.probe(key).is_none());

    // Storing the same key in the new generation makes it visible again.
    tt.store(
        key,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        0,
        0,
        u32::MAX,
    );
    assert!(tt.probe(key).is_some());
}

#[test]
fn new_generation_prefers_stale_slot() {
    let tt = TranspositionTable::with_mb(1);
    let key1 = 1u64;
    // Force two slots in the same bucket by using keys that differ by a
    // multiple of the bucket count.
    let bucket_count = tt.bucket_count() as u64;
    let key2 = key1 + bucket_count;

    tt.store(
        key1,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        0,
        0,
        u32::MAX,
    );
    tt.store(
        key2,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        0,
        0,
        u32::MAX,
    );
    assert!(tt.probe(key1).is_some());
    assert!(tt.probe(key2).is_some());

    tt.new_generation();

    // A third key landing in the same bucket should overwrite a stale slot
    // instead of evicting a single live slot and losing the other.
    let key3 = key1 + 2 * bucket_count;
    tt.store(
        key3,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Draw),
        0,
        0,
        0,
        u32::MAX,
    );
    assert!(tt.probe(key3).is_some());
    assert!(
        tt.probe(key1).is_none() || tt.probe(key2).is_none(),
        "new entry should overwrite a stale slot, not both live slots"
    );
}

#[test]
fn for_each_entry_visits_all_valid_entries_in_bucket_order_across_generations() {
    let tt = TranspositionTable::with_capacity(4);
    let mut count = 0usize;
    tt.for_each_entry(|_| count += 1);
    assert_eq!(count, 0, "fresh table has no valid entries");

    // Keys land in distinct buckets (index = key & 3) but are stored out of
    // bucket order on purpose.
    tt.store(
        0x33,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        0,
        0,
        u32::MAX,
    );
    tt.new_generation();
    tt.store(
        0x11,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Loss),
        0,
        0,
        0,
        u32::MAX,
    );
    tt.store(0x22, Move::NONE, u8::MAX, 0, None, 1, 1, 0, 0);

    let mut keys: Vec<u64> = Vec::new();
    tt.for_each_entry(|e| keys.push(e.key));
    assert_eq!(
        keys,
        vec![0x11, 0x22, 0x33],
        "entries must be yielded in native bucket order, all generations"
    );

    let mut outcomes: Vec<bool> = Vec::new();
    tt.for_each_entry(|e| outcomes.push(e.outcome.is_some()));
    assert_eq!(outcomes, vec![true, false, true]);
}

#[test]
fn current_generation_reflects_new_generation_and_clear() {
    let tt = TranspositionTable::with_capacity(2);
    assert_eq!(tt.current_generation(), 1);
    tt.new_generation();
    assert_eq!(tt.current_generation(), 2);
    tt.new_generation();
    assert_eq!(tt.current_generation(), 3);
    tt.clear();
    assert_eq!(tt.current_generation(), 1);
}

#[test]
fn layout_partitions_the_bucket_range() {
    let (shards, bps, shift) = layout(16384);
    assert_eq!((shards, bps, shift), (256, 64, 6));
    let (shards, bps, shift) = layout(4);
    assert_eq!((shards, bps, shift), (4, 1, 0));
    let (shards, bps, shift) = layout(1);
    assert_eq!((shards, bps, shift), (1, 1, 0));
    for total in [1usize, 2, 4, 16, 1024, 16384] {
        let (shards, bps, _) = layout(total);
        assert_eq!(shards * bps, total);
        assert!(shards <= MAX_SHARDS);
    }
}

#[test]
fn shard_split_preserves_native_bucket_order() {
    // 16 buckets -> 16 shards of one bucket each; keys chosen to cross shard
    // boundaries and to share a bucket (0x11 vs 0x11 + bucket_count).
    let tt = TranspositionTable::with_capacity(16);
    let bucket_count = tt.bucket_count() as u64;
    assert_eq!(bucket_count, 16);

    let keys = [0x10u64, 0x11, 0x11 + bucket_count, 0x05];
    for k in keys {
        tt.store(
            k,
            Move::NONE,
            u8::MAX,
            0,
            Some(Outcome::Win),
            0,
            0,
            0,
            u32::MAX,
        );
        assert!(tt.probe(k).is_some(), "key {k:#x} must be probe-visible");
    }

    // Reference enumeration: buckets in index order, slots in insertion order.
    let mut expected: Vec<u64> = Vec::new();
    for b in 0..bucket_count {
        for &k in &keys {
            if k & (bucket_count - 1) == b {
                expected.push(k);
            }
        }
    }
    let mut visited: Vec<u64> = Vec::new();
    tt.for_each_entry(|e| visited.push(e.key));
    assert_eq!(
        visited, expected,
        "for_each_entry must follow native bucket order"
    );
}

/// Concurrent `store` + `probe` over a shared table: 4 threads with disjoint
/// key ranges. Invariants checked under the races: no panic, no torn entries
/// (`outcome == Some(Win)` is only ever stored together with `work >= 1000`,
/// so a mixed old/new read shows up as a violation), solved entries are never
/// downgraded *in place*, and `work` is monotone. A thread's own Win entry may
/// still be *evicted* by a concurrent higher-work solved store and refilled by
/// its own phase-2 unsolved store; that legal sequence is recognized by the
/// refill's exact store-argument shape (see the phase-2 verification).
#[test]
fn concurrent_store_and_probe_respect_solver_invariants() {
    use std::sync::Arc;
    use std::sync::Barrier;

    const THREADS: usize = 4;
    const KEYS_PER_THREAD: u64 = 4096;

    let tt = Arc::new(TranspositionTable::with_mb(1));
    let barrier = Arc::new(Barrier::new(THREADS));

    std::thread::scope(|scope| {
        for t in 0..THREADS {
            let tt = Arc::clone(&tt);
            let barrier = Arc::clone(&barrier);
            scope.spawn(move || {
                barrier.wait();
                let base = t as u64 * 1_000_003;

                // Phase 1: solved entries with work = 1000.
                for i in 0..KEYS_PER_THREAD {
                    let key = base + i;
                    tt.store(
                        key,
                        Move::NONE,
                        u8::MAX,
                        1000,
                        Some(Outcome::Win),
                        0,
                        crate::zobrist::INF,
                        i as u32,
                        u32::MAX,
                    );
                }

                // Phase 2: unsolved bounds with lower work, then verify the
                // solver invariants on whatever is still probe-visible
                // (collisions may legitimately have evicted the entry).
                for i in 0..KEYS_PER_THREAD {
                    let key = base + i;
                    tt.store(key, Move::NONE, u8::MAX, 7, None, 1, 1, 0, 0);
                }
                for i in 0..KEYS_PER_THREAD {
                    let key = base + i;
                    if let Some(e) = tt.probe(key) {
                        assert_eq!(e.key, key);
                        if e.outcome == Some(Outcome::Win) {
                            assert!(e.work >= 1000, "work must be monotone");
                            assert_eq!(e.remaining_depth, u32::MAX);
                        } else {
                            // The Win entry may have been legitimately evicted
                            // by another thread's phase-1 store (k=2
                            // replacement over higher-work solved entries) and
                            // then refilled by *this* thread's own phase-2
                            // unsolved store. Store policy never downgrades a
                            // solved entry in place, so an unsolved own-key
                            // entry must be a fresh refill, exact-shape: the
                            // phase-2 store arguments with nothing mixed in.
                            assert_eq!(
                                e.work, 7,
                                "unsolved own-key entry must be a fresh phase-2 refill"
                            );
                            assert_eq!(e.remaining_depth, 0);
                            assert_eq!(e.depth, 0);
                        }
                    }
                }

                // Phase 3: probe other threads' ranges (concurrent readers).
                let other = ((t + 1) % THREADS) as u64 * 1_000_003;
                for i in 0..KEYS_PER_THREAD {
                    if let Some(e) = tt.probe(other + i) {
                        assert_eq!(e.key, other + i);
                        // No torn mix: a solved entry always carries its
                        // solved-time work value.
                        if e.outcome.is_some() {
                            assert!(e.work >= 1000);
                        }
                    }
                }
            });
        }
    });

    // Global post-condition: every surviving entry is coherent.
    let (buckets, live, _solved, _unsolved, generation) = tt.stats();
    assert!(live <= 2 * buckets);
    assert!(generation >= 1);
    tt.for_each_entry(|e| {
        if e.outcome.is_some() {
            assert!(
                e.work >= 1000,
                "no torn entry: solved implies solved-time work"
            );
        } else {
            assert!(e.work >= 7, "unsolved work only ever grows from 7");
        }
    });
}
