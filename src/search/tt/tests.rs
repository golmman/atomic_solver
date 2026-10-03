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
    let mut tt = TranspositionTable::with_capacity(1);

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
    let mut tt = TranspositionTable::with_mb(1);
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
    let mut tt = TranspositionTable::with_mb(1);
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
    let mut tt = TranspositionTable::with_mb(1);
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
    let mut tt = TranspositionTable::with_mb(1);
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
    let mut tt = TranspositionTable::with_mb(1);
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
    let mut tt = TranspositionTable::with_mb(1);
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
    let mut tt = TranspositionTable::with_mb(1);
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
fn entries_yields_all_valid_entries_in_bucket_order_across_generations() {
    let mut tt = TranspositionTable::with_capacity(4);
    assert_eq!(tt.entries().count(), 0, "fresh table has no valid entries");

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

    let keys: Vec<u64> = tt.entries().map(|e| e.key).collect();
    assert_eq!(
        keys,
        vec![0x11, 0x22, 0x33],
        "entries must be yielded in native bucket order, all generations"
    );

    let outcomes: Vec<bool> = tt.entries().map(|e| e.outcome.is_some()).collect();
    assert_eq!(outcomes, vec![true, false, true]);
}

#[test]
fn current_generation_reflects_new_generation_and_clear() {
    let mut tt = TranspositionTable::with_capacity(2);
    assert_eq!(tt.current_generation(), 1);
    tt.new_generation();
    assert_eq!(tt.current_generation(), 2);
    tt.new_generation();
    assert_eq!(tt.current_generation(), 3);
    tt.clear();
    assert_eq!(tt.current_generation(), 1);
}
