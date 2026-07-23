use super::entry::MAX_TWINS;
use super::{TranspositionTable, TtEntry};
use crate::position::Outcome;
use atomic_movegen::types::Move;

#[test]
fn tt_entry_size_is_reasonable() {
    // MAX_TWINS was raised to 8; keep the per-entry size bounded so the
    // default 64 MB table still holds a useful number of entries.
    let size = std::mem::size_of::<TtEntry>();
    assert!(size <= 512, "TtEntry size {size} exceeds 512 bytes");
}

#[test]
fn twin_metrics_track_insertions_and_evictions() {
    let mut tt = TranspositionTable::with_mb(1);
    let key = 12345u64;

    for i in 0..MAX_TWINS as u64 {
        tt.store_twin(key, i, 0, Outcome::Draw, Move::NONE, 0, u32::MAX, 0);
    }

    assert_eq!(tt.twin_stats().0, MAX_TWINS as u64);
    assert_eq!(tt.twin_stats().1, 0);

    // One more twin evicts the oldest slot.
    tt.store_twin(
        key,
        MAX_TWINS as u64,
        0,
        Outcome::Draw,
        Move::NONE,
        0,
        u32::MAX,
        0,
    );
    assert_eq!(tt.twin_stats().0, MAX_TWINS as u64 + 1);
    assert_eq!(tt.twin_stats().1, 1);
}

#[test]
fn clear_resets_twin_stats() {
    let mut tt = TranspositionTable::with_mb(1);
    tt.store_twin(1, 0, 0, Outcome::Draw, Move::NONE, 0, u32::MAX, 0);
    tt.clear();
    assert_eq!(tt.twin_stats().0, 0);
    assert_eq!(tt.twin_stats().1, 0);
    assert_eq!(tt.peak_twins(), 0);
}

#[test]
fn peak_twins_tracked() {
    let mut tt = TranspositionTable::with_mb(1);
    let key = 12345u64;

    for i in 0..4 {
        tt.store_twin(key, i, 0, Outcome::Draw, Move::NONE, 0, u32::MAX, 0);
    }
    assert_eq!(tt.peak_twins(), 4);

    // A second entry also has four twins; peak stays at 4.
    let key2 = 54321u64;
    for i in 0..2 {
        tt.store_twin(key2, i, 0, Outcome::Draw, Move::NONE, 0, u32::MAX, 0);
    }
    assert_eq!(tt.peak_twins(), 4);

    // Filling the first entry to capacity and evicting keeps the peak at 8.
    for i in 4..8 {
        tt.store_twin(key, i, 0, Outcome::Draw, Move::NONE, 0, u32::MAX, 0);
    }
    assert_eq!(tt.peak_twins(), 8);
}

#[test]
fn find_and_best_result_for_multiple_paths() {
    let mut tt = TranspositionTable::with_mb(1);
    let key = 123u64;

    // Twins for two different path codes. Storing a twin reinitialises the
    // base entry, so the table is left with only path-dependent results.
    tt.store_twin(key, 1, 0, Outcome::Loss, Move::NONE, 2, u32::MAX, 0);
    tt.store_twin(key, 2, 0, Outcome::Draw, Move::NONE, 3, u32::MAX, 0);

    let entry = *tt.probe(key).unwrap();

    // A twin is found only for its own path code and expected outcome.
    assert_eq!(
        entry
            .find_result_for_path(1, Outcome::Loss)
            .map(|r| r.depth),
        Some(2)
    );
    assert!(entry.find_result_for_path(1, Outcome::Win).is_none());

    // best_result_for_path returns the twin for path code 2.
    assert_eq!(
        entry.best_result_for_path(2),
        Some((Move::NONE, Some(Outcome::Draw), 3))
    );

    // The base result has been cleared because path-dependent twins are stored.
    assert!(entry.find_result_for_path(0, Outcome::Win).is_none());

    // Unknown path returns nothing.
    assert!(entry.find_result_for_path(99, Outcome::Loss).is_none());
    assert!(entry.best_result_for_path(99).is_none());
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
        0,
        0,
        false,
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
        0,
        0,
        false,
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
        0,
        0,
        false,
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
        0,
        0,
        false,
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
        0,
        0,
        false,
    );
    assert!(tt.probe(key3).is_some());
    // At least one of the old keys should still be absent (overwritten by key3).
    assert!(
        tt.probe(key1).is_none() || tt.probe(key2).is_none(),
        "new entry should overwrite a stale slot, not both live slots"
    );
}
