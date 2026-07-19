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
        tt.store_twin(key, i, 0, Outcome::Draw, Move::NONE, 0, u32::MAX);
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
    );
    assert_eq!(tt.twin_stats().0, MAX_TWINS as u64 + 1);
    assert_eq!(tt.twin_stats().1, 1);
}

#[test]
fn clear_resets_twin_stats() {
    let mut tt = TranspositionTable::with_mb(1);
    tt.store_twin(1, 0, 0, Outcome::Draw, Move::NONE, 0, u32::MAX);
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
        tt.store_twin(key, i, 0, Outcome::Draw, Move::NONE, 0, u32::MAX);
    }
    assert_eq!(tt.peak_twins(), 4);

    // A second entry also has four twins; peak stays at 4.
    let key2 = 54321u64;
    for i in 0..2 {
        tt.store_twin(key2, i, 0, Outcome::Draw, Move::NONE, 0, u32::MAX);
    }
    assert_eq!(tt.peak_twins(), 4);

    // Filling the first entry to capacity and evicting keeps the peak at 8.
    for i in 4..8 {
        tt.store_twin(key, i, 0, Outcome::Draw, Move::NONE, 0, u32::MAX);
    }
    assert_eq!(tt.peak_twins(), 8);
}

#[test]
fn find_and_best_result_for_multiple_paths() {
    let mut tt = TranspositionTable::with_mb(1);
    let key = 123u64;

    // Twins for two different path codes. Storing a twin reinitialises the
    // base entry, so the table is left with only path-dependent results.
    tt.store_twin(key, 1, 0, Outcome::Loss, Move::NONE, 2, u32::MAX);
    tt.store_twin(key, 2, 0, Outcome::Draw, Move::NONE, 3, u32::MAX);

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
